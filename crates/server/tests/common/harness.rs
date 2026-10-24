//! A Portal as a browser behind Traefik sees it: proxy headers, a client Address, a cookie jar and
//! a clock the test moves.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test helpers: a broken fixture is a test bug")]

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use config::Config;
use server::health::{self, Readiness};
use server::portal::{self, Clock, Files, Portal};
use server::trust::ProxyTrust;
use store::{Grants, Limits};
use tempfile::TempDir;
use tower::ServiceExt;

use super::{Headers, PORTAL_HOST, PROXY, TOKEN, TOKEN_HEADER, header};

pub const CLIENT: &str = "93.184.216.34";
pub const ORIGIN: &str = "https://access.example.com";
pub const COOKIE: &str = "__Host-accit";
pub const DAY: Duration = Duration::from_hours(24);
pub const WEEK: Duration = Duration::from_hours(7 * 24);

const BASE: &str = "profile: production
public_origin: https://access.example.com
trusted_proxy: 172.30.0.2
proxy_token_file: /secrets/proxy-token
database: /data/accit.db
title: Door
default_language: de
apps:
  - name: Jellyfin
    url: https://jellyfin.example.com
protection:
  mode: selected
";

/// 2027-01-15T08:00:00Z.
pub fn t0() -> SystemTime {
    UNIX_EPOCH + Duration::from_hours(500_000)
}

pub fn address(text: &str) -> access::Address {
    text.parse().unwrap()
}

pub fn secs(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).unwrap().as_secs()
}

#[derive(Debug)]
pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: String,
}

impl Reply {
    async fn of(response: axum::response::Response) -> Self {
        let (parts, body) = response.into_parts();
        let bytes = axum::body::to_bytes(body, 1 << 20).await.unwrap();

        Self {
            status: parts.status,
            headers: parts.headers,
            body: String::from_utf8(bytes.to_vec()).unwrap(),
        }
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or_else(|_| panic!("not JSON: {}", self.body))
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(|value| value.to_str().unwrap())
    }
}

pub struct Harness {
    app: Router,
    pub grants: Grants,
    pub readiness: Readiness,
    pub database: std::path::PathBuf,
    _dir: TempDir,
    clock: Arc<AtomicU64>,
}

impl Harness {
    pub async fn start() -> Self {
        Self::with(BASE).await
    }

    /// `yaml` replaces the default configuration.
    pub async fn with(yaml: &str) -> Self {
        Self::build(yaml, None).await
    }

    /// The default configuration with `files` as the built web app.
    pub async fn serving(files: Files) -> Self {
        Self::build(BASE, Some(files)).await
    }

    async fn build(yaml: &str, files: Option<Files>) -> Self {
        let config = Config::parse(yaml, |_| Ok(TOKEN.to_owned())).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("accit.db");
        let limits = Limits {
            grant_duration: config.grant_duration,
            max_grants: config.max_grants,
            audit_retention: config.audit_retention,
        };
        let grants = Grants::open(Some(&database), limits, t0()).await.unwrap();
        let readiness = Readiness::default();
        readiness.set(true);
        tokio::spawn(health::follow(readiness.clone(), grants.integrity()));
        let seconds = Arc::new(AtomicU64::new(secs(t0())));
        let clock = Clock::new({
            let seconds = Arc::clone(&seconds);
            move || UNIX_EPOCH + Duration::from_secs(seconds.load(Ordering::SeqCst))
        });
        let gate = server::verify::Verifier::new(&config, grants.clone());
        let portal = Portal::new(&config, grants.clone(), readiness.clone(), clock, &gate);
        let trust = ProxyTrust::from_config(&config);
        let app = match files {
            Some(files) => portal::router_with(trust, portal, files),
            None => portal::router(trust, portal),
        };

        Self { app, grants, readiness, database, _dir: dir, clock: seconds }
    }

    /// A configuration like the default one with `replace` swapped for `with`.
    pub async fn replacing(replace: &str, with: &str) -> Self {
        assert!(BASE.contains(replace), "{replace}");

        Self::with(&BASE.replace(replace, with)).await
    }

    /// Lets the readiness follower react to what the store just reported.
    pub async fn settle(&self) {
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
    }

    pub fn now(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(self.clock.load(Ordering::SeqCst))
    }

    pub fn advance(&self, by: Duration) {
        self.clock.fetch_add(by.as_secs(), Ordering::SeqCst);
    }

    pub fn browser(&self) -> Browser<'_> {
        self.browser_at(CLIENT)
    }

    /// A browser whose Address, as Traefik reports it, is `address`.
    pub fn browser_at(&self, address: &str) -> Browser<'_> {
        Browser { harness: self, address: address.to_owned(), cookie: None, csrf: None }
    }

    /// `request` as the pinned proxy's connection delivers it.
    async fn send(&self, mut request: Request<Body>) -> Reply {
        let peer: SocketAddr = format!("{PROXY}:40000").parse().unwrap();
        request.extensions_mut().insert(ConnectInfo(peer));

        Reply::of(self.app.clone().oneshot(request).await.unwrap()).await
    }

    /// A request from the pinned proxy that lacks its token: refused before the Portal looks at it.
    pub async fn without_token(&self, path: &str) -> Reply {
        let request = Request::builder().uri(path).header("host", PORTAL_HOST);

        self.send(request.body(Body::empty()).unwrap()).await
    }

    /// Every operation on `table` (`INSERT`, `DELETE`) fails, as a broken disk would.
    pub async fn fail(&self, operation: &str, table: &str) {
        use sqlx::{Connection, SqliteConnection};

        let mut connection =
            SqliteConnection::connect(&format!("sqlite://{}", self.database.display())).await.unwrap();
        let trigger = format!(
            "CREATE TRIGGER fail_{operation}_{table} BEFORE {operation} ON {table}
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END"
        );

        sqlx::raw_sql(sqlx::AssertSqlSafe(trigger)).execute(&mut connection).await.unwrap();
    }
}

pub struct Browser<'a> {
    harness: &'a Harness,
    address: String,
    cookie: Option<String>,
    csrf: Option<String>,
}

impl Browser<'_> {
    /// Sends `method path` through Traefik: the pinned peer, the token, the Portal host and this
    /// browser's Address, plus `headers`.
    pub async fn request(&self, method: Method, path: &str, headers: Headers, body: &str) -> Reply {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("host", PORTAL_HOST)
            .header(TOKEN_HEADER, TOKEN)
            .header("x-forwarded-for", &self.address);

        for (name, value) in headers {
            request = request.header(name, value);
        }

        let mut request = request.body(Body::from(body.to_owned())).unwrap();
        let peer: SocketAddr = format!("{PROXY}:40000").parse().unwrap();
        request.extensions_mut().insert(ConnectInfo(peer));

        Reply::of(self.harness.app.clone().oneshot(request).await.unwrap()).await
    }

    /// `GET /api/status` with this browser's cookie; keeps the session cookie and CSRF token.
    pub async fn status(&mut self) -> Reply {
        let headers = self.cookie.iter().map(|cookie| header("cookie", cookie)).collect();
        let reply = self.request(Method::GET, "/api/status", headers, "").await;

        if let Some(set) = reply.header("set-cookie") {
            self.cookie = set.split(';').next().map(str::to_owned);
        }

        if reply.status == StatusCode::OK {
            self.csrf = reply.json()["csrf_token"].as_str().map(str::to_owned);
        }

        reply
    }

    /// The headers a Portal page sends with a mutation.
    pub fn form(&self) -> Headers {
        let mut headers = vec![header("origin", ORIGIN), header("content-type", "application/json")];

        if let Some(csrf) = &self.csrf {
            headers.push(header("x-csrf-token", csrf));
        }

        if let Some(cookie) = &self.cookie {
            headers.push(header("cookie", cookie));
        }

        headers
    }

    pub fn csrf(&self) -> &str {
        self.csrf.as_deref().unwrap()
    }

    pub fn cookie(&self) -> &str {
        self.cookie.as_deref().unwrap()
    }

    pub async fn post(&self, path: &str, headers: Headers, body: &str) -> Reply {
        self.request(Method::POST, path, headers, body).await
    }

    /// A correct mutation: fresh status first, then the POST a page would send.
    pub async fn mutate(&mut self, path: &str) -> Reply {
        if self.csrf.is_none() {
            self.status().await;
        }

        self.post(path, self.form(), "{}").await
    }
}

/// A browser with a session, and the headers of an otherwise correct mutation.
pub async fn ready(harness: &Harness) -> (Browser<'_>, Headers) {
    let mut browser = harness.browser();
    browser.status().await;
    let form = browser.form();

    (browser, form)
}
