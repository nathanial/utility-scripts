mod cli;
mod proxy;
mod stats;
mod tui;

use anyhow::Result;
use clap::Parser;
use cli::Cli;
use proxy::{run_proxy, Config, TlsConfig};
use stats::channel as stats_channel;
use std::fs::File;
use std::io::BufReader;
use rustls::{pki_types::CertificateDer, pki_types::PrivateKeyDer, ServerConfig};
use tokio_rustls::TlsAcceptor;
use rustls_pemfile::{certs, pkcs8_private_keys, rsa_private_keys};

fn normalize_target(target: &str) -> (String, &'static str) {
    // Accept host:port or full http(s)://host[:port]
    if let Some(rest) = target.strip_prefix("http://") {
        (rest.to_string(), "http")
    } else if let Some(rest) = target.strip_prefix("https://") {
        (rest.to_string(), "https")
    } else if let Some((host, port_str)) = target.rsplit_once(':') {
        if let Ok(port) = port_str.parse::<u16>() {
            let scheme = if port == 443 { "https" } else { "http" };
            (format!("{}:{}", host, port), scheme)
        } else {
            (target.to_string(), "http")
        }
    } else {
        (target.to_string(), "http")
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let listen = cli.listen_addr()?;
    let (authority, scheme) = normalize_target(&cli.target);

    let tls_acceptor = if cli.listen_tls_cert.is_some() || cli.listen_tls_key.is_some() {
        Some(build_tls_acceptor(&cli)? )
    } else { None };

    let (stats_tx, stats_rx) = if cli.tui { let (tx, rx) = stats_channel(); (Some(tx), Some(rx)) } else { (None, None) };

    let cfg = Config {
        listen,
        target_authority: authority,
        target_scheme: if scheme == "https" { "https" } else { "http" },
        include_bodies: cli.include_bodies,
        max_body_bytes: cli.max_body_bytes,
        redact_header: cli.redact_header,
        tls: tls_acceptor.map(|a| TlsConfig { acceptor: a }),
        insecure_upstream: cli.insecure_upstream,
        stats: stats_tx,
    };

    if let Some(rx) = stats_rx {
        // Run proxy in background and TUI in foreground
        let proxy_task = tokio::spawn(async move { let _ = run_proxy(cfg).await; });
        tui::run_tui(rx).await?;
        // TUI exited; proxy task ends when process exits
        drop(proxy_task);
        Ok(())
    } else {
        run_proxy(cfg).await?;
        Ok(())
    }
}

fn build_tls_acceptor(cli: &Cli) -> Result<TlsAcceptor> {
    let cert_path = cli
        .listen_tls_cert
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("--listen-tls-cert is required when enabling TLS"))?;
    let key_path = cli
        .listen_tls_key
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("--listen-tls-key is required when enabling TLS"))?;

    let mut cert_reader = BufReader::new(File::open(cert_path)?);
    let mut key_reader = BufReader::new(File::open(key_path)?);

    let certs_der: Vec<CertificateDer> = certs(&mut cert_reader)
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .map(CertificateDer::from)
        .collect();

    if certs_der.is_empty() {
        anyhow::bail!("no certificates found in {}", cert_path.display());
    }

    // Try pkcs8 first, then RSA
    let mut keys: Vec<PrivateKeyDer> = pkcs8_private_keys(&mut key_reader)
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .map(PrivateKeyDer::from)
        .collect();

    if keys.is_empty() {
        // rewind and try RSA
        key_reader = BufReader::new(File::open(key_path)?);
        keys = rsa_private_keys(&mut key_reader)
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(PrivateKeyDer::from)
            .collect();
    }

    let Some(key_der) = keys.into_iter().next() else {
        anyhow::bail!("no private keys found in {}", key_path.display());
    };

    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs_der, key_der)?;

    Ok(TlsAcceptor::from(std::sync::Arc::new(server_config)))
}
