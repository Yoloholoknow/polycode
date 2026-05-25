use super::{
    classify_stderr, Adapter, AdapterRequest, AdapterResult, ErrorKind, HealthStatus, ModelInfo,
    ModelTier, TaskCategory,
};
use async_trait::async_trait;
use tokio::process::Command;

pub struct CodexAdapter {
    binary: String,
}

impl CodexAdapter {
    pub fn new() -> Self {
        Self {
            binary: "codex".to_string(),
        }
    }
}

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Adapter for CodexAdapter {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "o4-mini".to_string(),
                display_name: "OpenAI o4-mini".to_string(),
                tier: ModelTier::Fast,
                cost_weight: 2,
                strengths: vec![TaskCategory::QuickEdit, TaskCategory::BugDebug],
            },
            ModelInfo {
                id: "o3".to_string(),
                display_name: "OpenAI o3".to_string(),
                tier: ModelTier::Frontier,
                cost_weight: 9,
                strengths: vec![
                    TaskCategory::Architecture,
                    TaskCategory::Implementation,
                    TaskCategory::BugDebug,
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
        cmd.arg("exec").arg(&req.prompt);

        if let Some(model) = &req.model {
            cmd.arg("-m").arg(model);
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
            tracing::debug!(code, %stderr, "codex exited with error");
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
        let adapter = CodexAdapter {
            binary: "this-binary-does-not-exist-polycode-test".to_string(),
        };
        assert!(matches!(
            adapter.health_check().await,
            HealthStatus::Unavailable { .. }
        ));
    }

    #[tokio::test]
    async fn invoke_missing_binary_returns_tool_not_installed() {
        let adapter = CodexAdapter {
            binary: "this-binary-does-not-exist-polycode-test".to_string(),
        };
        let req = AdapterRequest::new("test", std::env::current_dir().unwrap());
        assert!(matches!(
            adapter.invoke(req).await.unwrap_err(),
            ErrorKind::ToolNotInstalled
        ));
    }
}
