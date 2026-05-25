# Polycode

> You already pay for three AI coding tools. Polycode makes them feel like one.

Polycode is a fast CLI that wraps multiple AI coding assistants — Claude Code,
Codex, Gemini CLI, OpenCode, Copilot CLI, and more — intelligently routes your
prompts to the best tool+model, stretches quota via automatic fallback, and keeps
a shared project context across all backends.

## Problems solved

- **Quota stretching.** Hit a rate limit? Polycode falls back to the next
  available tool automatically — you never see that error again.
- **Intelligent routing.** Quick edits go to fast models. Heavy reasoning goes to
  Opus or GPT-5. Free-tier capacity isn't wasted on simple tasks.
- **Shared context.** A per-project journal follows you across tools. Switching
  from Claude to Codex mid-session doesn't mean re-explaining the project.

## Privacy first

All state — quota tracking, project journal, invocation history — is **local to
your machine**. Nothing leaves without explicit opt-in. This is a feature, not a
footnote.

## Status

Early development. See [docs/ROADMAP.md](docs/ROADMAP.md) for the full plan and
timeline.

## Usage (Phase 1 — single adapter passthrough)

```bash
polycode "fix the off-by-one error in src/parser.rs"
polycode --model haiku "explain this function"
polycode --dry-run "refactor the auth middleware"   # shows routing decision, no invocation
polycode --help
```

## Install

```bash
cargo install --path .
```

## License

Licensed under either of:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
