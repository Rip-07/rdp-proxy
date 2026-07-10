use crate::config::{AppConfig, IpFilter};
use crate::metrics::Metrics;
use crate::shutdown::Shutdown;
use anyhow::{Context, Result};
use socket2::Socket;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tokio::io::{AsyncWriteExt, copy_bidirectional};
use tokio::net::{TcpSocket, TcpStream};
use tokio::sync::Notify;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tracing::{error, info, warn};

/// Run the TCP proxy until shutdown.
pub async fn run(
    listen_addr: SocketAddr,
    target_addr: SocketAddr,
    config: AppConfig,
    metrics: Arc<Metrics>,
    shutdown: Shutdown,
    connection_limiter: Option<Arc<Semaphore>>,
) -> Result<()> {
    let socket = socket_for(listen_addr)?;
    socket.set_reuseaddr(true)?;
    socket
        .bind(listen_addr)
        .with_context(|| format!("binding proxy listener to {listen_addr}"))?;
    let backlog = u32::try_from(config.backlog).context("backlog is too large")?;
    let listener = socket
        .listen(backlog)
        .with_context(|| format!("listening on {listen_addr}"))?;
    let sessions = Arc::new(SessionTracker::default());

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                break;
            }
            accept = listener.accept() => {
                let (client, peer) = match accept {
                    Ok(value) => value,
                    Err(err) => {
                        error!(error = %err, "accept failed");
                        continue;
                    }
                };

                if !is_allowed(&config.allowed_ips, peer.ip()) {
                    warn!(client_ip = %peer.ip(), "client rejected by IP whitelist");
                    metrics.failed_connection();
                    drop(client);
                    continue;
                }

                let permit = match &connection_limiter {
                    Some(limit) => match limit.clone().try_acquire_owned() {
                        Ok(permit) => Some(permit),
                        Err(_) => {
                            warn!(client_ip = %peer.ip(), "connection limit reached");
                            metrics.failed_connection();
                            drop(client);
                            continue;
                        }
                    },
                    None => None,
                };

                let metrics = metrics.clone();
                let config = config.clone();
                let sessions = sessions.clone();
                sessions.add();
                tokio::spawn(async move {
                    if let Err(err) = handle_session(client, peer, target_addr, config, metrics).await {
                        error!(client_ip = %peer.ip(), error = %err, "session failed");
                    }
                    drop(permit);
                    sessions.remove();
                });
            }
        }
    }

    sessions.wait_for_zero().await;
    Ok(())
}

async fn handle_session(
    client: TcpStream,
    peer: SocketAddr,
    target_addr: SocketAddr,
    config: AppConfig,
    metrics: Arc<Metrics>,
) -> Result<()> {
    let start = Instant::now();
    info!(
        client_ip = %peer.ip(),
        client_port = peer.port(),
        target_ip = %target_addr.ip(),
        target_port = target_addr.port(),
        "new connection"
    );

    let target = timeout(
        std::time::Duration::from_millis(config.connect_timeout_ms),
        TcpStream::connect(target_addr),
    )
    .await;

    let target = match target {
        Ok(connect) => match connect.with_context(|| format!("connecting to {target_addr}")) {
            Ok(stream) => stream,
            Err(err) => {
                metrics.failed_connection();
                return Err(err);
            }
        },
        Err(_) => {
            metrics.failed_connection();
            return Err(anyhow::anyhow!(
                "connect timeout after {} ms",
                config.connect_timeout_ms
            ));
        }
    };

    let client = match configure_stream(client, &config) {
        Ok(stream) => stream,
        Err(err) => {
            metrics.failed_connection();
            return Err(err);
        }
    };
    let target = match configure_stream(target, &config) {
        Ok(stream) => stream,
        Err(err) => {
            metrics.failed_connection();
            return Err(err);
        }
    };

    metrics.connection_opened();

    let (uploaded, downloaded, reason) = match relay(client, target).await {
        Ok(value) => value,
        Err(err) => {
            metrics.failed_connection();
            metrics.connection_closed();
            return Err(err);
        }
    };

    metrics.add_bytes_in(uploaded);
    metrics.add_bytes_out(downloaded);
    metrics.connection_closed();

    let elapsed = start.elapsed();
    info!(
        client_ip = %peer.ip(),
        target_ip = %target_addr.ip(),
        duration_ms = elapsed.as_millis(),
        bytes_uploaded = uploaded,
        bytes_downloaded = downloaded,
        disconnect_reason = reason,
        "connection closed"
    );

    Ok(())
}

async fn relay(mut client: TcpStream, mut target: TcpStream) -> Result<(u64, u64, &'static str)> {
    let relay_result = copy_bidirectional(&mut client, &mut target).await;
    let _ = client.shutdown().await;
    let _ = target.shutdown().await;
    let (uploaded, downloaded) = relay_result.context("bidirectional relay failed")?;
    Ok((uploaded, downloaded, "peer closed"))
}

#[derive(Debug, Default)]
struct SessionTracker {
    active: AtomicUsize,
    notify: Notify,
}

impl SessionTracker {
    fn add(&self) {
        self.active.fetch_add(1, Ordering::Relaxed);
    }

    fn remove(&self) {
        if self.active.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.notify.notify_waiters();
        }
    }

    async fn wait_for_zero(&self) {
        loop {
            if self.active.load(Ordering::Acquire) == 0 {
                return;
            }
            self.notify.notified().await;
        }
    }
}

fn configure_stream(stream: TcpStream, config: &AppConfig) -> Result<TcpStream> {
    let std_stream = stream.into_std()?;
    let socket = Socket::from(std_stream);
    socket.set_tcp_nodelay(config.tcp_nodelay)?;
    socket.set_keepalive(config.tcp_keepalive)?;
    let std_stream: std::net::TcpStream = socket.into();
    std_stream.set_nonblocking(true)?;
    Ok(TcpStream::from_std(std_stream)?)
}

fn socket_for(addr: SocketAddr) -> Result<TcpSocket> {
    Ok(match addr.ip() {
        IpAddr::V4(_) => TcpSocket::new_v4()?,
        IpAddr::V6(_) => TcpSocket::new_v6()?,
    })
}

fn is_allowed(filters: &[IpFilter], ip: IpAddr) -> bool {
    filters.is_empty() || filters.iter().any(|filter| filter.matches(ip))
}
