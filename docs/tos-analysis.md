# ToS / Policy Analysis — Polycode Adapter Legality

> Researched 2026-05-24. Review again before each major release.
> Context: Polycode invokes user-installed, user-authenticated CLIs as subprocesses.
> It does NOT proxy traffic, hide usage, or resell access.

---

## Executive Summary

| Tool | Verdict | Risk | Action |
|------|---------|------|--------|
| **Claude Code** | ✅ ALLOWED | None | Ship as-is |
| **Codex CLI (OpenAI)** | ✅ ALLOWED | None | Ship as-is |
| **GitHub Copilot CLI** | ✅ ALLOWED | None | Ship as-is |
| **Aider** | ✅ ALLOWED (out of scope) | None | No native model — removed from adapter chain. See §4. |
| **CodeRabbit** | ⚠️ PENDING | None | Validate headless ToS before implementation. |
| **OpenCode** | ✅ ALLOWED (with caveat) | Medium — don't route Google backend through it | Ship; block Google models via opencode |
| **Gemini CLI** | ⛔ DEAD | EOL June 18, 2026 (25 days) | Drop adapter entirely |
| **Antigravity CLI** | ❌ PROHIBITED | **HIGH** — accounts banned, named in Google FAQ | Do NOT wrap; use Gemini REST API instead |
| **Google Gemini (direct API)** | ✅ ALLOWED | None | Implement GeminiApiAdapter (reqwest, not CLI) |

---

## Detailed Analysis

### 1. Claude Code (Anthropic)

**Verdict: ✅ EXPLICITLY ALLOWED**

Anthropic's own documentation states:
> "Claude can run non-interactively with `claude -p "your prompt"`, which is how you
> integrate Claude into CI pipelines, pre-commit hooks, or any automated workflow."

The `-p/--print` flag exists specifically for non-interactive, programmatic use. Polycode's invocation of `claude -p` is documented, intended behavior.

Anthropic's Consumer ToS prohibits "automated or non-human means" **except via API key or where explicitly permitted** — and `-p` mode is explicitly permitted.

**No ToS risk. Ship as implemented.**

---

### 2. Codex CLI (OpenAI)

**Verdict: ✅ ALLOWED**

OpenAI designed `codex exec` as the non-interactive mode specifically for automation workflows. Their 2026 policies explicitly support "programmatic access tokens" and "trusted, non-interactive local workflows."

OpenAI's usage policy prohibition on "automatically extracting data" refers to data scraping, not automation workflows.

**No ToS risk. Ship as-is.**

---

### 3. GitHub Copilot CLI

**Verdict: ✅ EXPLICITLY ALLOWED**

GitHub's official documentation includes:
- "Running GitHub Copilot CLI programmatically" guide
- "Automating tasks with Copilot CLI and GitHub Actions"
- Agent Client Protocol (ACP) support for third-party tool integration

GitHub states: *"This allows you to use Copilot directly from the terminal, but also allows you to use the CLI programmatically in scripts, CI/CD pipelines, and automation workflows."*

**No ToS risk. Ship as-is.**

---

### 4. Aider

**Verdict: ✅ ALLOWED — but out of scope for Polycode**

Aider is open-source (Apache 2.0). Running open-source software as a subprocess has no ToS restrictions.

**However, Aider is removed from the Polycode adapter chain.** Aider has no native model. It is a front-end that routes prompts to user-supplied provider API keys (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc.). Those exact same keys can be used directly with claude-code, codex, or opencode — tools already in the Polycode chain. Wrapping Aider provides no additional free quota, no new model access, and no capability not already covered. It adds a dependency for zero gain.

**No ToS risk. Removed for scope reasons, not compliance.**

---

### 4a. CodeRabbit CLI

**Verdict: ⚠️ PENDING — re-verify before implementation**

CodeRabbit offers a cloud AI code review service with a documented free tier. A CLI wrapper (`coderabbit`) exists. Preliminary indicators:

- Open-source tooling supported (free plan for public repos)
- Automation-friendly (designed for CI integration)
- No known prohibition on third-party orchestration

**Action:** Before implementing the adapter, re-read CodeRabbit's current ToS and usage policy to confirm headless/automated invocation via Polycode is permitted. Verify that the free tier's CLI invocation is ToS-equivalent to CI integration (it should be — same binary, same user credentials).

---

### 5. OpenCode

**Verdict: ✅ ALLOWED with caveat**

OpenCode is open-source (MIT). Wrapping it is fine. **One critical caveat:** Google explicitly named OpenCode as a prohibited tool for accessing Antigravity/Gemini resources (see §7). Routing Google-backend tasks through OpenCode violates Google's ToS.

**Implementation rule:** When routing to OpenCode, block or warn if the selected model is a Google/Gemini model (e.g., `google/...`, `gemini/...`). Route Google tasks to GeminiApiAdapter (§8) instead.

---

### 6. Gemini CLI

**Verdict: ⛔ EOL — DROP**

**Dead on June 18, 2026** (25 days from research date). Google is deprecating Gemini CLI for all non-enterprise tiers (Google AI Pro, Ultra, Code Assist Individual, Code Assist GitHub). Replacement: Antigravity CLI.

**Remove gemini-cli from the adapter roadmap. Do not implement.**

---

### 7. Antigravity CLI (Google's Gemini CLI replacement)

**Verdict: ❌ EXPLICITLY PROHIBITED — DO NOT IMPLEMENT**

**This is the highest-risk finding in this research.**

Google's Antigravity ToS explicitly prohibits:
> "the use of 3rd party tools or proxies to access Antigravity resources and quotas"

In February 2026, Google enforced this against real users:
- Accounts paying $250/month for AI Ultra were banned without warning
- Google's FAQ was updated to **explicitly name Claude Code, OpenClaw, and OpenCode as prohibited third-party tools**
- No appeal path was provided
- Users were accessing "an increased number of tokens" via third-party proxies
- A mass unban was eventually issued, but **the ban policy remains in place**

**Polycode wrapping Antigravity CLI would:**
1. Violate Antigravity ToS
2. Risk Google account suspension for users
3. Potentially violate Google's broader account terms

**Do not implement an Antigravity CLI adapter under any circumstances. Use the Gemini API directly instead (§8).**

---

### 8. Google Gemini (Direct REST API) — WORKAROUND

**Verdict: ✅ ALLOWED**

The Gemini REST API (`generativelanguage.googleapis.com`) is Google's official integration path. Using it with the user's own `GEMINI_API_KEY` is:
- Explicitly documented and supported
- Not covered by the CLI ToS prohibition (that targets OAuth token piggybacking)
- Standard developer integration

**Recommended implementation (Phase 2+):**
- Add `reqwest` dependency (already planned for Phase 4)
- Implement `GeminiApiAdapter` as a proper API client, not a CLI wrapper
- User provides `GEMINI_API_KEY` in their polycode config
- Route Google-tier tasks through this adapter

```rust
// Adapter ID: "gemini-api" (NOT "gemini-cli")
// No subprocess invocation — pure HTTP
// Request: POST https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent
// Auth: API key in query param or header
```

This gives Polycode **better control** (streaming, structured output, token counts) than the CLI wrapper would have anyway.

---


## Key Principle Confirmed

The enforcement pattern across all vendors is consistent: the prohibition targets **OAuth token / credential misuse** and **quota arbitrage** (accessing more tokens than paid for), NOT users running CLIs as subprocesses for personal automation.

Polycode's architecture is on the correct side of this line because:
- Each invocation uses the user's own credentials
- No credential sharing, token arbitrage, or quota piggybacking
- User invokes Polycode; Polycode invokes their tool (human-initiated chain)
- No reselling, proxying of API traffic, or traffic laundering

**The one vendor that enforces against even this pattern is Google (Antigravity).** Avoid entirely; use direct API.

---

## Action Items

### Phase 2 (complete ✅)

1. **Implement GeminiApiAdapter** (reqwest-based, not CLI subprocess) instead of any Gemini/Antigravity CLI adapter.
2. **Block Google models in OpenCode adapter** — when model is `google/...` or `gemini/...`, redirect to GeminiApiAdapter or warn the user.
3. **Remove Gemini CLI from adapter matrix** — it will be dead before Phase 2 ships.
4. **Remove Aider from adapter chain** — no native model, adds no free quota. Done.
5. **Document clearly in README:** Polycode uses each tool's documented automation interface; users are responsible for their own subscriptions.
6. **Flag Antigravity** in any future "supported tools" list as explicitly unsupported with a note about Google's ToS.

### Pre-implementation (CodeRabbit)

1. **Re-read CodeRabbit ToS** at time of implementation — confirm headless invocation is permitted.
2. **Validate free-tier limits** — document quota/reset cadence in adapter-matrix.md.
3. **Confirm binary name + install path** before writing the adapter.

---

## Sources

- [Anthropic Best Practices — `claude -p` for automation](https://www.anthropic.com/engineering/claude-code-best-practices)
- [Anthropic Acceptable Use Policy](https://www.anthropic.com/legal/aup)
- [Anthropic Consumer Terms](https://www.anthropic.com/legal/consumer-terms)
- [OpenAI Codex CLI — Non-interactive mode](https://developers.openai.com/codex/noninteractive)
- [GitHub Copilot CLI — Programmatic reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-programmatic-reference)
- [GitHub Copilot CLI — Run programmatically](https://docs.github.com/en/copilot/how-tos/copilot-cli/automate-copilot-cli/run-cli-programmatically)
- [Gemini CLI ToS/Privacy — third-party software clause](https://geminicli.com/docs/resources/tos-privacy/)
- [Gemini CLI Issue #24011 — wrapping/orchestration question](https://github.com/google-gemini/gemini-cli/issues/24011)
- [Gemini CLI Discussion #22970 — service abuse update](https://github.com/google-gemini/gemini-cli/discussions/22970)
- [Gemini CLI → Antigravity transition announcement](https://developers.googleblog.com/an-important-update-transitioning-gemini-cli-to-antigravity-cli/)
- [Antigravity bans discussion — #20632](https://github.com/google-gemini/gemini-cli/discussions/20632)
- [OpenClaw ban — Google Antigravity enforcement](https://github.com/openclaw/openclaw/issues/14203)
- [Antigravity named Claude Code, OpenClaw, OpenCode as prohibited](https://mlq.ai/news/google-enforces-tos-bans-on-paid-antigravity-subscribers-using-openclaw-tool/)
- [The Register — Antigravity compute burden & enforcement](https://www.theregister.com/2026/02/23/google_antigravity_compute_burden/)
- [Gemini CLI EOL announcement](https://www.theregister.com/ai-ml/2026/05/20/bye-bye-gemini-cli-google-nudges-devs-toward-antigravity/)
