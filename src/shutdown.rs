use anyhow::Result;
use std::future::pending;
use std::sync::Arc;
use tokio::sync::Notify;

/// Cooperative shutdown coordinator.
#[derive(Clone, Debug)]
pub struct Shutdown {
    notify: Arc<Notify>,
}

impl Shutdown {
    /// Create a new shutdown coordinator.
    pub fn new() -> Self {
        Self {
            notify: Arc::new(Notify::new()),
        }
    }

    /// Return a clone of the shutdown handle.
    pub fn handle(&self) -> Self {
        self.clone()
    }

    /// Trigger shutdown.
    pub fn trigger(&self) {
        self.notify.notify_waiters();
    }

    /// Wait until shutdown has been triggered.
    pub async fn cancelled(&self) {
        self.notify.notified().await;
    }
}

/// Wait for SIGINT or SIGTERM.
pub async fn wait_for_signal() -> Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut sigterm = signal(SignalKind::terminate())?;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
        Ok(())
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await?;
        Ok(())
    }
}

/// Wait forever when no shutdown signal is needed.
pub async fn _never() -> ! {
    pending::<()>().await;
    unreachable!()
}
