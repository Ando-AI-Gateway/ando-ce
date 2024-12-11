use crate::proxy::ProxyWorker;
use crate::worker::SharedState;
use monoio::io::{AsyncReadRent, AsyncWriteRentExt};
use monoio::net::TcpStream;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::debug;

/// Handle a single client connection.
///
/// v2 design: Each connection is processed directly on the monoio worker
/// thread — no task scheduling, no future boxing. The HTTP parsing and
/// proxy logic run inline in the accept loop's async context.
///
/// For HTTP/1.1 keepalive connections, we loop over requests.
pub async fn handle_connection(
    mut client: TcpStream,
    peer_addr: SocketAddr,
    shared: &Arc<SharedState>,
) -> anyhow::Result<()> {
    let client_ip = peer_addr.ip().to_string();

    // Create a thread-local proxy worker for this connection
    let mut proxy = ProxyWorker::new(
        shared.router.load_full(),
        Arc::clone(&shared.plugin_registry),
        shared.config_cache.clone(),
        Arc::clone(&shared.config),
    );

    // Read buffer — reused across keepalive requests
    let mut buf = vec![0u8; 8192];

    loop {
        // Read request data
        let (res, read_buf) = client.read(buf).await;
        buf = read_buf;
        let n = match res {
            Ok(0) => return Ok(()), // Connection closed
            Ok(n) => n,
            Err(e) => return Err(e.into()),
        };

        // Parse HTTP request
        let mut headers_buf = [httparse::EMPTY_HEADER; 64];
        let mut req = httparse::Request::new(&mut headers_buf);

        match req.parse(&buf[..n]) {
            Ok(httparse::Status::Complete(body_offset)) => {
                let method = req.method.unwrap_or("GET");
                let path = req.path.unwrap_or("/");

                // Extract headers
                let headers: Vec<(String, String)> = req
                    .headers
                    .iter()
                    .filter(|h| h.name != httparse::EMPTY_HEADER.name)
                    .map(|h| {
                        (
                            h.name.to_lowercase(),
                            String::from_utf8_lossy(h.value).to_string(),
                        )
                    })
                    .collect();

                let host = headers
                    .iter()
                    .find(|(k, _)| k == "host")
                    .map(|(_, v)| v.as_str());

                // Check for router updates
                let current_router = shared.router.load_full();
                proxy.maybe_update_router(current_router);

                // Execute proxy logic
                let response = proxy.handle_request(method, path, host, &headers, &client_ip);

                if response.status == 0 {
                    // Proxy to upstream
                    if let Some(ref upstream_addr) = response.upstream_addr {
                        match proxy_to_upstream(
                            &mut client,
                            upstream_addr,
                            method,
                            path,
                            &headers,
                            &buf[body_offset..n],
                        )
                        .await
                        {
                            Ok(_) => {}
                            Err(e) => {
                                let error_resp = format!(
                                    "HTTP/1.1 502 Bad Gateway\r\ncontent-type: application/json\r\ncontent-length: 42\r\nconnection: keep-alive\r\n\r\n{{\"error\":\"upstream error\",\"status\":502}}"
                                );
                                let (res, _) = client.write_all(error_resp.into_bytes()).await;
                                res?;
                                debug!(error = %e, "Upstream proxy error");
                            }
                        }
                    }
                } else {
                    // Direct response from plugin (e.g., 401, 403, 429)
                    let body = &response.body;
                    let mut resp = format!(
                        "HTTP/1.1 {} {}\r\ncontent-length: {}\r\nconnection: keep-alive\r\n",
                        response.status,
                        status_text(response.status),
                        body.len(),
                    );
                    for (k, v) in &response.headers {
                        resp.push_str(&format!("{}: {}\r\n", k, v));
                    }
                    resp.push_str("\r\n");

                    let mut resp_bytes = resp.into_bytes();
                    resp_bytes.extend_from_slice(body);
                    let (res, _) = client.write_all(resp_bytes).await;
                    res?;
                }

                // Check if connection should be kept alive
                let keep_alive = headers
                    .iter()
                    .find(|(k, _)| k == "connection")
                    .map(|(_, v)| !v.eq_ignore_ascii_case("close"))
                    .unwrap_or(true); // HTTP/1.1 default is keep-alive

                if !keep_alive {
                    return Ok(());
                }
            }
            Ok(httparse::Status::Partial) => {
                // Need more data — for now, just send 400
                let resp = b"HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
                let (res, _) = client.write_all(resp.to_vec()).await;
                res?;
                return Ok(());
            }
            Err(e) => {
                debug!(error = %e, "HTTP parse error");
                let resp = b"HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
                let (res, _) = client.write_all(resp.to_vec()).await;
                res?;
                return Ok(());
            }
        }
    }
}

/// Proxy request to upstream and stream response back to client.
async fn proxy_to_upstream(
    client: &mut TcpStream,
    upstream_addr: &str,
    method: &str,
    path: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> anyhow::Result<()> {
    // Connect to upstream
    let mut upstream = TcpStream::connect(upstream_addr).await?;

    // Build HTTP request to upstream
    let mut req = format!("{} {} HTTP/1.1\r\n", method, path);
    for (k, v) in headers {
        // Skip hop-by-hop headers
        if k == "connection" || k == "keep-alive" || k == "transfer-encoding" || k == "upgrade" {
            continue;
        }
        req.push_str(&format!("{}: {}\r\n", k, v));
    }
    req.push_str("connection: keep-alive\r\n");
    if !body.is_empty() {
        req.push_str(&format!("content-length: {}\r\n", body.len()));
    }
    req.push_str("\r\n");

    let mut req_bytes = req.into_bytes();
    if !body.is_empty() {
        req_bytes.extend_from_slice(body);
    }

    // Send to upstream
    let (res, _) = upstream.write_all(req_bytes).await;
    res?;

    // Read upstream response and forward to client
    let resp_buf = vec![0u8; 65536];
    let (res, resp_buf) = upstream.read(resp_buf).await;
    let n = res?;

    if n == 0 {
        return Err(anyhow::anyhow!("Upstream closed connection"));
    }

    // Forward the response as-is to the client
    let (res, _) = client.write_all(resp_buf[..n].to_vec()).await;
    res?;

    Ok(())
}

/// HTTP status code to reason phrase.
fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Unknown",
    }
}
