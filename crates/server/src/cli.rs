//! Distroless images have no shell, so every operation is a command of the binary.

use std::net::Ipv4Addr;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use config::Config;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::serve;

const USAGE: &str = "Usage: accit [command]

  serve         Portal and verification listeners (default).
  healthcheck   Exit 0 when the local verification listener answers /healthz.

Configuration: $ACCIT_CONFIG, else /config/accit.yaml.";

async fn healthcheck(config: &Config) -> Result<()> {
    let mut address = config.listen.verify;

    if address.ip().is_unspecified() {
        address.set_ip(Ipv4Addr::LOCALHOST.into());
    }

    let mut stream = tokio::time::timeout(Duration::from_secs(3), TcpStream::connect(address))
        .await
        .context("healthcheck timed out")??;
    stream.write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").await?;

    let mut status = [0_u8; 12];
    stream.read_exact(&mut status).await?;

    if &status != b"HTTP/1.1 200" {
        bail!("unhealthy: {}", String::from_utf8_lossy(&status));
    }

    Ok(())
}

pub async fn run(args: &[String]) -> Result<()> {
    let command = args.first().map_or("serve", String::as_str);

    match command {
        "serve" => serve::run(Config::load(&config::path_from_env())?).await,
        "healthcheck" => healthcheck(&Config::load(&config::path_from_env())?).await,
        "-h" | "--help" | "help" => {
            tracing::info!("{USAGE}");
            Ok(())
        }
        _ => bail!("unknown command {command:?}\n\n{USAGE}"),
    }
}
