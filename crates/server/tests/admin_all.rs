//! Every `accit admin` command against one running daemon over its socket (docs/spec.md section 11).
//! The daemon starts failed closed with two stored Grants, so `recover` is a real step of the run.

#![allow(clippy::unwrap_used, reason = "test fixtures: a failure here is a test bug")]

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant, SystemTime};

use store::{Grants, Limits};
use tempfile::TempDir;

const WAIT: Duration = Duration::from_secs(20);
const FIRST: &str = "81.2.69.160";
const SECOND: &str = "93.184.216.34";
const UNKNOWN: &str = "81.2.69.161";
const HOST: &str = "photos.example.com";

struct Daemon {
    dir: TempDir,
    verify: SocketAddr,
    child: Child,
}

fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap().local_addr().unwrap().port()
}

fn config(dir: &Path, verify: SocketAddr, protection: &str) -> String {
    format!(
        "profile: test
listen:
  portal: 127.0.0.1:{}
  verify: {verify}
database: {}
admin_socket: {}
protection:
{protection}",
        free_port(),
        dir.join("accit.db").display(),
        dir.join("admin.sock").display(),
    )
}

/// Two Grants in the database and the fail-closed marker beside it, as a failed Revoke leaves them.
async fn seed(dir: &Path) {
    let limits = Limits {
        grant_duration: Duration::from_hours(7 * 24),
        max_grants: 1_000,
        audit_retention: Duration::from_hours(7 * 24),
    };
    let now = SystemTime::now();
    let grants = Grants::open(Some(&dir.join("accit.db")), limits, now).await.unwrap();

    for address in [FIRST, SECOND] {
        grants.unlock(address.parse().unwrap(), now).await.unwrap();
    }
    std::fs::write(dir.join(store::FAIL_CLOSED_MARKER), "").unwrap();
}

impl Daemon {
    async fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let verify = SocketAddr::from((Ipv4Addr::LOCALHOST, free_port()));
        seed(dir.path()).await;
        std::fs::write(dir.path().join("accit.yaml"), config(dir.path(), verify, "  mode: all\n")).unwrap();
        let log = std::fs::File::create(dir.path().join("daemon.log")).unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_accit"))
            .arg("serve")
            .env("ACCIT_CONFIG", dir.path().join("accit.yaml"))
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .unwrap();
        let daemon = Self { dir, verify, child };
        daemon.wait_for(|daemon| daemon.get("/readyz", &[]).is_some());

        daemon
    }

    fn config(&self) -> PathBuf {
        self.dir.path().join("accit.yaml")
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("daemon.log")).unwrap()
    }

    fn wait_for(&self, ready: impl Fn(&Self) -> bool) {
        let start = Instant::now();

        while !ready(self) {
            assert!(start.elapsed() < WAIT, "timed out; daemon log:\n{}", self.log());
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The status of a GET on the verification listener; `None` while it does not answer.
    fn get(&self, path: &str, headers: &[(&str, &str)]) -> Option<u16> {
        let head = [format!("GET {path} HTTP/1.1"), "Host: accit".into(), "Connection: close".into()];
        let lines: Vec<String> =
            head.into_iter().chain(headers.iter().map(|(name, value)| format!("{name}: {value}"))).collect();
        let request = lines.join("\r\n");
        let mut stream = TcpStream::connect(self.verify).ok()?;
        stream.set_read_timeout(Some(WAIT)).ok()?;
        stream.write_all(format!("{request}\r\n\r\n").as_bytes()).ok()?;
        let mut answer = String::new();
        stream.read_to_string(&mut answer).ok()?;

        answer.split_whitespace().nth(1)?.parse().ok()
    }

    fn verify(&self, client: &str) -> u16 {
        self.get("/verify", &[("x-forwarded-for", client), ("x-forwarded-host", HOST)]).unwrap()
    }

    fn admin(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_accit"))
            .arg("admin")
            .args(args)
            .env("ACCIT_CONFIG", self.config())
            .output()
            .unwrap()
    }

    /// stdout of a command that must succeed.
    fn run(&self, args: &[&str]) -> String {
        let output = self.admin(args);

        assert!(output.status.success(), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    fn listed(&self) -> Vec<String> {
        let list = self.run(&["list"]);

        list.lines().filter_map(|line| line.split_whitespace().next()).map(str::to_owned).collect()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn recover_from_the_failed_closed_start(daemon: &Daemon) {
    assert_eq!(daemon.get("/readyz", &[]), Some(503));
    assert_eq!(daemon.verify(FIRST), 503);

    assert_eq!(daemon.run(&["recover", "--keep-grants"]), "recovered\n");

    daemon.wait_for(|daemon| daemon.get("/readyz", &[]) == Some(200));
    let healthy = daemon.admin(&["recover", "--discard-grants"]);
    assert!(!healthy.status.success());
    assert!(String::from_utf8_lossy(&healthy.stderr).contains("nothing to recover"));
}

fn grants_decide_and_status_counts(daemon: &Daemon) {
    assert_eq!(daemon.listed(), [FIRST, SECOND]);

    assert_eq!([daemon.verify(FIRST), daemon.verify(UNKNOWN), daemon.verify(SECOND)], [204, 403, 204]);

    assert_eq!(
        daemon.run(&["status"]),
        "active grants: 2\nverifications per minute: 3\ndenials since start: 1\n"
    );
}

fn revoke_one_then_all(daemon: &Daemon) {
    assert_eq!(daemon.run(&["revoke", FIRST]), format!("revoked {FIRST}\n"));
    assert_eq!(daemon.verify(FIRST), 403);
    assert_eq!(daemon.listed(), [SECOND]);

    assert_eq!(daemon.run(&["revoke-all"]), "revoked: 1\n");
    assert_eq!(daemon.verify(SECOND), 403);
    assert_eq!(daemon.listed(), Vec::<String>::new());
}

fn reload_and_check(daemon: &Daemon) {
    let verify = daemon.verify;
    assert!(daemon.run(&["check"]).contains("other hosts  gated"));

    let selected = config(daemon.dir.path(), verify, "  mode: selected\n  hosts: [jellyfin.example.com]\n");
    let candidate = daemon.dir.path().join("candidate.yaml");
    std::fs::write(&candidate, &selected).unwrap();
    let report = daemon.run(&["check", &candidate.display().to_string()]);
    assert!(report.contains("jellyfin.example.com  gated"), "{report}");
    assert_eq!(daemon.verify(UNKNOWN), 403, "check applies nothing");

    std::fs::write(daemon.config(), selected).unwrap();
    assert!(daemon.run(&["reload"]).starts_with("policy reloaded\n"));
    assert_eq!(daemon.verify(UNKNOWN), 204);
    assert!(daemon.run(&["check"]).contains("jellyfin.example.com  gated"));
}

#[tokio::test]
async fn every_admin_command_works_against_one_daemon() {
    let daemon = Daemon::start().await;

    recover_from_the_failed_closed_start(&daemon);
    grants_decide_and_status_counts(&daemon);
    revoke_one_then_all(&daemon);
    reload_and_check(&daemon);
}
