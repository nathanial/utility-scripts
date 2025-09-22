mod cli;
mod proxy;

use anyhow::Result;
use clap::Parser;
use cli::Cli;
use proxy::{run_proxy, Config};

fn normalize_target(target: &str) -> (String, &'static str) {
    // Accept host:port or full http://host:port
    if let Some(rest) = target.strip_prefix("http://") {
        (rest.to_string(), "http")
    } else if let Some(rest) = target.strip_prefix("https://") {
        // We don't support TLS interception; still allow explicit https target to fail clearly.
        (rest.to_string(), "https")
    } else {
        (target.to_string(), "http")
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let listen = cli.listen_addr()?;
    let (authority, scheme) = normalize_target(&cli.target);

    if scheme != "http" {
        eprintln!("Warning: TLS interception is not supported. Target should be http://…; got {}.", scheme);
    }

    let cfg = Config {
        listen,
        target_authority: authority,
        target_scheme: "http",
        include_bodies: cli.include_bodies,
        max_body_bytes: cli.max_body_bytes,
        redact_header: cli.redact_header,
    };

    run_proxy(cfg).await?;
    Ok(())
}

