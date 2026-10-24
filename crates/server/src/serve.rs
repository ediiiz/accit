//! One process, two listeners. SIGTERM stops both and drains for at most `DRAIN`.

use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::Router;
use config::Config;
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::health::Readiness;
use crate::{portal, verify};

const DRAIN: Duration = Duration::from_secs(10);

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
    readiness: Readiness,
    stop: watch::Receiver<bool>,
) -> Result<()> {
    tokio::try_join!(
        serve(portal_listener, portal::router(), stop.clone()),
        serve(verify_listener, verify::router(readiness), stop),
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

    tracing::info!(portal = %portal_address, verify = %verify_address, "listening; no grant state yet, verification fails closed");

    Ok(())
}

pub async fn run(config: Config) -> Result<()> {
    let portal_listener = bind(config.listen.portal).await?;
    let verify_listener = bind(config.listen.verify).await?;
    let (stop, stopping) = watch::channel(false);

    log_start(&portal_listener, &verify_listener)?;

    tokio::select! {
        result = serve_both(portal_listener, verify_listener, Readiness::default(), stopping) => result,
        () = drain_after_signal(stop) => Ok(()),
    }
}
