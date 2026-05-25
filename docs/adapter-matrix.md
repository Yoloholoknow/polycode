# Adapter Matrix

> Phase 0 output. Documents headless invocation, model selection, output format,
> error signals, and install requirements for each supported AI coding CLI.
> Updated as new tools are installed and validated.

---

## Validation status

| Tool | Binary | Version | Headless | JSON out | Quota error signal | Status |
|------|--------|---------|----------|----------|--------------------|--------|
| claude-code | `claude` | 2.1.150 | `claude -p "<prompt>"` | `--output-format json` | exit 1, stderr: "rate limit" / "usage limit" | ✅ Validated |
| codex | `codex` | 0.128.0 | `codex exec "<prompt>"` | stream/text (no structured JSON) | exit non-0, stderr: quota / 429 messages | ✅ Installed, needs error validation |
| gemini-cli | `gemini` | 0.37.2 | `gemini -p "<prompt>"` | `-o json` | exit non-0, stderr: quota / RESOURCE_EXHAUSTED | ✅ Installed, needs error validation |
| opencode | `opencode` | 1.14.50 | `opencode run "<msg>"` | `--format json` | exit non-0, stderr: rate limit patterns | ✅ Installed, needs error validation |
| copilot | — | — | `gh copilot suggest -t shell "<prompt>"` | text only | — | ⬜ To install |
| aider | — | — | `aider --message "<msg>" --yes` | text + structured flags | exit non-0, various | ⬜ To install |

---

## Detailed entries

### claude-code

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
- **Auth prereqs:** Claude Pro/Max subscription; `claude` must be logged in (`claude auth login`)
- **Permission mode for P1:** default (non-interactive `-p` mode skips trust dialog; file editing tools still require permission grants — use read-only prompts in P1 or add `--permission-mode acceptEdits` for editing)
- **Session resume:** `--continue` or `--resume <session-id>`

### codex

- **Binary:** `codex` (at `/opt/homebrew/bin/codex`)
- **Version:** 0.128.0
- **Headless command:** `codex exec "<prompt>" [-m <model>]` or `echo "<prompt>" | codex exec -`
- **Model flag:** `-m` (e.g., `-m o4-mini`, `-m o3`)
- **JSON output:** not structured; plain text stream to stdout. No `--format json` flag.
- **Rate limit signal:** TBD — likely exit non-0 + stderr 429/quota message. Needs validation.
- **Auth prereqs:** OpenAI API key (`OPENAI_API_KEY`) or ChatGPT Plus login
- **Notes:** `codex exec` is the non-interactive subcommand. `codex review` for code review non-interactively.

### opencode

- **Binary:** `opencode` (at `/opt/homebrew/bin/opencode`)
- **Version:** 1.14.50
- **Headless command:** `opencode run "<message>" [-m provider/model] [--format json]`
- **Model flag:** `-m provider/model` (e.g., `-m anthropic/claude-sonnet-4-6`, `-m openai/gpt-4o`)
- **JSON output:** `--format json` emits JSON event stream
- **JSON event shape:** TBD — needs validation run
- **Rate limit signal:** TBD. Needs validation.
- **Auth prereqs:** provider-specific (Anthropic key, OpenAI key, etc.); configured via `opencode providers`
- **Notes:** `opencode stats` shows token usage + cost. `opencode models <provider>` lists available models.

### copilot (to install)

- **Install:** `gh extension install github/gh-copilot` or standalone `copilot` binary
- **Headless command:** `gh copilot suggest -t shell "<prompt>"` (limited; explain mode: `gh copilot explain`)
- **Notes:** GitHub Copilot CLI is narrower than other tools (shell commands + code explain only). May be better suited as a specialized sub-adapter. Validate scope in Phase 0.

### aider (to install)

- **Install:** `brew install aider` or `pip install aider-chat`
- **Headless command:** `aider --message "<msg>" --yes [--model <provider>/<model>]`
- **Notes:** aider supports multiple providers via `--model`. Integrates deeply with git. Good candidate for code-edit-heavy tasks.

---

## Error classification guide (for `classify_error` impls)

| Signal | ErrorKind |
|--------|-----------|
| Exit 0 | success |
| stderr contains "rate limit", "usage limit", "quota", "overloaded", "RESOURCE_EXHAUSTED", "429" | `QuotaExceeded` |
| stderr contains "auth", "login", "401", "unauthorized", "unauthenticated", "credential" | `AuthError` |
| stderr contains "network", "connection", "timeout", "502", "503" | `NetworkError` |
| `which <binary>` fails | `ToolNotInstalled` |
| anything else + exit non-0 | `UnknownError(stderr)` |

