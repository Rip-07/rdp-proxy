use crate::metrics::Metrics;
use crate::shutdown::Shutdown;
use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error};

/// Run the health endpoint server.
pub async fn run_health_server(addr: SocketAddr, shutdown: Shutdown) -> Result<()> {
    run_server(addr, shutdown, "/health", |_| {
        "HTTP/1.1 200 OK\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: 2\r\nconnection: close\r\n\r\nOK"
            .to_string()
    })
    .await
}

/// Run the metrics endpoint server.
pub async fn run_metrics_server(
    addr: SocketAddr,
    shutdown: Shutdown,
    metrics: Arc<Metrics>,
) -> Result<()> {
    run_server(addr, shutdown, "/metrics", move |_| {
        let body = metrics.render_prometheus();
        format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain; version=0.0.4; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    })
    .await
}

async fn run_server<F>(
    addr: SocketAddr,
    shutdown: Shutdown,
    expected_path: &'static str,
    handler: F,
) -> Result<()>
where
    F: Fn(&str) -> String + Send + Sync + Clone + 'static,
{
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding HTTP server to {addr}"))?;

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                return Ok(());
            }
            accept = listener.accept() => {
                match accept {
                    Ok((stream, peer)) => {
                        let handler = handler.clone();
                        let expected_path = expected_path;
                        tokio::spawn(async move {
                            if let Err(err) = handle_connection(stream, peer, expected_path, handler).await {
                                error!(%peer, error = %err, "http connection failed");
                            }
                        });
                    }
                    Err(err) => {
                        error!(error = %err, "http accept failed");
                    }
                }
            }
        }
    }
}

async fn handle_connection<F>(
    mut stream: TcpStream,
    peer: SocketAddr,
    expected_path: &'static str,
    handler: F,
) -> Result<()>
where
    F: Fn(&str) -> String,
{
    let mut request = vec![0u8; 1024];
    let n = stream.read(&mut request).await?;
    let request = String::from_utf8_lossy(&request[..n]);
    let request_line = request.lines().next().unwrap_or_default();
    debug!(%peer, %request_line, "http request");

    let response = if request_line.starts_with(&format!("GET {expected_path} ")) {
        handler(request_line)
    } else {
        "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_string()
    };

    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}
