use super::{
    Adapter, AdapterRequest, AdapterResult, ErrorKind, HealthStatus, ModelInfo, ModelTier,
    TaskCategory, TokenUsage, classify_stderr,
};
use async_trait::async_trait;
use serde::Deserialize;
use tokio::process::Command;

pub struct ClaudeAdapter {
    binary: String,
}

impl ClaudeAdapter {
    pub fn new() -> Self {
        Self {
            binary: "claude".to_string(),
        }
    }
}

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ── JSON output shape from `claude -p --output-format json` ──────────────────

#[derive(Deserialize, Debug)]
struct ClaudeOutput {
    result: Option<String>,
    usage: Option<ClaudeUsage>,
}

#[derive(Deserialize, Debug)]
struct ClaudeUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
}

// ── Adapter impl ──────────────────────────────────────────────────────────────

#[async_trait]
impl Adapter for ClaudeAdapter {
    fn id(&self) -> &'static str {
        "claude-code"
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "claude-haiku-4-5-20251001".to_string(),
                display_name: "Claude Haiku 4.5".to_string(),
                tier: ModelTier::Fast,
                cost_weight: 1,
                strengths: vec![TaskCategory::QuickEdit, TaskCategory::Explanation],
            },
            ModelInfo {
                id: "claude-sonnet-4-6".to_string(),
                display_name: "Claude Sonnet 4.6".to_string(),
                tier: ModelTier::Standard,
                cost_weight: 4,
                strengths: vec![
                    TaskCategory::Implementation,
                    TaskCategory::Refactor,
                    TaskCategory::BugDebug,
                ],
            },
            ModelInfo {
                id: "claude-opus-4-7".to_string(),
                display_name: "Claude Opus 4.7".to_string(),
                tier: ModelTier::Frontier,
                cost_weight: 10,
                strengths: vec![
                    TaskCategory::Architecture,
                    TaskCategory::BugDebug,
                    TaskCategory::CodeReview,
                ],
            },
        ]
    }

    async fn health_check(&self) -> HealthStatus {
        match Command::new(&self.binary).arg("--version").output().await {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HealthStatus::Unavailable {
                reason: format!("'{}' not found on PATH", self.binary),
            },
            Err(e) => HealthStatus::Unavailable {
                reason: e.to_string(),
            },
            Ok(out) if out.status.success() => {
                let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
                HealthStatus::Ok { version }
            }
            Ok(out) => HealthStatus::Degraded {
                reason: String::from_utf8_lossy(&out.stderr).trim().to_string(),
            },
        }
    }

    async fn invoke(&self, req: AdapterRequest) -> Result<AdapterResult, ErrorKind> {
        let mut cmd = Command::new(&self.binary);
        cmd.arg("-p").arg(&req.prompt);
        cmd.arg("--output-format").arg("json");

        if let Some(model) = &req.model {
            cmd.arg("--model").arg(model);
        }

        cmd.current_dir(&req.cwd);

        let out = cmd.output().await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ErrorKind::ToolNotInstalled
            } else {
                ErrorKind::NetworkError(e.to_string())
            }
        })?;

        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();

        if !out.status.success() {
            let code = out.status.code().unwrap_or(-1);
            tracing::debug!(code, %stderr, "claude exited with error");
            return Err(self.classify_error(&stderr, code));
        }

        let parsed: ClaudeOutput = serde_json::from_str(&stdout).map_err(|e| {
            ErrorKind::UnknownError(format!("failed to parse claude JSON output: {e}"))
        })?;

        let text = parsed
            .result
            .ok_or_else(|| ErrorKind::UnknownError("claude returned no result field".to_string()))?;

        let usage = parsed.usage.map(|u| TokenUsage {
            input: u.input_tokens.unwrap_or(0),
            output: u.output_tokens.unwrap_or(0),
            cache_read: u.cache_read_input_tokens,
        });

        let mut result = AdapterResult::success(text, stdout);
        result.usage = usage;
        Ok(result)
    }

    fn classify_error(&self, stderr: &str, exit_code: i32) -> ErrorKind {
        classify_stderr(stderr, exit_code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::ErrorKind;

    #[tokio::test]
    async fn health_check_missing_binary() {
        let adapter = ClaudeAdapter {
            binary: "this-binary-does-not-exist-polycode-test".to_string(),
        };
        let status = adapter.health_check().await;
        assert!(
            matches!(status, crate::adapter::HealthStatus::Unavailable { .. }),
            "expected Unavailable for missing binary"
        );
    }

    #[tokio::test]
    async fn invoke_missing_binary_returns_tool_not_installed() {
        let adapter = ClaudeAdapter {
            binary: "this-binary-does-not-exist-polycode-test".to_string(),
        };
        let req = AdapterRequest::new("test", std::env::current_dir().unwrap());
        let err = adapter.invoke(req).await.unwrap_err();
        assert!(
            matches!(err, ErrorKind::ToolNotInstalled),
            "expected ToolNotInstalled, got: {:?}",
            err
        );
    }
}
