use std::fmt::Write;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// In-memory proxy metrics.
#[derive(Debug, Default)]
pub struct Metrics {
    active_connections: AtomicUsize,
    total_connections: AtomicU64,
    bytes_in: AtomicU64,
    bytes_out: AtomicU64,
    failed_connections: AtomicU64,
}

impl Metrics {
    /// Record a successfully established session.
    pub fn connection_opened(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
        self.total_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a closed session.
    pub fn connection_closed(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Record inbound bytes from client to target.
    pub fn add_bytes_in(&self, value: u64) {
        self.bytes_in.fetch_add(value, Ordering::Relaxed);
    }

    /// Record outbound bytes from target to client.
    pub fn add_bytes_out(&self, value: u64) {
        self.bytes_out.fetch_add(value, Ordering::Relaxed);
    }

    /// Record a failed connection attempt.
    pub fn failed_connection(&self) {
        self.failed_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Render Prometheus exposition format.
    pub fn render_prometheus(&self) -> String {
        let mut output = String::new();
        let active = self.active_connections.load(Ordering::Relaxed);
        let total = self.total_connections.load(Ordering::Relaxed);
        let bytes_in = self.bytes_in.load(Ordering::Relaxed);
        let bytes_out = self.bytes_out.load(Ordering::Relaxed);
        let failed = self.failed_connections.load(Ordering::Relaxed);

        let _ = writeln!(
            output,
            "# HELP active_connections Current active TCP sessions."
        );
        let _ = writeln!(output, "# TYPE active_connections gauge");
        let _ = writeln!(output, "active_connections {active}");
        let _ = writeln!(
            output,
            "# HELP total_connections Total TCP sessions accepted."
        );
        let _ = writeln!(output, "# TYPE total_connections counter");
        let _ = writeln!(output, "total_connections {total}");
        let _ = writeln!(output, "# HELP bytes_in Bytes received from clients.");
        let _ = writeln!(output, "# TYPE bytes_in counter");
        let _ = writeln!(output, "bytes_in {bytes_in}");
        let _ = writeln!(output, "# HELP bytes_out Bytes sent to clients.");
        let _ = writeln!(output, "# TYPE bytes_out counter");
        let _ = writeln!(output, "bytes_out {bytes_out}");
        let _ = writeln!(
            output,
            "# HELP failed_connections Failed connection attempts."
        );
        let _ = writeln!(output, "# TYPE failed_connections counter");
        let _ = writeln!(output, "failed_connections {failed}");

        output
    }
}
