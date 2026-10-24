//! Administration (docs/spec.md section 11): `accit admin <command>` talks to the daemon over a
//! Unix socket only the daemon's user can open.

use std::fs::{DirBuilder, Permissions};
use std::io::ErrorKind;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::signal::unix::{Signal, SignalKind, signal};

use crate::verify::Verifier;

mod grants;

pub(crate) use grants::GrantCommand;

const OWNER_ONLY: u32 = 0o600;
const OWNER_ONLY_DIR: u32 = 0o700;
const MAX_REQUEST: u64 = 4096;
const TIMEOUT: Duration = Duration::from_secs(10);
const OK: &str = "ok";
const ERROR: &str = "error";

/// Removes a socket left by a previous run; anything else at `path` is a misconfiguration.
fn remove_stale(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => {
            std::fs::remove_file(path).with_context(|| format!("removing stale {}", path.display()))
        }
        Ok(_) => bail!("admin_socket {} exists and is not a socket", path.display()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("inspecting {}", path.display())),
    }
}

/// Binds inside a fresh 0700 directory and moves the socket into place at 0600, so whatever the
/// umask, `path` never names a socket anyone else can connect to.
pub fn bind(path: &Path) -> Result<UnixListener> {
    remove_stale(path)?;
    let name =
        path.file_name().with_context(|| format!("admin_socket {} has no file name", path.display()))?;
    let staging = path.with_file_name(format!(".{}.staging", name.to_string_lossy()));
    // Left behind only by a crash during a previous bind.
    if std::fs::symlink_metadata(&staging).is_ok() {
        std::fs::remove_dir_all(&staging).with_context(|| format!("removing stale {}", staging.display()))?;
    }
    DirBuilder::new()
        .mode(OWNER_ONLY_DIR)
        .create(&staging)
        .with_context(|| format!("creating {}", staging.display()))?;
    let bound = bind_staged(&staging.join("admin.sock"), path);
    let removed =
        std::fs::remove_dir_all(&staging).with_context(|| format!("removing {}", staging.display()));

    bound.and_then(|listener| removed.map(|()| listener))
}

fn bind_staged(staged: &Path, path: &Path) -> Result<UnixListener> {
    let listener = UnixListener::bind(staged).with_context(|| format!("binding {}", staged.display()))?;
    std::fs::set_permissions(staged, Permissions::from_mode(OWNER_ONLY))
        .with_context(|| format!("restricting {}", staged.display()))?;
    std::fs::rename(staged, path).with_context(|| format!("moving the socket to {}", path.display()))?;

    Ok(listener)
}

/// One request line, `<command> [arguments]`, answered by `ok` or `error` on the first line and
/// the text for the operator after it.
#[derive(Clone, Debug)]
struct Daemon {
    verifier: Verifier,
    config: PathBuf,
}

impl Daemon {
    async fn answer(&self, request: &str) -> Result<String> {
        if let Some(command) = GrantCommand::parse(request)? {
            return command.run(self.verifier.grants()).await;
        }
        match request {
            "reload" => reload(&self.verifier, &self.config)
                .map(|warning| format!("policy reloaded\n{}", warning.map(warning_line).unwrap_or_default())),
            _ => bail!("unknown command {request:?}"),
        }
    }

    async fn handle(self, stream: UnixStream) {
        let (read, mut write) = stream.into_split();
        let mut line = String::new();
        let mut reader = BufReader::new(read.take(MAX_REQUEST));
        let answer = match tokio::time::timeout(TIMEOUT, reader.read_line(&mut line)).await {
            Ok(Ok(_)) => self.answer(line.trim_end()).await,
            _ => Err(anyhow::anyhow!("unreadable request")),
        };
        let encoded = match answer {
            Ok(text) => format!("{OK}\n{text}"),
            Err(error) => format!("{ERROR}\n{error:#}\n"),
        };

        let _ = tokio::time::timeout(TIMEOUT, write.write_all(encoded.as_bytes())).await;
    }
}

fn warning_line(warning: &str) -> String {
    format!("warning: {warning}\n")
}

/// The one reload path, for the socket and for SIGHUP.
fn reload(verifier: &Verifier, config: &Path) -> Result<Option<&'static str>> {
    verifier.reload(config).inspect_err(|error| tracing::error!("reload failed: {error:#}"))
}

async fn reload_on_hangup(mut hangup: Signal, verifier: Verifier, config: PathBuf) {
    while hangup.recv().await.is_some() {
        let _ = reload(&verifier, &config);
    }
}

/// Install first thing: until then SIGHUP ends the process. Signals that arrive before `start`
/// are kept and reload then.
pub fn hangup() -> Result<Signal> {
    signal(SignalKind::hangup()).context("installing the SIGHUP handler")
}

/// SIGHUP and the socket at `socket` both reload from `config`. Both are in place on return.
pub fn start(hangup: Signal, socket: Option<&Path>, verifier: &Verifier, config: &Path) -> Result<()> {
    tokio::spawn(reload_on_hangup(hangup, verifier.clone(), config.to_owned()));

    if let Some(path) = socket {
        tokio::spawn(serve(bind(path)?, verifier.clone(), config.to_owned()));
    } else {
        tracing::warn!("no admin_socket or database configured; accit admin is unavailable");
    }

    Ok(())
}

/// Answers admin requests until the process ends.
pub async fn serve(listener: UnixListener, verifier: Verifier, config: PathBuf) {
    let daemon = Daemon { verifier, config };

    loop {
        match listener.accept().await {
            Ok((stream, _)) => drop(tokio::spawn(daemon.clone().handle(stream))),
            Err(error) => tracing::warn!(%error, "admin socket accept failed"),
        }
    }
}

/// The CLI side: sends `command` and returns the daemon's text, or its error.
pub async fn request(socket: &Path, command: &str) -> Result<String> {
    let exchange = async {
        let mut stream = UnixStream::connect(socket)
            .await
            .with_context(|| format!("connecting to {}", socket.display()))?;
        stream.write_all(format!("{command}\n").as_bytes()).await?;
        stream.shutdown().await?;
        let mut answer = String::new();
        stream.read_to_string(&mut answer).await?;

        anyhow::Ok(answer)
    };
    let answer = tokio::time::timeout(TIMEOUT, exchange).await.context("admin socket timed out")??;

    match answer.split_once('\n') {
        Some((OK, text)) => Ok(text.to_owned()),
        Some((ERROR, message)) => bail!("{}", message.trim_end()),
        _ => bail!("malformed answer from the daemon"),
    }
}
