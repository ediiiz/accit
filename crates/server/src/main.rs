//! accit: one binary serving the Portal and Traefik's verification endpoint.

use std::io::IsTerminal;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    let logs = tracing_subscriber::fmt().with_env_filter(filter).with_writer(std::io::stderr);

    // Structured logs in containers, readable ones in a terminal.
    if std::io::stderr().is_terminal() {
        logs.init();
    } else {
        logs.json().init();
    }

    let args: Vec<String> = std::env::args().skip(1).collect();

    server::cli::run(&args).await
}
