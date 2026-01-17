use std::{
    env,
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    panic,
    str::FromStr,
    thread,
    time::{Duration, SystemTime},
};
use tokio::net::TcpListener;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, Registry};

pub struct TracingGuards {
    _file_guard: Option<WorkerGuard>,
}

pub fn init_tracing(service_name: &str) -> TracingGuards {
    // Initialize tracing with environment overrides if present.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let log_dir = env::var("LOG_DIR").unwrap_or_else(|_| "/var/log/newral".to_string());
    let log_root = PathBuf::from(log_dir).join(service_name);
    let stdout_layer = fmt::layer().with_writer(std::io::stdout);
    let mut file_guard: Option<WorkerGuard> = None;
    let mut file_layer = None;

    if fs::create_dir_all(&log_root).is_ok() {
        let appender = panic::catch_unwind(|| {
            tracing_appender::rolling::daily(&log_root, format!("{service_name}.log"))
        })
        .ok();

        if let Some(appender) = appender {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            file_layer = Some(fmt::layer().with_writer(writer));
            file_guard = Some(guard);
        }
    }

    if let Some(layer) = file_layer {
        let subscriber = Registry::default()
            .with(filter)
            .with(stdout_layer)
            .with(layer);
        let _ = tracing::subscriber::set_global_default(subscriber);
    } else {
        let subscriber = Registry::default().with(filter).with(stdout_layer);
        let _ = tracing::subscriber::set_global_default(subscriber);
    }

    if file_guard.is_some() {
        let retention_days = env_or("LOG_RETENTION_DAYS", 14u64);
        let cleanup_interval = env_or("LOG_CLEANUP_INTERVAL_MINUTES", 360u64);
        spawn_log_cleanup(log_root, retention_days, cleanup_interval);
    }

    TracingGuards {
        _file_guard: file_guard,
    }
}

pub fn env_or<T: FromStr>(key: &str, default: T) -> T {
    // Parse typed environment values with a fallback.
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<T>().ok())
        .unwrap_or(default)
}

fn spawn_log_cleanup(log_root: PathBuf, retention_days: u64, cleanup_interval_minutes: u64) {
    if retention_days == 0 || cleanup_interval_minutes == 0 {
        return;
    }

    let retention = Duration::from_secs(retention_days * 24 * 60 * 60);
    let interval = Duration::from_secs(cleanup_interval_minutes * 60);

    thread::spawn(move || loop {
        let cutoff = SystemTime::now().checked_sub(retention);
        if let Some(cutoff) = cutoff {
            cleanup_old_logs(&log_root, cutoff);
        }
        thread::sleep(interval);
    });
}

fn cleanup_old_logs(root: &Path, cutoff: SystemTime) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            cleanup_old_logs(&path, cutoff);
            continue;
        }
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let modified = match metadata.modified() {
            Ok(modified) => modified,
            Err(_) => continue,
        };
        if modified < cutoff {
            let _ = fs::remove_file(&path);
        }
    }
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
