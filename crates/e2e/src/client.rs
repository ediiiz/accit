//! Requests as a client of the harness: `curl` run inside the client container, which owns the
//! fixed source addresses (`curl --interface`).

use anyhow::{Context, Result, ensure};

use crate::PORTAL;
use crate::reply::Reply;
use crate::stack::Stack;

const ORIGIN: &str = "https://portal.accit.test";

/// What came back: an HTTP response, or no response at all (TLS handshake refused, connection reset).
#[derive(Debug)]
pub enum Outcome {
    Reply(Reply),
    Failed(String),
}

impl Outcome {
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Reply(reply) => Some(reply.status),
            Self::Failed(_) => None,
        }
    }

    /// A 2xx: the request got through.
    pub fn served(&self) -> bool {
        self.status().is_some_and(|status| (200..300).contains(&status))
    }

    /// A whoami app answered: the request passed the gate and reached the upstream.
    pub fn reached_app(&self) -> bool {
        self.served() && self.body().contains("Hostname:")
    }

    pub fn body(&self) -> &str {
        match self {
            Self::Reply(reply) => &reply.body,
            Self::Failed(_) => "",
        }
    }
}

/// One request. Anonymous (no client certificate) unless asked otherwise.
#[derive(Debug)]
#[must_use]
pub struct Call<'a> {
    stack: &'a Stack,
    from: &'a str,
    host: &'a str,
    path: &'a str,
    certificate: bool,
    headers: Vec<String>,
    post: bool,
}

impl Stack {
    /// A GET of `https://<host><path>` from the source address `from`.
    pub fn call<'a>(&'a self, from: &'a str, host: &'a str, path: &'a str) -> Call<'a> {
        Call { stack: self, from, host, path, certificate: false, headers: Vec::new(), post: false }
    }

    /// What the Portal does for a browser: read the status for its session and CSRF token, then
    /// POST `/api/unlock` from `from`.
    pub fn unlock(&self, from: &str) -> Result<Outcome> {
        let status = self.call(from, PORTAL, "/api/status").with_certificate().send()?;
        let Outcome::Reply(status) = status else {
            anyhow::bail!("Portal status failed before Unlock");
        };
        let token = status.field("csrf_token").context("no csrf_token in the Portal status")?;

        self.call(from, PORTAL, "/api/unlock")
            .with_certificate()
            .header("Origin", ORIGIN)
            .header("Content-Type", "application/json")
            .header("X-CSRF-Token", &token)
            .post()
            .send()
    }
}

impl Call<'_> {
    pub fn with_certificate(mut self) -> Self {
        self.certificate = true;
        self
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push(format!("{name}: {value}"));
        self
    }

    /// POST with the body `{}`, the only one the Portal accepts.
    pub fn post(mut self) -> Self {
        self.post = true;
        self
    }

    fn curl_args(&self) -> Vec<String> {
        let jar = format!("/tmp/jar-{}", self.from);
        let mut args: Vec<String> =
            ["curl", "-sS", "-i", "--max-time", "10", "--cacert", "/certs/server-ca.pem"]
                .map(String::from)
                .into();

        args.extend(["--interface", self.from, "-b", &jar, "-c", &jar].map(String::from));

        if self.certificate {
            args.extend(["--cert", "/certs/client.pem", "--key", "/certs/client.key"].map(String::from));
        }

        for header in &self.headers {
            args.extend(["-H".to_owned(), header.clone()]);
        }

        if self.post {
            args.extend(["-X", "POST", "-d", "{}"].map(String::from));
        }

        args.push(format!("https://{}{}", self.host, self.path));
        args
    }

    pub fn send(self) -> Result<Outcome> {
        let output = self.stack.exec("client", &self.curl_args())?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        if output.status.success() {
            let reply = Reply::parse(&stdout).with_context(|| format!("not an HTTP response: {stdout}"))?;

            return Ok(Outcome::Reply(reply));
        }

        // `docker compose exec` itself failing (no such service) must not pass for a refused handshake.
        ensure!(
            output.status.code().is_some_and(|code| code != 125 && code != 126 && code != 127),
            "exec failed"
        );

        Ok(Outcome::Failed(String::from_utf8_lossy(&output.stderr).into_owned()))
    }
}
