# Polycode — Full Project Roadmap

> **Pitch:** You already pay for three AI coding tools. Polycode makes them feel like one.

Polycode is a Rust CLI that wraps multiple AI coding CLIs (Claude Code, Codex,
Gemini CLI, OpenCode, Copilot …), routes prompts to the best
tool+model, stretches quota via automatic fallback, and shares project context
across backends.

---

## Problems solved

1. **Quota stretching.** When one tool hits a limit, Polycode automatically falls
   back to the next available one.
2. **Intelligent routing.** Different tools excel at different tasks. Polycode
   picks the right one and conserves expensive quota (e.g., Claude Opus) for
   tasks that actually need it.
3. **Shared context.** A single project memory layer means switching tools
   doesn't mean re-explaining the project.

---

## Architecture (target)

```
CLI / TUI  ──▶  Orchestrator  ──▶  { Context Manager, Router }  ──▶  Adapter Layer  ──▶  Quota Tracker (SQLite)
```

| Component | Role |
|-----------|------|
| **Adapter layer** | One adapter per tool. Vendor quirks quarantined. Every adapter returns a normalized `AdapterResult`. |
| **Router** | Stage 1: task classifier (local Ollama LLM). Stage 2: tool+model selector emitting a ranked preference list. Orchestrator walks it, skipping unavailable tools. |
| **Quota tracker** | Local SQLite. Tracks events, per-adapter availability, invocations. No telemetry without opt-in. |
| **Context manager** | MVP: per-project `.polycode/journal.md` (markdown). Later: vector + graph memory. |

---

## Full Timeline

Single developer, part-time evenings/weekends. Each phase ends in a shippable artifact.

| Phase | Duration | Goal | Shippable |
|-------|----------|------|-----------|
| **0. Validation** | 1 week | Install + test every CLI headless, ToS skim, adapter matrix | `docs/adapter-matrix.md` |
| **1. Skeleton + 1 adapter** | 1 week | Rust project, Adapter trait, Claude adapter, passthrough end-to-end | `polycode "prompt"` works with one tool |
| **2. Multi-adapter + quota fallback** | 2 weeks | All installed adapters, SQLite quota tracker, fallback chain, `doctor` + `status` | **v0.1.0** — manual select + fallback. **First public release.** |
| **3. Journal context** | 1 week | `.polycode/` layout, `journal.md`, auto-update, `init` + `journal` commands | **v0.2.0** — context-aware |
| **4. Rule-based router** | 1 week | Heuristic router (no LLM) on prompt patterns + quota state | **v0.3.0** — automatic selection |
| **5. LLM classifier** | 2 weeks | Ollama integration, classifier prompt + categories, latency tuning, caching | **v0.4.0** — intelligent routing. **Launch on HN/Reddit.** |
| **6. TUI mode** | 1–2 weeks | `polycode chat` (ratatui), multi-turn sessions, live journal pane | **v0.5.0** — interactive |
| **7. Polish + telemetry** | 1 week | Opt-in local telemetry, `replay`, `history`, better errors | **v0.6.0** — daily-driver quality |
| **8. Vector memory** | 3–4 weeks | Embedded vector store, hybrid retrieval, journal → vector pipeline | **v0.7.0** — advanced context |
| **9. v1.0 release** | 2 weeks | Docs site, demo video, blog post, cross-platform binaries | **v1.0.0** — public launch |

**To v0.5 (launch-worthy):** ~8–9 weeks part-time.
**To v1.0:** ~14–18 weeks part-time.

### Critical milestones

- **End of Phase 2 (v0.1.0):** First public release. "I have 5 AI CLIs and one
  manages quota + fallback" is already a useful tool. Post to small communities.
- **End of Phase 5 (v0.4.0):** The marketable moment. Smart routing + journal +
  fallback is a complete product story. Demo video, HN post, dev.to, X/Bluesky.
- **End of Phase 9 (v1.0.0):** Documentation site + demo video. Push for
  influencer adoption.

---

## CLI surface (target)

```bash
polycode "<prompt>"              # route to best tool, run, return
polycode chat                    # interactive TUI mode
polycode --tool claude "..."     # force a specific tool
polycode --model opus "..."      # force a specific model
polycode --dry-run "..."         # show routing decision without executing

polycode status                  # quota state for all tools
polycode doctor                  # detect installed adapters, suggest fixes
polycode journal                 # view/edit project journal
polycode journal clear           # reset project journal
polycode replay <id>             # re-run a past prompt
polycode history                 # browse past invocations
polycode config                  # open config in $EDITOR
polycode init                    # initialize .polycode/ in current directory
```

---

## Tech stack

| Area | Choice | Rationale |
|------|--------|-----------|
| Language | Rust | Single-binary distribution, fast startup (<100ms cold), clean async story |
| CLI | `clap` v4+ | Derive macros, excellent ergonomics |
| TUI | `ratatui` + `crossterm` | Phase 6 |
| Async | `tokio` | rt-multi-thread for subprocess management |
| State | `rusqlite` | Bundled, no external DB, quota tracker (Phase 2) |
| Config | `serde` + `toml` | Phase 2 |
| Logging | `tracing` + `tracing-subscriber` | Structured, level-filtered |
| Classifier | Ollama (local LLM) | Already installed; user picks model (Phase 5) |
| Vector memory | `lancedb` or `qdrant` embedded | Phase 8 — decide based on Rust client maturity |

---

## Adapter targets

Tools targeted by the adapter layer. All have headless/non-interactive modes.

| Tool | Status | Integration method | Notes |
|------|--------|------------------|-------|
| claude-code | ✅ Installed | `claude -p "<prompt>" --output-format json` | Phase 1 — proven. ToS: explicitly allowed. |
| codex | ✅ Installed | `codex exec "<prompt>"` | Phase 2. ToS: allowed (`codex exec` designed for automation). |
| copilot | ✅ Installed | `copilot -p "<prompt>" --output-format json` | Phase 2. ToS: explicitly allowed (GitHub docs programmatic use). |
| opencode | ✅ Installed | `opencode run "<msg>" --format json` | Phase 2. ToS: allowed, but **block Google models** (see ToS analysis). |
| aider | ✅ Installed | `aider --message "<msg>" --yes-always` | Phase 2. ToS: open source (Apache 2.0), no restrictions. |
| gemini-api | Direct API | `reqwest` → generativelanguage.googleapis.com | Phase 2. ToS: ✅ official integration. **NOT a CLI wrapper** — user provides GEMINI_API_KEY. |
| ~~gemini-cli~~ | ❌ EOL | — | **Dead June 18, 2026.** Do not implement. |
| ~~antigravity~~ | ❌ BANNED | — | Google ToS explicitly prohibits; accounts banned. Claude Code named in Google FAQ. |

See [docs/adapter-matrix.md](adapter-matrix.md) and [docs/tos-analysis.md](tos-analysis.md).

---

## Engineering principles

1. **Adapters are quarantined.** Vendor weirdness never leaks into the router or
   orchestrator. Every adapter returns a normalized `AdapterResult`.
2. **Fail gracefully, always.** A broken adapter must never break Polycode. Catch
   errors at the adapter boundary; no panics cross that line.
3. **Speed is a feature.** Cold start <100ms. Every command feels instant.
4. **Privacy is a feature.** No data leaves the machine without opt-in. Local
   state only.
5. **Open by default.** Dual MIT/Apache-2.0. Public roadmap. Issues triaged.
6. **The journal is markdown.** `vim`-friendly. No JSON in human-readable files.
7. **One config per scope.** Global config + project config. No nested includes.
8. **Don't reinvent.** Use crates. Don't write a TUI from scratch.

---

## Definition of done (v1.0)

- [ ] ≥5 adapters working stably
- [ ] LLM classifier <500ms p95 latency
- [ ] Quota tracker with reliable fallback
- [ ] Journal context with auto-update
- [ ] TUI interactive mode
- [ ] Vector memory layer
- [ ] Cross-platform binaries (macOS x86/ARM, Linux x86/ARM, Windows)
- [ ] Documentation site
- [ ] Demo video <2 minutes
- [ ] README with animated GIF
- [ ] 90% test coverage on router and quota logic
