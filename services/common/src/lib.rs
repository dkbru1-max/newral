use std::{env, net::SocketAddr, str::FromStr};
use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

pub fn init_tracing() {
    // Initialize tracing with environment overrides if present.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = FmtSubscriber::builder().with_env_filter(filter).finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}

pub fn env_or<T: FromStr>(key: &str, default: T) -> T {
    // Parse typed environment values with a fallback.
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<T>().ok())
        .unwrap_or(default)
}

pub async fn bind_listener(port: u16) -> TcpListener {
    // Bind on all interfaces for container compatibility.
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    TcpListener::bind(addr).await.expect("bind listener")
}

pub async fn shutdown_signal() {
    // Handle ctrl-c and SIGTERM to allow graceful shutdown.
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("sigterm handler");
        tokio::select! {
            _ = ctrl_c => {},
            _ = sigterm.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        let _ = ctrl_c.await;
    }
}
