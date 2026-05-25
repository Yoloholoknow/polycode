use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

// ── Taxonomy ─────────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskCategory {
    QuickEdit,
    Refactor,
    BugDebug,
    Architecture,
    Implementation,
    CodeReview,
    Documentation,
    Research,
    Explanation,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ModelTier {
    Fast,
    Standard,
    Frontier,
}

// ── Model info ────────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub tier: ModelTier,
    /// Relative cost weight 1–10 (used by router to conserve expensive quota)
    pub cost_weight: u8,
    pub strengths: Vec<TaskCategory>,
}

// ── Request / Result ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AdapterRequest {
    pub prompt: String,
    /// Model ID override — None means adapter picks its default
    pub model: Option<String>,
    /// Working directory for the subprocess
    pub cwd: PathBuf,
}

impl AdapterRequest {
    pub fn new(prompt: impl Into<String>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            prompt: prompt.into(),
            model: None,
            cwd: cwd.into(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterResult {
    pub text: String,
    pub model_used: Option<String>,
    pub usage: Option<TokenUsage>,
    /// Raw stdout from the subprocess (for debugging)
    pub raw: String,
}

impl AdapterResult {
    pub fn success(text: impl Into<String>, raw: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            model_used: None,
            usage: None,
            raw: raw.into(),
        }
    }
}

// ── Error kinds ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, thiserror::Error)]
pub enum ErrorKind {
    #[error("quota exceeded (reset in {reset_hint:?})")]
    QuotaExceeded { reset_hint: Option<Duration> },

    #[error("authentication error — run the tool's login command")]
    AuthError,

    #[error("network error: {0}")]
    NetworkError(String),

    #[error("tool not installed or not on PATH")]
    ToolNotInstalled,

    #[error("unknown error: {0}")]
    UnknownError(String),
}

// ── Health status ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum HealthStatus {
    Ok {
        #[allow(dead_code)]
        version: String,
    },
    Degraded { reason: String },
    Unavailable { reason: String },
}

impl HealthStatus {
    pub fn is_ok(&self) -> bool {
        matches!(self, HealthStatus::Ok { .. })
    }
}

// ── Adapter trait ─────────────────────────────────────────────────────────────

#[async_trait]
pub trait Adapter: Send + Sync {
    fn id(&self) -> &'static str;
    #[allow(dead_code)]
    fn models(&self) -> Vec<ModelInfo>;
    async fn health_check(&self) -> HealthStatus;
    async fn invoke(&self, request: AdapterRequest) -> Result<AdapterResult, ErrorKind>;
    fn classify_error(&self, stderr: &str, exit_code: i32) -> ErrorKind;
}

// ── Shared error classification helper ───────────────────────────────────────

/// Default classify logic shared across adapters. Adapters can call this then
/// override for tool-specific patterns.
pub fn classify_stderr(stderr: &str, exit_code: i32) -> ErrorKind {
    let lower = stderr.to_lowercase();

    if lower.contains("rate limit")
        || lower.contains("usage limit")
        || lower.contains("quota")
        || lower.contains("overloaded")
        || lower.contains("resource_exhausted")
        || lower.contains("429")
        || lower.contains("too many requests")
    {
        return ErrorKind::QuotaExceeded { reset_hint: None };
    }

    if lower.contains("auth")
        || lower.contains("login")
        || lower.contains("unauthorized")
        || lower.contains("unauthenticated")
        || lower.contains("credential")
        || exit_code == 401
    {
        return ErrorKind::AuthError;
    }

    if lower.contains("network")
        || lower.contains("connection")
        || lower.contains("timeout")
        || lower.contains("502")
        || lower.contains("503")
    {
        return ErrorKind::NetworkError(stderr.to_string());
    }

    ErrorKind::UnknownError(stderr.to_string())
}

pub mod aider;
pub mod claude;
pub mod codex;
pub mod copilot;
pub mod opencode;

// ── Adapter registry ──────────────────────────────────────────────────────────

pub const DEFAULT_CHAIN: &[&str] = &["claude-code", "codex", "copilot", "opencode", "aider"];

pub fn build_all() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(claude::ClaudeAdapter::new()),
        Box::new(codex::CodexAdapter::new()),
        Box::new(copilot::CopilotAdapter::new()),
        Box::new(opencode::OpenCodeAdapter::new()),
        Box::new(aider::AiderAdapter::new()),
    ]
}

pub fn by_id(id: &str) -> Option<Box<dyn Adapter>> {
    match id {
        "claude-code" => Some(Box::new(claude::ClaudeAdapter::new())),
        "codex" => Some(Box::new(codex::CodexAdapter::new())),
        "copilot" => Some(Box::new(copilot::CopilotAdapter::new())),
        "opencode" => Some(Box::new(opencode::OpenCodeAdapter::new())),
        "aider" => Some(Box::new(aider::AiderAdapter::new())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_patterns() {
        let cases = [
            "Error: rate limit reached",
            "Usage limit exceeded for this period",
            "RESOURCE_EXHAUSTED: quota depleted",
            "429 Too Many Requests",
            "Model overloaded, try again",
        ];
        for stderr in &cases {
            matches!(
                classify_stderr(stderr, 1),
                ErrorKind::QuotaExceeded { .. }
            );
        }
    }

    #[test]
    fn auth_patterns() {
        let cases = [
            "Authentication failed",
            "Please login first",
            "401 unauthorized",
            "Invalid credential",
        ];
        for stderr in &cases {
            assert!(
                matches!(classify_stderr(stderr, 1), ErrorKind::AuthError),
                "expected AuthError for: {stderr}"
            );
        }
    }

    #[test]
    fn unknown_fallthrough() {
        let kind = classify_stderr("Something unexpected happened", 2);
        assert!(matches!(kind, ErrorKind::UnknownError(_)));
    }
}
