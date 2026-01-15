use crate::models::ValidateRequest;

pub enum Decision {
    Ok,
    NeedsRecheck,
    Suspicious,
}

pub fn decide(payload: &ValidateRequest) -> Decision {
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

pub fn reputation_delta(decision: &Decision) -> f64 {
    // MVP scoring constants.
    match decision {
        Decision::Ok => 1.0,
        Decision::NeedsRecheck => -1.0,
        Decision::Suspicious => -5.0,
    }
}

impl Decision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Decision::Ok => "ok",
            Decision::NeedsRecheck => "needs_recheck",
            Decision::Suspicious => "suspicious",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, Decision};
    use crate::models::ValidateRequest;

    #[test]
    fn decision_from_outcome() {
        let request = ValidateRequest {
            task_id: 1,
            device_id: 2,
            result_hash: None,
            outcome: Some("suspicious".to_string()),
        };
        assert!(matches!(decide(&request), Decision::Suspicious));
    }
}
