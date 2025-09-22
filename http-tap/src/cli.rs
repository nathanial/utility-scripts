use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

use clap::{Parser, ValueHint};

#[derive(Debug, Clone, Parser)]
#[command(
    name = "http-tap",
    about = "Listen on a port and proxy to a target, printing HTTP requests/responses.",
    version,
    propagate_version = true
)]
pub struct Cli {
    /// Address to listen on (e.g., 127.0.0.1:8888)
    #[arg(long, value_hint = ValueHint::Other, default_value = "127.0.0.1:8888")]
    pub listen: String,

    /// Target HTTP endpoint to forward to (host:port or full URL base)
    #[arg(long, value_hint = ValueHint::Url, required = true)]
    pub target: String,

    /// Print request/response bodies (truncated by --max-body-bytes)
    #[arg(long, default_value_t = false)]
    pub include_bodies: bool,

    /// Maximum number of body bytes to print per message
    #[arg(long, default_value_t = 2048)]
    pub max_body_bytes: usize,

    /// Header names to redact in logs (repeatable)
    #[arg(long, value_delimiter = ',', num_args = 0.., default_values_t = vec![
        String::from("authorization"),
        String::from("cookie"),
        String::from("set-cookie"),
    ])]
    pub redact_header: Vec<String>,

    /// Enable TLS on the listening port using the provided cert (PEM) and key (PEM)
    #[arg(long, value_hint = ValueHint::FilePath)]
    pub listen_tls_cert: Option<PathBuf>,

    /// Private key for --listen-tls-cert (PEM, RSA or ECDSA)
    #[arg(long, value_hint = ValueHint::FilePath)]
    pub listen_tls_key: Option<PathBuf>,

    /// Disable TLS certificate and hostname verification for upstream HTTPS
    #[arg(long, short = 'k', default_value_t = false)]
    pub insecure_upstream: bool,

    /// Generate and use an in-memory self-signed cert for HTTPS listening (dev only)
    #[arg(long, default_value_t = false)]
    pub listen_self_signed: bool,

    /// Run an interactive TUI that shows a live table of paths, method counts, and last-seen
    #[arg(long, default_value_t = false)]
    pub tui: bool,

    /// Extra CA bundle(s) for upstream TLS verification (PEM). Comma-separated or repeatable.
    #[arg(long, value_hint = ValueHint::FilePath, value_delimiter = ',', num_args = 0..)]
    pub upstream_ca: Vec<PathBuf>,

    /// Upstream client certificate (PEM) for mTLS
    #[arg(long, value_hint = ValueHint::FilePath)]
    pub upstream_client_cert: Option<PathBuf>,

    /// Upstream client private key (PEM) for mTLS
    #[arg(long, value_hint = ValueHint::FilePath)]
    pub upstream_client_key: Option<PathBuf>,

    /// Override SNI/hostname for upstream TLS (useful when targeting an IP)
    #[arg(long)]
    pub upstream_server_name: Option<String>,

    /// Override the Host header sent to the upstream (virtual host routing)
    #[arg(long)]
    pub upstream_host: Option<String>,
}

impl Cli {
    pub fn listen_addr(&self) -> anyhow::Result<SocketAddr> {
        SocketAddr::from_str(&self.listen)
            .map_err(|e| anyhow::anyhow!("invalid --listen address '{}': {}", self.listen, e))
    }
}
