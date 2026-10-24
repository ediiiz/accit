//! Distroless images have no shell, so every operation is a command of the binary.

use std::io::Write;
use std::net::Ipv4Addr;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use config::Config;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::{admin, serve};

const USAGE: &str = "Usage: accit [command]

  serve         Portal and verification listeners (default).
  healthcheck   Exit 0 when the local verification listener answers /healthz.
  admin reload  Reload the Policy in the running daemon (as SIGHUP does).

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

async fn serve() -> Result<()> {
    let path = config::path_from_env();

    serve::run(Config::load(&path)?, &path).await
}

/// Errors come back as the daemon's message and a non-zero exit.
async fn admin(args: &[String]) -> Result<()> {
    let command = match args {
        [command] if command == "reload" => command,
        _ => bail!("unknown admin command {args:?}\n\n{USAGE}"),
    };
    let path = config::path_from_env();
    let yaml = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let socket = config::admin_socket(&yaml)
        .with_context(|| format!("in {}", path.display()))?
        .with_context(|| format!("no admin_socket or database in {}", path.display()))?;
    let answer = admin::request(&socket, command).await?;

    Ok(std::io::stdout().write_all(answer.as_bytes())?)
}

fn help() {
    tracing::info!("{USAGE}");
}

pub async fn run(args: &[String]) -> Result<()> {
    let command = args.first().map_or("serve", String::as_str);

    match command {
        "serve" => serve().await,
        "admin" => admin(&args[1..]).await,
        "healthcheck" => healthcheck(&Config::load(&config::path_from_env())?).await,
        "-h" | "--help" | "help" => {
            help();
            Ok(())
        }
        _ => bail!("unknown command {command:?}\n\n{USAGE}"),
    }
}
