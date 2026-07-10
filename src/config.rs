use crate::errors::AppError;
use anyhow::Context;
use anyhow::Result as AnyResult;
use serde::Deserialize;
use std::env;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use tracing_subscriber::EnvFilter;

/// Runtime configuration for the proxy.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Address the proxy listens on.
    pub listen_host: IpAddr,
    /// TCP port the proxy listens on.
    pub listen_port: u16,
    /// Target host for forwarded connections.
    pub target_host: IpAddr,
    /// Target port for forwarded connections.
    pub target_port: u16,
    /// TCP connect timeout in milliseconds.
    pub connect_timeout_ms: u64,
    /// TCP keepalive enabled for client and upstream sockets.
    pub tcp_keepalive: bool,
    /// TCP_NODELAY enabled for client and upstream sockets.
    pub tcp_nodelay: bool,
    /// Listener backlog.
    pub backlog: usize,
    /// Logging level string, for example `info` or `debug`.
    pub log_level: String,
    /// Log output format, `text` or `json`.
    pub log_format: LogFormat,
    /// Optional connection limit.
    pub connection_limit: Option<usize>,
    /// Optional allowed client IPs or CIDRs.
    pub allowed_ips: Vec<IpFilter>,
}

/// Log format selection.
#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// Human-readable text logs.
    #[default]
    Text,
    /// JSON logs.
    Json,
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    listen_host: Option<IpAddr>,
    listen_port: Option<u16>,
    target_host: Option<IpAddr>,
    target_port: Option<u16>,
    connect_timeout_ms: Option<u64>,
    tcp_keepalive: Option<bool>,
    tcp_nodelay: Option<bool>,
    backlog: Option<usize>,
    log_level: Option<String>,
    log_format: Option<LogFormat>,
    connection_limit: Option<usize>,
    allowed_ips: Option<Vec<String>>,
}

/// A client IP filter.
#[derive(Debug, Clone)]
pub enum IpFilter {
    /// Exact IP address.
    Exact(IpAddr),
    /// IPv4 or IPv6 CIDR range.
    Cidr(Cidr),
}

/// IP network in CIDR notation.
#[derive(Debug, Clone)]
pub struct Cidr {
    network: IpAddr,
    prefix_len: u8,
}

impl Cidr {
    fn contains(&self, ip: IpAddr) -> bool {
        match (self.network, ip) {
            (IpAddr::V4(network), IpAddr::V4(addr)) => {
                let prefix = self.prefix_len.min(32);
                let network = u32::from(network);
                let addr = u32::from(addr);
                let mask = if prefix == 0 {
                    0
                } else {
                    u32::MAX << (32 - prefix)
                };
                (network & mask) == (addr & mask)
            }
            (IpAddr::V6(network), IpAddr::V6(addr)) => {
                let prefix = self.prefix_len.min(128);
                let network = u128::from(network);
                let addr = u128::from(addr);
                let mask = if prefix == 0 {
                    0
                } else {
                    u128::MAX << (128 - prefix)
                };
                (network & mask) == (addr & mask)
            }
            _ => false,
        }
    }
}

impl IpFilter {
    fn parse(value: &str) -> AnyResult<Self> {
        if let Ok(ip) = value.parse::<IpAddr>() {
            return Ok(Self::Exact(ip));
        }

        let (network, prefix_len) = value.split_once('/').context("missing CIDR prefix")?;
        let network = network.parse::<IpAddr>().context("invalid network IP")?;
        let prefix_len = prefix_len.parse::<u8>().context("invalid prefix length")?;
        Ok(Self::Cidr(Cidr {
            network,
            prefix_len,
        }))
    }

    /// Returns whether a client IP matches this filter.
    pub fn matches(&self, ip: IpAddr) -> bool {
        match self {
            Self::Exact(expected) => *expected == ip,
            Self::Cidr(cidr) => cidr.contains(ip),
        }
    }
}

impl AppConfig {
    /// Load configuration from YAML and environment variables.
    pub fn load(path: &Path) -> std::result::Result<Self, AppError> {
        let file = if path.exists() {
            let contents = fs::read_to_string(path).map_err(AppError::Io)?;
            serde_yaml::from_str::<FileConfig>(&contents).map_err(|err| {
                AppError::Config(format!("parsing configuration file {path:?}: {err}"))
            })?
        } else {
            FileConfig::default()
        };

        let listen_host = env_ip("RDP_PROXY_LISTEN_HOST")
            .or(file.listen_host)
            .unwrap_or(Ipv4Addr::UNSPECIFIED.into());
        let listen_port = env_u16("RDP_PROXY_LISTEN_PORT")
            .or(file.listen_port)
            .unwrap_or(3389);
        let target_host = env_ip("RDP_PROXY_TARGET_HOST")
            .or(file.target_host)
            .unwrap_or_else(|| Ipv4Addr::new(192, 0, 2, 10).into());
        let target_port = env_u16("RDP_PROXY_TARGET_PORT")
            .or(file.target_port)
            .unwrap_or(3389);
        let connect_timeout_ms = env_u64("RDP_PROXY_CONNECT_TIMEOUT_MS")
            .or(file.connect_timeout_ms)
            .unwrap_or(10_000);
        let tcp_keepalive = env_bool("RDP_PROXY_TCP_KEEPALIVE")
            .or(file.tcp_keepalive)
            .unwrap_or(true);
        let tcp_nodelay = env_bool("RDP_PROXY_TCP_NODELAY")
            .or(file.tcp_nodelay)
            .unwrap_or(true);
        let backlog = env_usize("RDP_PROXY_BACKLOG")
            .or(file.backlog)
            .unwrap_or(1024);
        let log_level = env_string("RDP_PROXY_LOG_LEVEL")
            .or(file.log_level)
            .unwrap_or_else(|| "info".to_string());
        let log_format = env_string("RDP_PROXY_LOG_FORMAT")
            .and_then(|value| parse_log_format(&value).ok())
            .or(file.log_format)
            .unwrap_or_default();
        let connection_limit = env_usize("RDP_PROXY_CONNECTION_LIMIT").or(file.connection_limit);
        let allowed_ips = match env_allowed_ips("RDP_PROXY_ALLOWED_IPS").or_else(|| {
            file.allowed_ips
                .map(|values| parse_allowed_ips(values.as_slice()))
        }) {
            Some(Ok(values)) => values,
            Some(Err(err)) => {
                return Err(AppError::Config(format!("parsing allowed_ips: {err}")));
            }
            None => Vec::new(),
        };

        Ok(Self {
            listen_host,
            listen_port,
            target_host,
            target_port,
            connect_timeout_ms,
            tcp_keepalive,
            tcp_nodelay,
            backlog,
            log_level,
            log_format,
            connection_limit,
            allowed_ips,
        })
    }

    /// Initialize tracing according to the selected format and level.
    pub fn init_tracing(&self) -> std::result::Result<(), AppError> {
        let filter = EnvFilter::try_new(&self.log_level)
            .or_else(|_| EnvFilter::try_new("info"))
            .map_err(|err| AppError::Config(format!("building tracing filter: {err}")))?;

        let builder = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(false)
            .with_ansi(false);

        match self.log_format {
            LogFormat::Text => builder
                .compact()
                .try_init()
                .map_err(|err| AppError::Config(format!("installing tracing subscriber: {err}")))?,
            LogFormat::Json => builder
                .json()
                .try_init()
                .map_err(|err| AppError::Config(format!("installing tracing subscriber: {err}")))?,
        }

        Ok(())
    }
}

fn parse_log_format(value: &str) -> AnyResult<LogFormat> {
    match value.trim().to_ascii_lowercase().as_str() {
        "text" => Ok(LogFormat::Text),
        "json" => Ok(LogFormat::Json),
        other => Err(anyhow::anyhow!("unsupported log format: {other}")),
    }
}

fn parse_allowed_ips(values: &[String]) -> AnyResult<Vec<IpFilter>> {
    values.iter().map(|value| IpFilter::parse(value)).collect()
}

fn env_string(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.trim().is_empty())
}

fn env_bool(key: &str) -> Option<bool> {
    env_string(key).and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    })
}

fn env_u16(key: &str) -> Option<u16> {
    env_string(key).and_then(|value| value.parse().ok())
}

fn env_u64(key: &str) -> Option<u64> {
    env_string(key).and_then(|value| value.parse().ok())
}

fn env_usize(key: &str) -> Option<usize> {
    env_string(key).and_then(|value| value.parse().ok())
}

fn env_ip(key: &str) -> Option<IpAddr> {
    env_string(key).and_then(|value| value.parse().ok())
}

fn env_allowed_ips(key: &str) -> Option<AnyResult<Vec<IpFilter>>> {
    let raw = env_string(key)?;
    let values = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(IpFilter::parse)
        .collect::<AnyResult<Vec<_>>>();
    Some(values)
}
