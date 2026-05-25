# Adapter Matrix

> Phase 0 output. Documents headless invocation, model selection, output format,
> error signals, and install requirements for each supported AI coding CLI.
> Updated as new tools are installed and validated.

---

## Validation status

| Tool | Binary | Version | Headless command | JSON out | Status |
|------|--------|---------|-----------------|----------|--------|
| claude-code | `claude` | 2.1.150 | `claude -p "<prompt>"` | `--output-format json` | ✅ Validated + implemented |
| codex | `codex` | 0.133.0 | `codex exec "<prompt>"` | text stdout (banner → stderr) | ✅ Validated + implemented |
| opencode | `opencode` | 1.15.10 | `opencode run "<msg>"` | `--format json` JSONL event stream | ✅ Validated + implemented (block Google models) |
| copilot | `copilot` | 1.0.54 | `copilot -p "<prompt>"` | `--output-format json` JSONL event stream | ✅ Validated + implemented |
| aider | `aider` | 0.86.2 | `aider --message "<msg>" --yes-always` | text stdout | ✅ Validated + implemented |
| gemini-api | (reqwest) | — | direct REST API | json | ✅ Phase 2 — use GEMINI_API_KEY |
| ~~gemini-cli~~ | ~~`gemini`~~ | ~~0.37.2~~ | — | — | ❌ **EOL June 18 2026** — do not implement |
| ~~antigravity~~ | — | — | — | — | ❌ **PROHIBITED** — ToS bans 3rd-party wrappers; accounts banned |

---

## Detailed entries

### claude-code ✅ (Phase 1 — implemented)

- **Binary:** `claude` (at `~/.local/bin/claude`)
- **Version:** 2.1.150
- **Headless command:** `claude -p "<prompt>" --output-format json [--model <id>]`
- **Model flag:** `--model` (aliases: `sonnet`, `opus`, `haiku`; full IDs: `claude-sonnet-4-6`, `claude-opus-4-7`, `claude-haiku-4-5-20251001`)
- **JSON output shape:**
  ```json
  {
    "type": "result",
    "subtype": "success",
    "result": "<assistant text>",
    "session_id": "...",
    "total_cost_usd": 0.001,
    "usage": { "input_tokens": 100, "output_tokens": 50 }
  }
  ```
- **Rate limit signal:** exit code 1; stderr contains "rate limit", "usage limit", or "overloaded"
- **Auth error signal:** exit code 1; stderr contains "authentication", "login", "401", "unauthorized"
- **Tool not installed:** `which claude` fails
- **Auth prereqs:** Claude Pro/Max subscription; logged in via `claude auth login`
- **Session resume:** `--continue` or `--resume <session-id>`

---

### codex ✅ (Phase 2 — implemented)

- **Binary:** `codex` (at `/opt/homebrew/bin/codex`)
- **Version:** 0.133.0
- **Headless command:** `codex exec "<prompt>" [-m <model>]`
- **Model flag:** `-m` (e.g., `-m o4-mini`, `-m o3`)
- **Output:** stdout = clean answer text only. All session info/banner goes to stderr. Parse `stdout.trim()`.
- **Rate limit signal:** exit non-0 + stderr 429/quota message.
- **Auth prereqs:** OpenAI API key (`OPENAI_API_KEY`) or ChatGPT Plus login
- **Notes:** `codex exec` is the non-interactive subcommand. Default model: gpt-5.5.

---

### opencode ✅ (Phase 2 — implemented)

- **Binary:** `opencode` (at `/opt/homebrew/bin/opencode`)
- **Version:** 1.15.10
- **Headless command:** `opencode run "<message>" [-m provider/model] --format json`
- **Model flag:** `-m provider/model` (e.g., `-m anthropic/claude-sonnet-4-6`, `-m openai/gpt-4o`)
- **JSON event stream shape** (`--format json`):
  ```
  {"type":"step_start",...}
  {"type":"text","part":{"text":"<content chunk>","time":{...},...},...}   ← collect all
  {"type":"step_finish","part":{"tokens":{"input":N,"output":M,...},...},...}
  ```
  Collect all `type:"text"` `part.text` values; join for final answer. Tokens from `step_finish`.
- **Rate limit signal:** exit non-0 + stderr/event quota/429 message.
- **Auth prereqs:** provider-specific; configured via `opencode providers`
- **Google model block:** Polycode rejects any model starting with `google/`, `gemini/`, or containing `gemini` (ToS). Route Google tasks to `gemini-api` adapter instead.
- **Notes:** `opencode stats` shows token usage + cost; `opencode models <provider>` lists models.

---

### copilot ✅ (Phase 2 — implemented)

- **Binary:** `copilot` (at `/opt/homebrew/bin/copilot`, installed via `brew install copilot-cli`)
- **Version:** 1.0.54
- **Headless command:** `copilot -p "<prompt>" [--model <model>] --output-format json`
- **Model flag:** `--model` (e.g., `--model gpt-5.2`)
- **JSON event stream shape** (`--output-format json`):
  ```
  {"type":"session.mcp_server_status_changed",...,"ephemeral":true}   ← noise, skip
  {"type":"assistant.message","data":{"content":"<text>","outputTokens":N,"model":"..."},...}   ← result
  {"type":"result","exitCode":0,...}
  ```
  Parse for `type:"assistant.message"`, extract `data.content`. Input tokens not exposed in this event stream (output tokens available via `data.outputTokens`).
- **Rate limit signal:** exit non-0 + event/stderr 429/quota.
- **Auth prereqs:** GitHub Copilot subscription; authenticated via `gh auth login` (with `copilot` scope)
- **Notes:** Interface nearly identical to claude-code. ClaudeAdapter is the template.

---

### aider ✅ (Phase 2 — implemented)

- **Binary:** `aider` (at `/opt/homebrew/bin/aider`)
- **Version:** 0.86.2
- **Headless command:** `aider --message "<msg>" --yes-always --no-git --no-pretty [--model <provider>/<model>]`
- **Model flag:** `--model` (supports many providers: `openai/gpt-4o`, `anthropic/claude-sonnet-4-6`, etc.)
- **Output:** text stdout (trimmed). `--no-pretty` reduces ANSI formatting. `--no-git` prevents auto-commits.
- **Rate limit signal:** varies by provider; exit non-0 + stderr quota/429 message.
- **Auth prereqs:** provider-specific API key env vars (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc.)
- **Notes:** Strong code-edit capabilities (applies diffs directly). `--yes-always` for non-interactive approval.

---

## Error classification guide (for `classify_error` impls)

| Signal | ErrorKind |
|--------|-----------|
| Exit 0 | success |
| stderr contains "rate limit", "usage limit", "quota", "overloaded", "RESOURCE_EXHAUSTED", "429", "too many requests" | `QuotaExceeded` |
| stderr contains "auth", "login", "401", "unauthorized", "unauthenticated", "credential" | `AuthError` |
| stderr contains "network", "connection", "timeout", "502", "503" | `NetworkError` |
| `which <binary>` fails | `ToolNotInstalled` |
| anything else + exit non-0 | `UnknownError(stderr)` |

Implemented in `src/adapter/mod.rs::classify_stderr()` — shared helper all adapters call.

