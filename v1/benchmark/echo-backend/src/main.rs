//! Ultra-fast HTTP echo backend for Ando benchmarks.
//!
//! Responds with `200 OK` + `ok` body for every request.
//! Supports HTTP/1.1 keep-alive (persistent connections) which is
//! critical for accurate gateway proxy benchmarks.

use clap::Parser;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[derive(Parser)]
#[command(name = "echo-backend", about = "Ultra-fast HTTP benchmark backend")]
struct Cli {
    /// Listen address
    #[arg(short, long, default_value = "0.0.0.0:3000")]
    addr: SocketAddr,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Use all available cores for maximum throughput
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(threads)
        .enable_all()
        .build()?
        .block_on(run(cli.addr))
}

async fn run(addr: SocketAddr) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    eprintln!(
        "[echo-backend] listening on http://{} (keep-alive, {} threads)",
        addr,
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    );

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            let conn = http1::Builder::new()
                .keep_alive(true)
                .serve_connection(io, service_fn(handle));

            if let Err(e) = conn.await {
                // Suppress benign wrk-teardown errors
                let msg = e.to_string();
                if !msg.contains("connection closed")
                    && !msg.contains("Connection reset")
                    && !msg.contains("incomplete message")
                {
                    eprintln!("[echo-backend] conn error: {e}");
                }
            }
        });
    }
}

/// Responds instantly with 200 OK + "ok" body.
async fn handle(
    _req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::builder()
        .status(200)
        .header("content-type", "text/plain")
        .header("content-length", "2")
        .body(Full::new(Bytes::from_static(b"ok")))
        .unwrap())
}
