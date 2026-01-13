use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{env, net::SocketAddr, sync::Arc};
use tokio_postgres::{error::SqlState, Client, NoTls};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[derive(Clone)]
struct AppState {
    db: Arc<Client>,
}

#[allow(dead_code)]
const SQL_INSERT_FLAG: &str =
    "INSERT INTO flags (device_id, task_id, reason, details) VALUES ($1, $2, $3, $4)";
#[allow(dead_code)]
const SQL_SELECT_TASK_RESULT: &str =
    "SELECT id, status, result FROM task_results WHERE task_id = $1 AND device_id = $2";
#[allow(dead_code)]
const SQL_UPSERT_REPUTATION: &str = "INSERT INTO device_reputation (device_id, score) \
VALUES ($1, $2) \
ON CONFLICT (device_id) DO UPDATE SET score = device_reputation.score + $2, updated_at = NOW() \
RETURNING score";

#[derive(Deserialize)]
struct ValidateRequest {
    task_id: i64,
    device_id: i64,
    result_hash: Option<String>,
    outcome: Option<String>,
}

#[derive(Serialize)]
struct ValidateResponse {
    status: &'static str,
    decision: &'static str,
    reputation_score: f64,
}

#[derive(Serialize)]
struct ErrorResponse {
    code: &'static str,
    message: &'static str,
}

enum Decision {
    Ok,
    NeedsRecheck,
    Suspicious,
}

#[tokio::main]
async fn main() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = FmtSubscriber::builder().with_env_filter(filter).finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    let port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080);
    // Database is required for reputation/flag writes.
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL is required");

    let (db, connection) = tokio_postgres::connect(&database_url, NoTls)
        .await
        .expect("connect db");
    tokio::spawn(async move {
        // Drive the connection in the background.
        if let Err(err) = connection.await {
            tracing::error!(error = %err, "database connection error");
        }
    });
    let state = AppState { db: Arc::new(db) };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/validate", post(validate))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind listener");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve");
}

async fn healthz() -> StatusCode {
    StatusCode::OK
}

async fn readyz() -> StatusCode {
    StatusCode::OK
}

async fn validate(
    State(state): State<AppState>,
    Json(payload): Json<ValidateRequest>,
) -> impl IntoResponse {
    let decision = decide(&payload);
    let delta = reputation_delta(&decision);

    // Emit audit-style decision log.
    tracing::info!(
        task_id = payload.task_id,
        device_id = payload.device_id,
        decision = decision.as_str(),
        reputation_delta = delta,
        "validation decision"
    );

    // Keep reputation updates and flags in a single transaction.
    let transaction = match state.db.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            tracing::error!(error = %err, "start transaction failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: "database error",
                }),
            )
                .into_response();
        }
    };

    let score_row = match transaction
        .query_one(SQL_UPSERT_REPUTATION, &[&payload.device_id, &delta])
        .await
    {
        Ok(row) => row,
        Err(err) => {
            if let Some(db_err) = err.as_db_error() {
                if db_err.code() == &SqlState::FOREIGN_KEY_VIOLATION {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            code: "unknown_device",
                            message: "device not found",
                        }),
                    )
                        .into_response();
                }
            }
            tracing::error!(error = %err, "update reputation failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: "database error",
                }),
            )
                .into_response();
        }
    };

    let reputation_score: f64 = score_row.get(0);
    tracing::info!(
        device_id = payload.device_id,
        reputation_score = reputation_score,
        "reputation updated"
    );

    if matches!(decision, Decision::Suspicious) {
        // Suspicious outcome triggers an immediate flag.
        let details = serde_json::json!({
            "task_id": payload.task_id,
            "result_hash": payload.result_hash,
        });
        if let Err(err) = transaction
            .execute(
                SQL_INSERT_FLAG,
                &[
                    &payload.device_id,
                    &payload.task_id,
                    &"suspicious_result",
                    &details,
                ],
            )
            .await
        {
            tracing::error!(error = %err, "insert flag failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: "database error",
                }),
            )
                .into_response();
        }
        tracing::info!(
            device_id = payload.device_id,
            task_id = payload.task_id,
            "flag created: suspicious_result"
        );
    }

    if reputation_score <= -10.0 {
        // Persistent low reputation triggers a separate flag.
        let details = serde_json::json!({
            "score": reputation_score,
        });
        if let Err(err) = transaction
            .execute(
                SQL_INSERT_FLAG,
                &[&payload.device_id, &payload.task_id, &"low_reputation", &details],
            )
            .await
        {
            tracing::error!(error = %err, "insert low reputation flag failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: "database error",
                }),
            )
                .into_response();
        }
        tracing::info!(
            device_id = payload.device_id,
            task_id = payload.task_id,
            "flag created: low_reputation"
        );
    }

    if let Err(err) = transaction.commit().await {
        tracing::error!(error = %err, "commit failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                code: "db_error",
                message: "database error",
            }),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(ValidateResponse {
            status: "ok",
            decision: decision.as_str(),
            reputation_score,
        }),
    )
        .into_response()
}

fn decide(payload: &ValidateRequest) -> Decision {
    // Use explicit outcome if provided, else fallback to hash hints.
    if let Some(outcome) = payload.outcome.as_deref() {
        return match outcome {
            "ok" | "OK" => Decision::Ok,
            "needs_recheck" | "NEEDS_RECHECK" => Decision::NeedsRecheck,
            "suspicious" | "SUSPICIOUS" => Decision::Suspicious,
            _ => Decision::NeedsRecheck,
        };
    }

    match payload.result_hash.as_deref() {
        Some("recheck") => Decision::NeedsRecheck,
        Some("suspicious") => Decision::Suspicious,
        _ => Decision::Ok,
    }
}

fn reputation_delta(decision: &Decision) -> f64 {
    // MVP scoring constants.
    match decision {
        Decision::Ok => 1.0,
        Decision::NeedsRecheck => -1.0,
        Decision::Suspicious => -5.0,
    }
}

impl Decision {
    fn as_str(&self) -> &'static str {
        match self {
            Decision::Ok => "ok",
            Decision::NeedsRecheck => "needs_recheck",
            Decision::Suspicious => "suspicious",
        }
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("sigterm handler");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        #[cfg(unix)]
        _ = sigterm.recv() => {},
    }
}
