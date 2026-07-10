mod config;
mod errors;
mod http;
mod metrics;
mod proxy;
mod shutdown;

use crate::config::AppConfig;
use anyhow::Context;
use clap::Parser;
use metrics::Metrics;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::info;

/// Command-line arguments.
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the YAML configuration file.
    #[arg(long, env = "RDP_PROXY_CONFIG", default_value = "config.yaml")]
    config: std::path::PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::load(&cli.config).context("loading configuration")?;
    config.init_tracing().context("initializing tracing")?;

    let metrics = Arc::new(Metrics::default());
    let shutdown = shutdown::Shutdown::new();
    let connection_limiter = config
        .connection_limit
        .map(|limit| Arc::new(Semaphore::new(limit)));

    let proxy_addr = SocketAddr::new(config.listen_host, config.listen_port);
    let target_addr = SocketAddr::new(config.target_host, config.target_port);

    info!(
        listen = %proxy_addr,
        target = %target_addr,
        "starting proxy"
    );

    let health_handle = tokio::spawn(http::run_health_server(
        "127.0.0.1:8080".parse().expect("valid health address"),
        shutdown.handle(),
    ));

    let metrics_handle = tokio::spawn(http::run_metrics_server(
        "127.0.0.1:9090".parse().expect("valid metrics address"),
        shutdown.handle(),
        metrics.clone(),
    ));

    let proxy_future = proxy::run(
        proxy_addr,
        target_addr,
        config.clone(),
        metrics.clone(),
        shutdown.handle(),
        connection_limiter,
    );

    tokio::pin!(proxy_future);

    let proxy_finished = tokio::select! {
        result = &mut proxy_future => {
            Some(result)
        }
        signal = shutdown::wait_for_signal() => {
            signal.context("waiting for termination signal")?;
            shutdown.trigger();
            None
        }
    };

    let proxy_error = if let Some(result) = proxy_finished {
        match result.context("proxy server") {
            Ok(()) => {
                shutdown.trigger();
                None
            }
            Err(err) => {
                shutdown.trigger();
                Some(err)
            }
        }
    } else {
        proxy_future.await.context("proxy server")?;
        None
    };

    health_handle
        .await
        .context("health task join")?
        .context("health server")?;
    metrics_handle
        .await
        .context("metrics task join")?
        .context("metrics server")?;

    if let Some(err) = proxy_error {
        return Err(err);
    }

    Ok(())
}
