//! Distroless images have no shell, so every operation is a command of the binary.

use std::io::Write;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, bail, ensure};
use config::Config;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::{admin, serve};

const USAGE: &str = "Usage: accit [command]

  serve         Portal and verification listeners (default).
  healthcheck   Exit 0 when the local verification listener answers /healthz.
  admin reload  Reload the Policy in the running daemon (as SIGHUP does).
  admin status  Active Grants, verifications per minute and denials.
  admin check [file]
                Validate a config without applying it (default: the running config):
                coverage, warnings, client CA expiry.

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

/// Runs here, not in the daemon: it validates a file and never applies it.
fn check(args: &[String]) -> Result<()> {
    let path = match args {
        [] => config::path_from_env(),
        [file] => PathBuf::from(file),
        _ => bail!("admin check takes at most one file\n\n{USAGE}"),
    };
    let report = crate::check::run(&path, SystemTime::now())?;
    std::io::stdout().write_all(report.text.as_bytes())?;
    ensure!(!report.failed, "check failed");

    Ok(())
}

/// Errors come back as the daemon's message and a non-zero exit.
async fn admin(args: &[String]) -> Result<()> {
    if let Some((command, rest)) = args.split_first()
        && command == "check"
    {
        return check(rest);
    }

    let command = match args {
        [command] if matches!(command.as_str(), "reload" | "status") => command,
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
    let (command, rest) =
        args.split_first().map_or(("serve", &[][..]), |(command, rest)| (command.as_str(), rest));

    match command {
        "serve" => serve().await,
        "admin" => admin(rest).await,
        "healthcheck" => healthcheck(&Config::load(&config::path_from_env())?).await,
        "-h" | "--help" | "help" => {
            help();
            Ok(())
        }
        _ => bail!("unknown command {command:?}\n\n{USAGE}"),
    }
}
