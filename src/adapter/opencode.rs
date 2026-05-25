use super::{
    classify_stderr, Adapter, AdapterRequest, AdapterResult, ErrorKind, HealthStatus, ModelInfo,
    ModelTier, TaskCategory, TokenUsage,
};
use async_trait::async_trait;
use serde::Deserialize;
use tokio::process::Command;

pub struct OpenCodeAdapter {
    binary: String,
}

impl OpenCodeAdapter {
    pub fn new() -> Self {
        Self {
            binary: "opencode".to_string(),
        }
    }

    /// Returns true if the model ID targets a Google/Gemini backend.
    /// Routing Google tasks through OpenCode violates Google's ToS — use
    /// gemini-api adapter instead.
    fn is_google_model(model: &str) -> bool {
        let lower = model.to_lowercase();
        lower.starts_with("google/") || lower.starts_with("gemini/") || lower.contains("gemini")
    }
}

impl Default for OpenCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ── JSONL event shapes from `opencode run --format json` ─────────────────────
// {"type":"text","part":{"type":"text","text":"<content>","time":{...},...},...}
// {"type":"step_finish","part":{"tokens":{"input":N,"output":M,...},...},...}

#[derive(Deserialize)]
struct OpenCodeEvent {
    #[serde(rename = "type")]
    event_type: String,
    part: Option<OpenCodePart>,
}

#[derive(Deserialize)]
struct OpenCodePart {
    text: Option<String>,
    tokens: Option<OpenCodeTokens>,
}

#[derive(Deserialize)]
struct OpenCodeTokens {
    input: Option<u64>,
    output: Option<u64>,
}

fn parse_opencode_output(stdout: &str) -> Option<(String, Option<TokenUsage>)> {
    let mut text_parts: Vec<String> = Vec::new();
    let mut usage: Option<TokenUsage> = None;

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(event) = serde_json::from_str::<OpenCodeEvent>(line) {
            match event.event_type.as_str() {
                "text" => {
                    if let Some(part) = event.part {
                        if let Some(t) = part.text {
                            if !t.is_empty() {
                                text_parts.push(t);
                            }
                        }
                    }
                }
                "step_finish" => {
                    if let Some(part) = event.part {
                        if let Some(tokens) = part.tokens {
                            usage = Some(TokenUsage {
                                input: tokens.input.unwrap_or(0),
                                output: tokens.output.unwrap_or(0),
                                cache_read: None,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    if text_parts.is_empty() {
        return None;
    }
    Some((text_parts.join(""), usage))
}

// ── Adapter impl ──────────────────────────────────────────────────────────────

#[async_trait]
impl Adapter for OpenCodeAdapter {
    fn id(&self) -> &'static str {
        "opencode"
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "anthropic/claude-sonnet-4-6".to_string(),
                display_name: "Claude Sonnet 4.6 (via opencode)".to_string(),
                tier: ModelTier::Standard,
                cost_weight: 4,
                strengths: vec![TaskCategory::Implementation, TaskCategory::Refactor],
            },
            ModelInfo {
                id: "openai/gpt-4o".to_string(),
                display_name: "GPT-4o (via opencode)".to_string(),
                tier: ModelTier::Standard,
                cost_weight: 5,
                strengths: vec![TaskCategory::Implementation, TaskCategory::CodeReview],
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
        // Block Google/Gemini models — routing through OpenCode violates Google ToS.
        // Use the gemini-api adapter for Google tasks.
        if let Some(model) = &req.model {
            if Self::is_google_model(model) {
                return Err(ErrorKind::UnknownError(format!(
                    "opencode: model '{}' blocked by polycode — Google/Gemini models must use the gemini-api adapter (ToS)",
                    model
                )));
            }
        }

        let mut cmd = Command::new(&self.binary);
        cmd.arg("run").arg(&req.prompt);
        cmd.arg("--format").arg("json");

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
            tracing::debug!(code, %stderr, "opencode exited with error");
            return Err(self.classify_error(&stderr, code));
        }

        match parse_opencode_output(&stdout) {
            Some((text, usage)) => {
                let mut result = AdapterResult::success(text, stdout);
                result.usage = usage;
                Ok(result)
            }
            None => Err(ErrorKind::UnknownError(
                "opencode: no text events found in output".to_string(),
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
        let adapter = OpenCodeAdapter {
            binary: "this-binary-does-not-exist-polycode-test".to_string(),
        };
        assert!(matches!(
            adapter.health_check().await,
            HealthStatus::Unavailable { .. }
        ));
    }

    #[tokio::test]
    async fn invoke_missing_binary_returns_tool_not_installed() {
        let adapter = OpenCodeAdapter {
            binary: "this-binary-does-not-exist-polycode-test".to_string(),
        };
        let req = AdapterRequest::new("test", std::env::current_dir().unwrap());
        assert!(matches!(
            adapter.invoke(req).await.unwrap_err(),
            ErrorKind::ToolNotInstalled
        ));
    }

    #[tokio::test]
    async fn google_model_blocked_before_spawn() {
        let adapter = OpenCodeAdapter::new();
        for model in &["google/gemini-2.0", "gemini/flash", "gemini-2.5-pro"] {
            let mut req = AdapterRequest::new("hi", std::env::current_dir().unwrap());
            req.model = Some(model.to_string());
            let err = adapter.invoke(req).await.unwrap_err();
            assert!(
                matches!(err, ErrorKind::UnknownError(_)),
                "expected UnknownError for google model '{}', got: {:?}",
                model,
                err
            );
        }
    }

    #[test]
    fn parse_opencode_jsonl_extracts_text_and_tokens() {
        let jsonl = r#"{"type":"step_start","timestamp":1,"sessionID":"s","part":{"id":"p1","type":"step-start"}}
{"type":"text","timestamp":2,"sessionID":"s","part":{"id":"p2","type":"text","text":"hi","time":{"start":1,"end":2}}}
{"type":"step_finish","timestamp":3,"sessionID":"s","part":{"id":"p3","reason":"stop","tokens":{"total":100,"input":90,"output":10,"reasoning":0,"cache":{"write":0,"read":0}},"cost":0}}"#;
        let (text, usage) = parse_opencode_output(jsonl).expect("should parse");
        assert_eq!(text, "hi");
        let u = usage.expect("should have usage");
        assert_eq!(u.input, 90);
        assert_eq!(u.output, 10);
    }

    #[test]
    fn parse_opencode_joins_multiple_text_parts() {
        let jsonl = r#"{"type":"text","part":{"text":"hel"}}
{"type":"text","part":{"text":"lo"}}"#;
        let (text, _) = parse_opencode_output(jsonl).expect("should parse");
        assert_eq!(text, "hello");
    }
}
