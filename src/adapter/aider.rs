use super::{
    classify_stderr, Adapter, AdapterRequest, AdapterResult, ErrorKind, HealthStatus, ModelInfo,
    ModelTier, TaskCategory,
};
use async_trait::async_trait;
use tokio::process::Command;

pub struct AiderAdapter {
    binary: String,
}

impl AiderAdapter {
    pub fn new() -> Self {
        Self {
            binary: "aider".to_string(),
        }
    }
}

impl Default for AiderAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Adapter for AiderAdapter {
    fn id(&self) -> &'static str {
        "aider"
    }

    fn models(&self) -> Vec<ModelInfo> {
        // Aider supports many providers via --model provider/model.
        // Listing a few representative defaults; router populates this in Phase 4.
        vec![
            ModelInfo {
                id: "openai/gpt-4o".to_string(),
                display_name: "GPT-4o (via aider)".to_string(),
                tier: ModelTier::Standard,
                cost_weight: 5,
                strengths: vec![TaskCategory::Implementation, TaskCategory::Refactor],
            },
            ModelInfo {
                id: "anthropic/claude-sonnet-4-6".to_string(),
                display_name: "Claude Sonnet 4.6 (via aider)".to_string(),
                tier: ModelTier::Standard,
                cost_weight: 4,
                strengths: vec![TaskCategory::Implementation, TaskCategory::BugDebug],
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
        cmd.arg("--message").arg(&req.prompt);
        cmd.arg("--yes-always");
        cmd.arg("--no-git");
        cmd.arg("--no-pretty");

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
            tracing::debug!(code, %stderr, "aider exited with error");
            return Err(self.classify_error(&stderr, code));
        }

        let text = stdout.trim().to_string();
        Ok(AdapterResult::success(text, stdout))
    }

    fn classify_error(&self, stderr: &str, exit_code: i32) -> ErrorKind {
        classify_stderr(stderr, exit_code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn health_check_missing_binary() {
        let adapter = AiderAdapter {
            binary: "this-binary-does-not-exist-polycode-test".to_string(),
        };
        assert!(matches!(
            adapter.health_check().await,
            HealthStatus::Unavailable { .. }
        ));
    }

    #[tokio::test]
    async fn invoke_missing_binary_returns_tool_not_installed() {
        let adapter = AiderAdapter {
            binary: "this-binary-does-not-exist-polycode-test".to_string(),
        };
        let req = AdapterRequest::new("test", std::env::current_dir().unwrap());
        assert!(matches!(
            adapter.invoke(req).await.unwrap_err(),
            ErrorKind::ToolNotInstalled
        ));
    }
}
