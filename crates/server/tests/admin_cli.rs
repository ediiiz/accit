//! The `accit` binary end to end: `accit serve` opens the admin socket, `accit admin reload` and
//! SIGHUP reload the Policy through it (docs/spec.md sections 4 and 11).

#![allow(clippy::unwrap_used, reason = "test fixtures: a failure here is a test bug")]

use std::fs::File;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

const SELECTED: &str = "  mode: selected\n  hosts: [jellyfin.example.com]\n";
const WAIT: Duration = Duration::from_secs(20);

fn accit() -> Command {
    Command::new(env!("CARGO_BIN_EXE_accit"))
}

struct Daemon {
    dir: TempDir,
    child: Child,
}

impl Daemon {
    fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        write_config(&dir, SELECTED);
        let log = File::create(dir.path().join("daemon.log")).unwrap();
        let child = accit()
            .arg("serve")
            .env("ACCIT_CONFIG", dir.path().join("accit.yaml"))
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .unwrap();
        let daemon = Self { dir, child };
        daemon.wait_for(|daemon| daemon.socket().exists());

        daemon
    }

    fn config(&self) -> PathBuf {
        self.dir.path().join("accit.yaml")
    }

    fn socket(&self) -> PathBuf {
        self.dir.path().join("admin.sock")
    }

    fn log_path(&self) -> PathBuf {
        self.dir.path().join("daemon.log")
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.log_path()).unwrap()
    }

    fn write(&self, protection: &str) {
        write_config(&self.dir, protection);
    }

    fn admin(&self, command: &str) -> Output {
        accit().args(["admin", command]).env("ACCIT_CONFIG", self.config()).output().unwrap()
    }

    fn wait_for(&self, ready: impl Fn(&Self) -> bool) {
        let start = Instant::now();

        while !ready(self) {
            assert!(start.elapsed() < WAIT, "timed out; daemon log:\n{}", self.log());
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn write_config(dir: &TempDir, protection: &str) {
    let yaml = format!(
        "profile: test
listen:
  portal: 127.0.0.1:0
  verify: 127.0.0.1:0
admin_socket: {}
protection:
{protection}",
        dir.path().join("admin.sock").display()
    );
    std::fs::write(dir.path().join("accit.yaml"), yaml).unwrap();
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn help_lists_admin_reload() {
    let output = accit().arg("--help").output().unwrap();
    let help = format!("{}{}", text(&output.stdout), text(&output.stderr));

    assert!(output.status.success());
    assert!(help.contains("admin reload"), "{help}");
}

#[test]
fn serve_opens_an_owner_only_admin_socket() {
    let daemon = Daemon::start();

    assert_eq!(std::fs::metadata(daemon.socket()).unwrap().permissions().mode() & 0o777, 0o600);
}

#[test]
fn admin_reload_reports_the_daemons_answer_and_its_errors() {
    let daemon = Daemon::start();

    daemon.write("  mode: all\n");
    let reloaded = daemon.admin("reload");
    assert!(reloaded.status.success(), "{}", text(&reloaded.stderr));
    assert_eq!(text(&reloaded.stdout), "policy reloaded\n");

    daemon.write("  mode: selected\n");
    let reloaded = daemon.admin("reload");
    assert!(reloaded.status.success(), "{}", text(&reloaded.stderr));
    assert!(text(&reloaded.stdout).contains("no host requires a Grant"), "{}", text(&reloaded.stdout));

    daemon.write("  mode: everything\n");
    let refused = daemon.admin("reload");
    assert!(!refused.status.success());
    assert!(text(&refused.stderr).contains("accit.yaml"), "{}", text(&refused.stderr));
}

#[test]
fn sighup_runs_the_same_reload() {
    let daemon = Daemon::start();
    daemon.write("  mode: selected\n");

    let pid = daemon.child.id().to_string();
    assert!(Command::new("kill").args(["-HUP", &pid]).status().unwrap().success());

    daemon.wait_for(|daemon| daemon.log().contains("no host requires a Grant"));
    assert!(daemon.log().contains("policy reloaded"));
}
