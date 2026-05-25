# Adapter Matrix

> Phase 0 output. Documents headless invocation, model selection, output format,
> error signals, and install requirements for each supported AI coding CLI.
> Updated as new tools are installed and validated.

---

## Validation status

| Tool | Binary | Version | Headless command | JSON out | Status |
|------|--------|---------|-----------------|----------|--------|
| claude-code | `claude` | 2.1.150 | `claude -p "<prompt>"` | `--output-format json` | ✅ Validated + implemented |
| codex | `codex` | 0.128.0 | `codex exec "<prompt>"` | no structured JSON | ✅ Installed, Phase 2 |
| gemini-cli | `gemini` | 0.37.2 | `gemini -p "<prompt>"` | `-o json` | ✅ Installed, Phase 2 |
| opencode | `opencode` | 1.14.50 | `opencode run "<msg>"` | `--format json` | ✅ Installed, Phase 2 |
| copilot | `copilot` | 1.0.54 | `copilot -p "<prompt>"` | `--output-format json` | ✅ Installed, Phase 2 |
| aider | `aider` | 0.86.2 | `aider --message "<msg>" --yes-always` | text only | ✅ Installed, Phase 2 |

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

### codex (Phase 2)

- **Binary:** `codex` (at `/opt/homebrew/bin/codex`)
- **Version:** 0.128.0
- **Headless command:** `codex exec "<prompt>" [-m <model>]` or `echo "<prompt>" | codex exec -`
- **Model flag:** `-m` (e.g., `-m o4-mini`, `-m o3`)
- **JSON output:** no `--format json` flag; plain text stream to stdout. Parse stdout as text.
- **Rate limit signal:** TBD — likely exit non-0 + stderr 429/quota message.
- **Auth prereqs:** OpenAI API key (`OPENAI_API_KEY`) or ChatGPT Plus login
- **Notes:** `codex exec` is the non-interactive subcommand. `codex review` for code review.

---

### opencode (Phase 2)

- **Binary:** `opencode` (at `/opt/homebrew/bin/opencode`)
- **Version:** 1.14.50
- **Headless command:** `opencode run "<message>" [-m provider/model] [--format json]`
- **Model flag:** `-m provider/model` (e.g., `-m anthropic/claude-sonnet-4-6`, `-m openai/gpt-4o`)
- **JSON output:** `--format json` emits a JSON event stream
- **Rate limit signal:** TBD. Needs validation per provider.
- **Auth prereqs:** provider-specific; configured via `opencode providers`
- **Notes:** `opencode stats` shows token usage + cost; `opencode models <provider>` lists models.

---

### copilot (Phase 2)

- **Binary:** `copilot` (at `/opt/homebrew/bin/copilot`, installed via `brew install copilot-cli`)
- **Version:** 1.0.54
- **Headless command:** `copilot -p "<prompt>" [--model <model>] [--output-format json]`
- **Model flag:** `--model` (e.g., `--model gpt-5.2`)
- **JSON output:** `--output-format json` (JSONL — one JSON object per line)
- **Rate limit signal:** TBD — likely exit non-0 + stderr 429/quota.
- **Auth prereqs:** GitHub Copilot subscription; authenticated via `gh auth login` (with `copilot` scope — already present)
- **Notes:** Interface nearly identical to claude-code (-p, --model, --output-format json). ClaudeAdapter is a template for CopilotAdapter.

---

### aider (Phase 2)

- **Binary:** `aider` (at `/opt/homebrew/bin/aider`)
- **Version:** 0.86.2
- **Headless command:** `aider --message "<msg>" --yes-always [--model <provider>/<model>] [--no-git]`
- **Model flag:** `--model` (supports many providers: `openai/gpt-4o`, `anthropic/claude-sonnet-4-6`, etc.)
- **JSON output:** no JSON output flag; stdout is text + ANSI diff output. `--no-pretty` reduces formatting.
- **Rate limit signal:** TBD — varies by provider.
- **Auth prereqs:** provider-specific API key env vars (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc.)
- **Notes:** `--yes-always` for non-interactive. `--no-git` to prevent automatic git commits in passthrough mode. Strong code-edit capabilities.

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

