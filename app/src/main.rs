//! Runs the app (port 8080) and ops (port 9090) servers until SIGTERM.

use std::{error::Error, future::IntoFuture, process::ExitCode, time::Duration};

use miniarcade::{AppState, config::Config, logging, metrics::Metrics, store::Store};
use tokio::{
    net::TcpListener,
    signal::unix::{SignalKind, signal},
    sync::watch,
};
use tracing::{error, info};

#[tokio::main]
async fn main() -> ExitCode {
    tracing::subscriber::set_global_default(logging::json_subscriber(std::io::stdout))
        .expect("no other subscriber is installed");
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error!(%error, "exiting");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let config = Config::from_env()?;
    let state = AppState::new(Store::new(&config.redis_url)?, Metrics::new());
    let app_listener = TcpListener::bind(config.app_addr).await?;
    let ops_listener = TcpListener::bind(config.ops_addr).await?;
    info!(app = %config.app_addr, ops = %config.ops_addr, "listening");

    let (stop, stopped) = watch::channel(false);
    let drain = config.drain;
    tokio::spawn(async move {
        shutdown_signal(drain).await;
        stop.send_replace(true);
    });
    let app = axum::serve(app_listener, miniarcade::app(state.clone()))
        .with_graceful_shutdown(wait_for(stopped.clone()));
    let ops =
        axum::serve(ops_listener, miniarcade::ops(state)).with_graceful_shutdown(wait_for(stopped));
    tokio::try_join!(app.into_future(), ops.into_future())?;
    info!("stopped");
    Ok(())
}

/// After this resolves, a server takes no new connections and finishes the
/// requests in flight.
async fn wait_for(mut stopped: watch::Receiver<bool>) {
    // An error means the sender is gone, which also means stop.
    let _ = stopped.wait_for(|&stop| stop).await;
}

/// On SIGTERM, keeps serving for `drain` while Kubernetes takes the pod out
/// of the Service, so no request lands on a closed port. Ctrl-C (a local
/// run) stops at once.
async fn shutdown_signal(drain: Duration) {
    let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM handler installs");
    tokio::select! {
        _ = sigterm.recv() => {
            info!(drain_seconds = drain.as_secs(), "SIGTERM: draining");
            tokio::time::sleep(drain).await;
        }
        _ = tokio::signal::ctrl_c() => info!("interrupted"),
    }
    info!("shutting down");
}
