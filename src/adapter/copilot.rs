use super::{
    classify_stderr, Adapter, AdapterRequest, AdapterResult, ErrorKind, HealthStatus, ModelInfo,
    ModelTier, TaskCategory, TokenUsage,
};
use async_trait::async_trait;
use serde::Deserialize;
use tokio::process::Command;

pub struct CopilotAdapter {
    binary: String,
}

impl CopilotAdapter {
    pub fn new() -> Self {
        Self {
            binary: "copilot".to_string(),
        }
    }
}

impl Default for CopilotAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ── JSONL event shapes from `copilot -p --output-format json` ────────────────
// Relevant events (all others are ephemeral/noise):
//   {"type":"assistant.message","data":{"content":"<text>","outputTokens":N,...}}
//   {"type":"result","exitCode":0,...}

#[derive(Deserialize)]
struct CopilotEvent {
    #[serde(rename = "type")]
    event_type: String,
    data: Option<CopilotEventData>,
}

#[derive(Deserialize)]
struct CopilotEventData {
    content: Option<String>,
    #[serde(rename = "outputTokens")]
    output_tokens: Option<u64>,
}

fn parse_copilot_output(stdout: &str) -> Option<(String, Option<u64>)> {
    let mut content: Option<String> = None;
    let mut output_tokens: Option<u64> = None;

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(event) = serde_json::from_str::<CopilotEvent>(line) {
            if event.event_type == "assistant.message" {
                if let Some(data) = event.data {
                    if let Some(c) = data.content {
                        content = Some(c);
                    }
                    if data.output_tokens.is_some() {
                        output_tokens = data.output_tokens;
                    }
                }
            }
        }
    }

    content.map(|c| (c, output_tokens))
}

// ── Adapter impl ──────────────────────────────────────────────────────────────

#[async_trait]
impl Adapter for CopilotAdapter {
    fn id(&self) -> &'static str {
        "copilot"
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "gpt-5.2".to_string(),
            display_name: "GPT-5.2 (GitHub Copilot)".to_string(),
            tier: ModelTier::Standard,
            cost_weight: 3,
            strengths: vec![
                TaskCategory::Implementation,
                TaskCategory::CodeReview,
                TaskCategory::BugDebug,
            ],
        }]
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
            tracing::debug!(code, %stderr, "copilot exited with error");
            return Err(self.classify_error(&stderr, code));
        }

        match parse_copilot_output(&stdout) {
            Some((text, output_tokens)) => {
                let mut result = AdapterResult::success(text, stdout);
                result.usage = output_tokens.map(|out| TokenUsage {
                    input: 0,
                    output: out,
                    cache_read: None,
                });
                Ok(result)
            }
            None => Err(ErrorKind::UnknownError(
                "copilot: no assistant.message event found in output".to_string(),
            )),
        }
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
        let adapter = CopilotAdapter {
            binary: "this-binary-does-not-exist-polycode-test".to_string(),
        };
        assert!(matches!(
            adapter.health_check().await,
            HealthStatus::Unavailable { .. }
        ));
    }

    #[tokio::test]
    async fn invoke_missing_binary_returns_tool_not_installed() {
        let adapter = CopilotAdapter {
            binary: "this-binary-does-not-exist-polycode-test".to_string(),
        };
        let req = AdapterRequest::new("test", std::env::current_dir().unwrap());
        assert!(matches!(
            adapter.invoke(req).await.unwrap_err(),
            ErrorKind::ToolNotInstalled
        ));
    }

    #[test]
    fn parse_copilot_jsonl_extracts_content() {
        let jsonl = r#"{"type":"session.mcp_server_status_changed","data":{"serverName":"github-mcp-server","status":"connected"},"ephemeral":true}
{"type":"assistant.message","data":{"messageId":"abc","model":"gpt-5.2","content":"hi","outputTokens":5}}"#;
        let (text, tokens) = parse_copilot_output(jsonl).expect("should parse");
        assert_eq!(text, "hi");
        assert_eq!(tokens, Some(5));
    }

    #[test]
    fn parse_copilot_returns_none_on_empty() {
        assert!(parse_copilot_output("").is_none());
    }
}
