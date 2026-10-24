//! The running Compose harness: bring accit back to a known state, change its config, reach into
//! containers.

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};

use crate::reply::wget_status;

const ACCIT_VERIFY: &str = "http://10.89.20.3:8081/verify";
const CONFLICT: &str = "conflict.yml";

/// Scenarios share one stack and restart accit: one at a time, whatever `--test-threads` says.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// Handle on the harness started by `scripts/e2e`.
#[derive(Debug)]
pub struct Stack {
    harness: PathBuf,
}

/// A scenario's exclusive hold on a freshly reset stack.
#[derive(Debug)]
pub struct Session {
    pub stack: Stack,
    _exclusive: MutexGuard<'static, ()>,
}

impl Session {
    /// Waits for any other scenario, then resets accit and Traefik's routing to the base config.
    pub fn begin() -> Result<Self> {
        let exclusive = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
        let stack = Stack { harness: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../harness") };
        stack.reset()?;

        Ok(Self { stack, _exclusive: exclusive })
    }
}

fn run(command: &mut Command) -> Result<Output> {
    command.output().with_context(|| format!("running {command:?}"))
}

/// Polls `check` until it holds or `timeout` passes.
pub fn eventually(timeout: Duration, mut check: impl FnMut() -> Result<bool>) -> Result<bool> {
    let deadline = Instant::now() + timeout;

    loop {
        if check()? {
            return Ok(true);
        }

        if Instant::now() >= deadline {
            return Ok(false);
        }

        sleep(Duration::from_millis(500));
    }
}

impl Stack {
    fn compose(&self) -> Command {
        let mut command = Command::new("docker");
        command.args(["compose", "-f"]).arg(self.harness.join("compose.yml"));
        command
    }

    /// `docker compose exec` without a TTY; the exit status is the command's, so callers decide.
    pub(crate) fn exec(&self, service: &str, args: &[String]) -> Result<Output> {
        run(self.compose().args(["exec", "-T", service]).args(args))
    }

    fn run_dir(&self) -> PathBuf {
        self.harness.join("run")
    }

    /// `accit.yaml` as the harness ships it.
    pub fn base_config(&self) -> Result<String> {
        let path = self.harness.join("accit.yaml");

        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))
    }

    /// Replaces the config accit reads; takes effect on `reload`.
    pub fn set_config(&self, yaml: &str) -> Result<()> {
        let path = self.run_dir().join("accit/accit.yaml");

        std::fs::write(&path, yaml).with_context(|| format!("writing {}", path.display()))
    }

    /// `accit admin reload`, the same code as SIGHUP.
    pub fn reload(&self) -> Result<()> {
        let output = self.exec("accit", &["/accit".into(), "admin".into(), "reload".into()])?;

        ensure!(output.status.success(), "admin reload failed: {}", String::from_utf8_lossy(&output.stderr));

        Ok(())
    }

    pub fn stop_accit(&self) -> Result<()> {
        let output = run(self.compose().args(["stop", "accit"]))?;

        ensure!(output.status.success(), "stopping accit failed");

        Ok(())
    }

    /// The secret Traefik sets in `X-Accit-Proxy-Token`.
    pub fn token(&self) -> Result<String> {
        let path = self.run_dir().join("accit/proxy-token");

        Ok(std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?
            .trim()
            .to_owned())
    }

    /// Adds a dynamic config file next to the chain; Traefik's file watcher picks it up.
    pub fn add_conflicting_router(&self) -> Result<()> {
        let path = self.run_dir().join("dynamic").join(CONFLICT);
        // Same host as the Portal, default TLS options: no client certificate required.
        let yaml = "http:\n  routers:\n    conflict:\n      rule: Host(`portal.accit.test`)\n      entryPoints: [websecure]\n      service: whoami-open\n      tls: {}\n";

        std::fs::write(&path, yaml).with_context(|| format!("writing {}", path.display()))
    }

    fn remove_conflicting_router(&self) -> Result<()> {
        let path = self.run_dir().join("dynamic").join(CONFLICT);

        match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                Err(error).with_context(|| format!("removing {}", path.display()))
            }
            _ => Ok(()),
        }
    }

    /// A request to accit's verification listener from inside the Traefik container, so the peer
    /// is Traefik's pinned address. `None` when no HTTP status came back.
    pub fn verify_as_traefik(&self, headers: &[(&str, &str)]) -> Result<Option<u16>> {
        let mut args: Vec<String> = ["wget", "-S", "-q", "-O", "/dev/null"].map(String::from).into();

        for (name, value) in headers {
            args.push("--header".into());
            args.push(format!("{name}: {value}"));
        }

        args.push(ACCIT_VERIFY.into());
        let output = self.exec("traefik", &args)?;

        Ok(wget_status(&String::from_utf8_lossy(&output.stderr)))
    }

    /// A router as Traefik's API reports it (JSON), read on the internal `traefik` entrypoint;
    /// `None` while Traefik does not know it.
    pub fn router(&self, name: &str) -> Result<Option<String>> {
        let url = format!("http://127.0.0.1:8080/api/http/routers/{name}");
        let args = ["wget", "-q", "-O", "-", &url].map(String::from);
        let output = self.exec("traefik", &args)?;

        Ok(output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned()))
    }

    /// Base config, no conflicting router, a fresh accit, Portal reachable.
    fn reset(&self) -> Result<()> {
        self.remove_conflicting_router()?;
        self.set_config(&self.base_config()?)?;
        let up = run(self.compose().args([
            "up",
            "-d",
            "--wait",
            "--force-recreate",
            "--no-deps",
            "--no-build",
            "accit",
        ]))?;

        ensure!(up.status.success(), "starting accit failed: {}", String::from_utf8_lossy(&up.stderr));

        let ready = eventually(Duration::from_secs(30), || {
            Ok(self.call(crate::ADDRESS_A, crate::PORTAL, "/api/status").with_certificate().send()?.served())
        })?;

        if !ready {
            bail!("the Portal never became reachable through Traefik; see `docker compose logs`");
        }

        Ok(())
    }
}
