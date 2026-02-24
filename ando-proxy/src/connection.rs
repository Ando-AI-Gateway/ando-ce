use crate::proxy::{
    ConnPool, ProxyWorker, RESP_502, RequestResult, build_response, build_upstream_request,
};
use monoio::io::{AsyncReadRent, AsyncWriteRentExt};
use monoio::net::TcpStream;
use std::cell::RefCell;
use std::net::SocketAddr;
use std::rc::Rc;

/// Resolve an `addr` string (e.g. `"localhost:3001"`) to a list of `SocketAddr`s.
///
/// We resolve explicitly via std's blocking `ToSocketAddrs` before passing
/// to monoio's `TcpStream::connect`.  Monoio's internal hostname-resolution
/// path can behave differently on macOS (e.g. under FusionDriver) when the
/// kernel AIO interface does not support `getaddrinfo`.  The blocking call is
/// acceptable here because it only runs when the connection pool is empty
/// (startup, first request, or after upstream failure) — it is NOT on the
/// steady-state hot path.
///
/// Returns candidates sorted IPv4-first, because on macOS `localhost` resolves
/// to both `::1` (IPv6) and `127.0.0.1` (IPv4), and `.next()` often returns
/// `::1` first.  Most upstream servers listen on IPv4-only, so we try IPv4
/// first to avoid spurious "Connection refused" on the IPv6 address.
fn resolve_addrs(addr: &str) -> Vec<SocketAddr> {
    // Fast path: already an IP:port literal
    if let Ok(sa) = addr.parse::<SocketAddr>() {
        return vec![sa];
    }
    // Slow path: DNS/hosts lookup (blocking — intentional, see above)
    use std::net::ToSocketAddrs;
    let all: Vec<SocketAddr> = match addr.to_socket_addrs() {
        Ok(iter) => iter.collect(),
        Err(_) => return vec![],
    };
    // Sort: IPv4 addresses before IPv6
    let mut v4: Vec<SocketAddr> = all.iter().copied().filter(|a| a.is_ipv4()).collect();
    let v6: Vec<SocketAddr> = all.iter().copied().filter(|a| a.is_ipv6()).collect();
    v4.extend(v6);
    v4
}

/// Open a new TCP connection to `addr`, trying all resolved addresses
/// (IPv4-first) and returning the first that succeeds.
async fn new_upstream_conn(addr: &str) -> Option<TcpStream> {
    let candidates = resolve_addrs(addr);
    if candidates.is_empty() {
        tracing::warn!(addr = %addr, "Upstream address resolve failed");
        return None;
    }
    for sa in &candidates {
        match TcpStream::connect(*sa).await {
            Ok(s) => {
                let _ = s.set_nodelay(true);
                tracing::debug!(addr = %addr, resolved = %sa, "Upstream connected");
                return Some(s);
            }
            Err(e) => {
                tracing::debug!(addr = %addr, resolved = %sa, error = %e, "Upstream candidate failed, trying next");
            }
        }
    }
    tracing::warn!(addr = %addr, tried = candidates.len(), "Upstream connect failed on all candidates");
    None
}

/// Handle a single client connection (HTTP/1.1 with keepalive).
///
/// Shares ProxyWorker and ConnPool with all other connections
/// on this thread via Rc<RefCell> — zero atomic overhead.
///
/// Optimizations:
///   - All buffers allocated ONCE, reused across keepalive requests
///   - Zero-copy header parsing (httparse &str refs into read buffer)
///   - TCP_NODELAY on new upstream connections
///   - Connection pool with stale-retry
///   - Upstream response streaming for large bodies
pub async fn handle_connection(
    mut client: TcpStream,
    peer_addr: SocketAddr,
    proxy: Rc<RefCell<ProxyWorker>>,
    conn_pool: Rc<RefCell<ConnPool>>,
) -> anyhow::Result<()> {
    let client_ip = peer_addr.ip().to_string();

    // ── All buffers allocated ONCE, reused across keepalive requests ──
    let mut read_buf = vec![0u8; 8192];
    let mut upstream_req_buf = Vec::with_capacity(2048);
    let mut resp_buf = Vec::with_capacity(4096);
    let mut upstream_buf = vec![0u8; 65536];

    loop {
        // ── Read request ──
        let (res, returned_buf) = client.read(read_buf).await;
        read_buf = returned_buf;
        let n = match res {
            Ok(0) => return Ok(()),
            Ok(n) => n,
            Err(e) => return Err(e.into()),
        };

        // ── Parse HTTP request ──
        let mut headers_raw = [httparse::EMPTY_HEADER; 64];
        let mut req = httparse::Request::new(&mut headers_raw);

        match req.parse(&read_buf[..n]) {
            Ok(httparse::Status::Complete(body_offset)) => {
                let method = req.method.unwrap_or("GET");
                let path = req.path.unwrap_or("/");

                // Zero-copy header extraction (references into read_buf)
                let mut headers: Vec<(&str, &str)> = Vec::with_capacity(16);
                let mut host: Option<&str> = None;
                let mut keep_alive = true;

                for h in req.headers.iter() {
                    if h.name.is_empty() {
                        break;
                    }
                    let val = std::str::from_utf8(h.value).unwrap_or("");
                    headers.push((h.name, val));
                    if h.name.eq_ignore_ascii_case("host") {
                        host = Some(val);
                    } else if h.name.eq_ignore_ascii_case("connection") {
                        keep_alive = !val.eq_ignore_ascii_case("close");
                    }
                }

                // ── Process request (brief RefCell borrow, NO await) ──
                let result = {
                    let mut pw = proxy.borrow_mut();
                    pw.handle_request(method, path, host, &headers, &client_ip)
                };
                // Borrow dropped here — safe to do async I/O

                match result {
                    RequestResult::Proxy {
                        ref upstream_addr,
                        ref upstream_path,
                    } => {
                        // Build upstream request while header refs are valid
                        let body_data = &read_buf[body_offset..n];
                        build_upstream_request(
                            &mut upstream_req_buf,
                            method,
                            upstream_path,
                            &headers,
                            body_data,
                        );

                        // Get or open upstream connection
                        let maybe_conn = conn_pool.borrow_mut().take(upstream_addr);
                        let mut upstream = match maybe_conn {
                            Some(s) => s,
                            None => match new_upstream_conn(upstream_addr).await {
                                Some(s) => s,
                                None => {
                                    let (res, _) = client.write_all(RESP_502.to_vec()).await;
                                    res?;
                                    if !keep_alive {
                                        return Ok(());
                                    }
                                    continue;
                                }
                            },
                        };

                        // Send request to upstream
                        let req_data = upstream_req_buf.clone();
                        let (res, _) = upstream.write_all(req_data).await;
                        if res.is_err() {
                            // Pooled conn was stale — retry with a fresh connection
                            match new_upstream_conn(upstream_addr).await {
                                Some(mut new_upstream) => {
                                    let req_data = upstream_req_buf.clone();
                                    let (res, _) = new_upstream.write_all(req_data).await;
                                    if res.is_err() {
                                        tracing::warn!(addr = %upstream_addr, "Upstream write failed after reconnect");
                                        let (res, _) = client.write_all(RESP_502.to_vec()).await;
                                        res?;
                                        if !keep_alive {
                                            return Ok(());
                                        }
                                        continue;
                                    }
                                    upstream = new_upstream;
                                }
                                None => {
                                    let (res, _) = client.write_all(RESP_502.to_vec()).await;
                                    res?;
                                    if !keep_alive {
                                        return Ok(());
                                    }
                                    continue;
                                }
                            }
                        }

                        // Read upstream response — reuse buffer across keepalive
                        let (res, returned_ubuf) = upstream.read(upstream_buf).await;
                        upstream_buf = returned_ubuf;
                        let resp_n = match res {
                            Ok(0) => {
                                tracing::warn!(addr = %upstream_addr, "Upstream closed connection without response");
                                let (res, _) = client.write_all(RESP_502.to_vec()).await;
                                res?;
                                if !keep_alive {
                                    return Ok(());
                                }
                                continue;
                            }
                            Ok(n) => n,
                            Err(e) => {
                                tracing::warn!(addr = %upstream_addr, error = %e, "Upstream read error");
                                let (res, _) = client.write_all(RESP_502.to_vec()).await;
                                res?;
                                if !keep_alive {
                                    return Ok(());
                                }
                                continue;
                            }
                        };

                        // Parse upstream response headers for content-length
                        let mut resp_headers = [httparse::EMPTY_HEADER; 64];
                        let mut resp = httparse::Response::new(&mut resp_headers);
                        let mut content_length: Option<usize> = None;
                        let mut upstream_keepalive = true;

                        if let Ok(httparse::Status::Complete(hdr_len)) =
                            resp.parse(&upstream_buf[..resp_n])
                        {
                            for h in resp.headers.iter() {
                                if h.name.is_empty() {
                                    break;
                                }
                                if h.name.eq_ignore_ascii_case("content-length") {
                                    content_length = std::str::from_utf8(h.value)
                                        .ok()
                                        .and_then(|s| s.parse().ok());
                                }
                                if h.name.eq_ignore_ascii_case("connection") {
                                    let v = std::str::from_utf8(h.value).unwrap_or("");
                                    upstream_keepalive = !v.eq_ignore_ascii_case("close");
                                }
                            }

                            // Forward first chunk to client
                            let first_chunk = upstream_buf[..resp_n].to_vec();
                            let (res, _) = client.write_all(first_chunk).await;
                            res?;

                            // Stream remaining body if needed
                            if let Some(cl) = content_length {
                                let body_in_first = resp_n - hdr_len;
                                let mut remaining = cl.saturating_sub(body_in_first);

                                while remaining > 0 {
                                    let chunk_size = remaining.min(65536);
                                    let mut chunk_buf = vec![0u8; chunk_size];
                                    let (res, returned_chunk) = upstream.read(chunk_buf).await;
                                    chunk_buf = returned_chunk;
                                    let cn = match res {
                                        Ok(0) => break,
                                        Ok(n) => n,
                                        Err(_) => break,
                                    };
                                    remaining -= cn;
                                    let data = chunk_buf[..cn].to_vec();
                                    let (res, _) = client.write_all(data).await;
                                    if res.is_err() {
                                        return Ok(());
                                    }
                                }
                            }
                        } else {
                            // Couldn't parse response headers — forward raw
                            let data = upstream_buf[..resp_n].to_vec();
                            let (res, _) = client.write_all(data).await;
                            res?;
                            upstream_keepalive = false;
                        }

                        // Return upstream connection to pool if keepalive
                        if upstream_keepalive {
                            conn_pool.borrow_mut().put(upstream_addr.clone(), upstream);
                        }
                    }

                    RequestResult::Static(resp_bytes) => {
                        let (res, _) = client.write_all(resp_bytes.to_vec()).await;
                        res?;
                    }

                    RequestResult::PluginResponse {
                        status,
                        ref headers,
                        ref body,
                    } => {
                        build_response(&mut resp_buf, status, headers, body);
                        let data = resp_buf.clone();
                        let (res, _) = client.write_all(data).await;
                        res?;
                    }
                }

                if !keep_alive {
                    return Ok(());
                }
            }
            Ok(httparse::Status::Partial) => {
                let resp =
                    b"HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
                let (res, _) = client.write_all(resp.to_vec()).await;
                res?;
                return Ok(());
            }
            Err(e) => {
                tracing::debug!(error = %e, "HTTP parse error");
                let resp =
                    b"HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
                let (res, _) = client.write_all(resp.to_vec()).await;
                res?;
                return Ok(());
            }
        }
    }
}
