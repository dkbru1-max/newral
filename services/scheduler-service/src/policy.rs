use std::{env, fmt};

#[derive(Clone)]
#[allow(dead_code)]
pub struct PolicyConfig {
    pub ai_mode: AiMode,
    pub max_concurrent_tasks: u32,
    pub max_daily_budget: f64,
    pub recheck_threshold: f64,
}

impl PolicyConfig {
    pub fn from_env() -> Self {
        // Defaults keep MVP deterministic when env vars are missing.
        let ai_mode = env::var("AI_MODE")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(AiMode::AiOff);
        let max_concurrent_tasks = env::var("POLICY_MAX_CONCURRENT_TASKS")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(10);
        let max_daily_budget = env::var("POLICY_MAX_DAILY_BUDGET")
            .ok()
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(100.0);
        let recheck_threshold = env::var("POLICY_RECHECK_THRESHOLD")
            .ok()
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.2);

        Self {
            ai_mode,
            max_concurrent_tasks,
            max_daily_budget,
            recheck_threshold,
        }
    }
}

#[derive(Clone, Copy)]
#[allow(clippy::enum_variant_names)]
pub enum AiMode {
    AiOff,
    AiAdvisory,
    AiAssisted,
    AiFull,
}

impl fmt::Display for AiMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            AiMode::AiOff => "AI_OFF",
            AiMode::AiAdvisory => "AI_ADVISORY",
            AiMode::AiAssisted => "AI_ASSISTED",
            AiMode::AiFull => "AI_FULL",
        };
        write!(f, "{value}")
    }
}

impl std::str::FromStr for AiMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "AI_OFF" => Ok(AiMode::AiOff),
            "AI_ADVISORY" => Ok(AiMode::AiAdvisory),
            "AI_ASSISTED" => Ok(AiMode::AiAssisted),
            "AI_FULL" => Ok(AiMode::AiFull),
            _ => Err(()),
        }
    }
}

#[derive(Clone)]
pub struct PolicyEngine {
    config: PolicyConfig,
}

impl PolicyEngine {
    pub fn new(config: PolicyConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &PolicyConfig {
        &self.config
    }

    pub fn evaluate_task_request(&self, proposal: TaskRequestProposal) -> PolicyDecision {
        let mut reasons = Vec::new();

        // AI proposals are denied when AI is disabled.
        if proposal.source == ProposalSource::Ai && matches!(self.config.ai_mode, AiMode::AiOff) {
            reasons.push("ai_off: ai proposals disabled".to_string());
            return PolicyDecision::Denied { reasons };
        }

        // Clamp requested tasks to configured max.
        if proposal.requested_tasks > self.config.max_concurrent_tasks {
            reasons.push(format!(
                "requested_tasks {} exceeds max {}",
                proposal.requested_tasks, self.config.max_concurrent_tasks
            ));
            return PolicyDecision::Limited {
                granted_tasks: self.config.max_concurrent_tasks,
                reasons,
            };
        }

        PolicyDecision::Allowed { reasons }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum ProposalSource {
    Ai,
    System,
    Unknown,
}

impl ProposalSource {
    pub fn from_optional(value: Option<&str>) -> Self {
        match value {
            Some("ai") | Some("AI") => ProposalSource::Ai,
            Some("system") | Some("SYSTEM") => ProposalSource::System,
            _ => ProposalSource::Unknown,
        }
    }
}

pub struct TaskRequestProposal {
    pub requested_tasks: u32,
    pub source: ProposalSource,
}

pub enum PolicyDecision {
    Allowed {
        reasons: Vec<String>,
    },
    Limited {
        granted_tasks: u32,
        reasons: Vec<String>,
    },
    Denied {
        reasons: Vec<String>,
    },
}

impl PolicyDecision {
    pub fn decision(&self) -> &'static str {
        match self {
            PolicyDecision::Allowed { .. } => "allow",
            PolicyDecision::Limited { .. } => "limit",
            PolicyDecision::Denied { .. } => "deny",
        }
    }

    pub fn reasons(&self) -> &[String] {
        match self {
            PolicyDecision::Allowed { reasons } => reasons,
            PolicyDecision::Limited { reasons, .. } => reasons,
            PolicyDecision::Denied { reasons } => reasons,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_off_denies_ai_proposals() {
        let mut config = PolicyConfig::from_env();
        config.ai_mode = AiMode::AiOff;
        let engine = PolicyEngine::new(config);
        let decision = engine.evaluate_task_request(TaskRequestProposal {
            requested_tasks: 1,
            source: ProposalSource::Ai,
        });
        assert!(matches!(decision, PolicyDecision::Denied { .. }));
    }

    #[test]
    fn clamps_requested_tasks() {
        let mut config = PolicyConfig::from_env();
        config.max_concurrent_tasks = 2;
        let engine = PolicyEngine::new(config);
        let decision = engine.evaluate_task_request(TaskRequestProposal {
            requested_tasks: 5,
            source: ProposalSource::System,
        });
        match decision {
            PolicyDecision::Limited { granted_tasks, .. } => assert_eq!(granted_tasks, 2),
            _ => panic!("expected limited decision"),
        }
    }
}
