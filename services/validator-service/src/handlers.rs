use crate::ai;
use crate::db;
use crate::logic::{decide, reputation_delta, Decision};
use crate::models::{
    ErrorResponse, SandboxAggregateRequest, SandboxAggregateResponse, SandboxRecheckRequest,
    SandboxRecheckResponse, ValidateRequest, ValidateResponse,
};
use crate::sandbox::run_server_sandbox;
use crate::state::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use tokio_postgres::error::SqlState;

const SQL_INSERT_FLAG: &str =
    "INSERT INTO flags (device_id, task_id, reason, details) VALUES ($1, $2, $3, $4)";

pub async fn healthz() -> StatusCode {
    StatusCode::OK
}

pub async fn readyz() -> StatusCode {
    StatusCode::OK
}

pub async fn validate(
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

    let mut db = state.db.lock().await;
    let transaction = match db.transaction().await {
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

    let reputation_score = match db::update_reputation(&transaction, payload.device_id, delta).await
    {
        Ok(score) => score,
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
                &[
                    &payload.device_id,
                    &payload.task_id,
                    &"low_reputation",
                    &details,
                ],
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

pub async fn recheck(
    State(state): State<AppState>,
    Json(payload): Json<SandboxRecheckRequest>,
) -> impl IntoResponse {
    let mut db = state.db.lock().await;
    let schema = match db::resolve_project_schema(&mut db, payload.project_id).await {
        Ok(schema) => schema,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    code: "project_not_found",
                    message: "project not found",
                }),
            )
                .into_response();
        }
    };

    let task_payload = match db::fetch_task_payload(&mut db, &schema, payload.task_id).await {
        Ok(payload) => payload,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    code: "task_not_found",
                    message: "task not found",
                }),
            )
                .into_response();
        }
    };

    let server_result = match run_server_sandbox(&task_payload, &state.sandbox).await {
        Ok(result) => result,
        Err(err) => {
            let details = serde_json::json!({
                "project_id": payload.project_id,
                "task_id": payload.task_id,
                "error": err,
            });
            let _ = db::insert_audit_flag(
                &*db,
                payload.device_id,
                payload.task_id,
                "sandbox_error",
                &details,
            )
            .await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "sandbox_error",
                    message: "server sandbox failed",
                }),
            )
                .into_response();
        }
    };

    let agent_result_value = db::fetch_latest_result(&mut db, &schema, payload.task_id)
        .await
        .ok();
    let agent_result = agent_result_value
        .as_ref()
        .and_then(|value| serde_json::from_value::<crate::models::AgentResult>(value.clone()).ok());

    let mut decision = Decision::Ok;
    if let Some(agent) = &agent_result {
        let match_ok = agent.stdout_sha256.as_ref() == Some(&server_result.stdout_sha256)
            && agent.status.as_deref().unwrap_or("error") == server_result.status;
        if !match_ok {
            decision = Decision::NeedsRecheck;
        }
    } else {
        decision = Decision::NeedsRecheck;
    }

    let ai_flag = if state.ai_enabled {
        ai::inspect(&task_payload, &server_result)
    } else {
        None
    };
    if ai_flag.is_some() {
        decision = Decision::Suspicious;
    }

    let details = serde_json::json!({
        "project_id": payload.project_id,
        "task_id": payload.task_id,
        "decision": decision.as_str(),
        "ai_flag": ai_flag,
        "server_result": server_result,
        "agent_result": agent_result_value,
    });
    let _ = db::insert_audit_flag(
        &*db,
        payload.device_id,
        payload.task_id,
        "sandbox_recheck",
        &details,
    )
    .await;

    (
        StatusCode::OK,
        Json(SandboxRecheckResponse {
            status: "ok",
            decision: decision.as_str(),
            ai_flag,
            server_result: serde_json::to_value(server_result)
                .unwrap_or_else(|_| serde_json::json!({})),
            agent_result: agent_result_value,
        }),
    )
        .into_response()
}

pub async fn aggregate(
    State(state): State<AppState>,
    Json(payload): Json<SandboxAggregateRequest>,
) -> impl IntoResponse {
    let mut db = state.db.lock().await;
    let schema = match db::resolve_project_schema(&mut db, payload.project_id).await {
        Ok(schema) => schema,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    code: "project_not_found",
                    message: "project not found",
                }),
            )
                .into_response();
        }
    };

    let (total, completed) =
        match db::shard_counts(&mut db, &schema, payload.group_id.as_str()).await {
            Ok(counts) => counts,
            Err(_) => {
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

    let aggregated = serde_json::json!({
        "group_id": payload.group_id,
        "total": total,
        "completed": completed,
    });

    (
        StatusCode::OK,
        Json(SandboxAggregateResponse {
            status: "ok",
            group_id: payload.group_id,
            total,
            completed,
            aggregated,
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_label() {
        let decision = Decision::NeedsRecheck;
        assert_eq!(decision.as_str(), "needs_recheck");
    }
}
