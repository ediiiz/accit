//! One process, two listeners. SIGTERM stops both and drains for at most `DRAIN`.

use std::net::SocketAddr;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use axum::Router;
use config::Config;
use store::{Grants, Integrity, Limits};
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::health::{self, Readiness};
use crate::portal::{self, Clock, Portal};
use crate::trust::ProxyTrust;
use crate::verify::{self, Verifier};

const DRAIN: Duration = Duration::from_secs(10);
const CLEANUP: Duration = Duration::from_hours(1);
const CLOCK_CHECK: Duration = Duration::from_secs(30);

async fn bind(address: SocketAddr) -> Result<TcpListener> {
    TcpListener::bind(address).await.with_context(|| format!("binding {address}"))
}

async fn terminated() {
    let interrupt = tokio::signal::ctrl_c();
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .unwrap_or_else(|error| unreachable!("installing the SIGTERM handler: {error}"));

    tokio::select! {
        _ = interrupt => {}
        _ = terminate.recv() => {}
    }
}

async fn stopped(mut stop: watch::Receiver<bool>) {
    let _ = stop.wait_for(|stopped| *stopped).await;
}

async fn serve(listener: TcpListener, app: Router, stop: watch::Receiver<bool>) -> Result<()> {
    let service = app.into_make_service_with_connect_info::<SocketAddr>();

    Ok(axum::serve(listener, service).with_graceful_shutdown(stopped(stop)).await?)
}

async fn serve_both(
    portal_listener: TcpListener,
    verify_listener: TcpListener,
    portal_app: Router,
    verify_app: Router,
    stop: watch::Receiver<bool>,
) -> Result<()> {
    tokio::try_join!(
        serve(portal_listener, portal_app, stop.clone()),
        serve(verify_listener, verify_app, stop),
    )?;

    Ok(())
}

/// Tells both listeners to stop, then gives in-flight requests `DRAIN` to finish.
async fn drain_after_signal(stop: watch::Sender<bool>) {
    terminated().await;
    let _ = stop.send(true);
    tokio::time::sleep(DRAIN).await;
}

fn log_start(portal_listener: &TcpListener, verify_listener: &TcpListener) -> Result<()> {
    let (portal_address, verify_address) = (portal_listener.local_addr()?, verify_listener.local_addr()?);

    tracing::info!(portal = %portal_address, verify = %verify_address, "listening");

    Ok(())
}

async fn open_grants(config: &Config) -> Result<Grants> {
    let limits = Limits {
        grant_duration: config.grant_duration,
        max_grants: config.max_grants,
        audit_retention: config.audit_retention,
    };

    if config.database.is_none() {
        tracing::warn!("no database configured; grants are lost on restart");
    }

    Grants::open(config.database.as_deref(), limits, SystemTime::now())
        .await
        .context("opening grant database")
}

pub async fn run(config: Config) -> Result<()> {
    let portal_listener = bind(config.listen.portal).await?;
    let verify_listener = bind(config.listen.verify).await?;
    let (stop, stopping) = watch::channel(false);
    let readiness = Readiness::default();
    let trust = ProxyTrust::from_config(&config);

    log_start(&portal_listener, &verify_listener)?;
    // Before serving, so no request sees readiness without the Grants behind it.
    let grants = open_grants(&config).await?;
    let integrity = grants.integrity();
    readiness.set(*integrity.borrow() == Integrity::Sound);
    tokio::spawn(grants.clone().clean_up_every(CLEANUP));
    tokio::spawn(health::guard_clocks(readiness.clone(), SystemTime::now, CLOCK_CHECK));

    trust.log_startup();
    let verifier = Verifier::new(&config, grants.clone());
    verifier.log_startup();
    let verify_app = verify::router(readiness.clone(), trust.clone(), verifier);
    let portal_app = portal::router(trust, Portal::new(&config, grants, readiness.clone(), Clock::system()));

    tokio::select! {
        result = serve_both(portal_listener, verify_listener, portal_app, verify_app, stopping) => result,
        error = health::follow(readiness, integrity) => Err(error),
        () = drain_after_signal(stop) => Ok(()),
    }
}
