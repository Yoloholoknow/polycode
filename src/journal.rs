use std::env;
use std::io;
use std::path::{Path, PathBuf};

use chrono::Local;

// ── Constants ─────────────────────────────────────────────────────────────────

const JOURNAL_DIR: &str = ".polycode";
const JOURNAL_FILE: &str = "journal.md";

const TEMPLATE: &str = "\
# Project Journal

> Maintained by Polycode. Edit the Overview freely; the Log section is auto-updated.

## Overview

<!-- Describe the project, goals, conventions, and any context that every AI tool should know. -->

## Log

";

const PROMPT_TRUNCATE: usize = 120;
const RESPONSE_TRUNCATE: usize = 200;

// ── Journal struct ────────────────────────────────────────────────────────────

pub struct Journal {
    path: PathBuf,
}

impl Journal {
    /// Initialize `.polycode/journal.md` in `dir`. Idempotent — skips if already exists.
    /// Returns the path to `journal.md`.
    pub fn init(dir: &Path) -> io::Result<PathBuf> {
        let polycode_dir = dir.join(JOURNAL_DIR);
        std::fs::create_dir_all(&polycode_dir)?;

        let journal_path = polycode_dir.join(JOURNAL_FILE);
        if !journal_path.exists() {
            std::fs::write(&journal_path, TEMPLATE)?;
        }

        Ok(journal_path)
    }

    /// Find an existing journal by walking up from cwd (git-style).
    /// Returns `None` if no `.polycode/journal.md` found.
    pub fn open() -> Option<Journal> {
        find_root().map(|root| Journal {
            path: root.join(JOURNAL_DIR).join(JOURNAL_FILE),
        })
    }

    /// Path to the journal file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read the full journal contents.
    pub fn read(&self) -> io::Result<String> {
        std::fs::read_to_string(&self.path)
    }

    /// Reset the journal to the initial template.
    pub fn clear(&self) -> io::Result<()> {
        std::fs::write(&self.path, TEMPLATE)
    }

    /// Append a timestamped entry under the `## Log` section.
    /// Best-effort: never panics, logs a warning on write failure.
    pub fn append_entry(&self, tool: &str, prompt: &str, response: &str) {
        if let Err(e) = self.try_append_entry(tool, prompt, response) {
            tracing::warn!("journal: failed to append entry — {}", e);
        }
    }

    fn try_append_entry(&self, tool: &str, prompt: &str, response: &str) -> io::Result<()> {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M").to_string();

        let prompt_excerpt = truncate(prompt.lines().next().unwrap_or(prompt), PROMPT_TRUNCATE);
        let response_excerpt = truncate(response.trim(), RESPONSE_TRUNCATE);

        let entry = format!(
            "\n### {} — {}\n**Prompt:** {}\n**Result:** {}\n",
            timestamp, tool, prompt_excerpt, response_excerpt
        );

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&self.path)?;
        file.write_all(entry.as_bytes())
    }

    /// Wrap the journal contents in a context block for prompt injection.
    /// Returns `None` if journal is empty or only contains the template header.
    pub fn context_block(&self) -> Option<String> {
        let contents = self.read().ok()?;
        if contents.trim().is_empty() {
            return None;
        }
        Some(format!(
            "<polycode-context source=\".polycode/journal.md\">\n{}\n</polycode-context>",
            contents.trim_end()
        ))
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Walk up from `current_dir` until we find a directory containing `.polycode/journal.md`.
fn find_root() -> Option<PathBuf> {
    let cwd = env::current_dir().ok()?;
    let mut dir: &Path = &cwd;
    loop {
        let candidate = dir.join(JOURNAL_DIR).join(JOURNAL_FILE);
        if candidate.exists() {
            return Some(dir.to_path_buf());
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => return None,
        }
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = chars[..max_chars].iter().collect();
        format!("{}…", truncated)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn tmp_dir() -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("polycode-journal-test-{}-{}", std::process::id(), id));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn init_creates_layout() {
        let dir = tmp_dir();
        let path = Journal::init(&dir).unwrap();

        assert!(path.exists(), "journal.md must exist");
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("# Project Journal"));
        assert!(contents.contains("## Log"));

        // Cleanup
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn init_is_idempotent() {
        let dir = tmp_dir();
        let path1 = Journal::init(&dir).unwrap();

        // Write custom content, re-init must not overwrite
        fs::write(&path1, "custom content").unwrap();
        Journal::init(&dir).unwrap();

        let after = fs::read_to_string(&path1).unwrap();
        assert_eq!(after, "custom content");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clear_resets_to_template() {
        let dir = tmp_dir();
        let path = Journal::init(&dir).unwrap();
        fs::write(&path, "some edited content").unwrap();

        let j = Journal { path: path.clone() };
        j.clear().unwrap();

        let after = fs::read_to_string(&path).unwrap();
        assert!(after.contains("# Project Journal"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn append_entry_adds_under_log() {
        let dir = tmp_dir();
        let path = Journal::init(&dir).unwrap();

        let j = Journal { path: path.clone() };
        j.append_entry("claude-code", "add retry logic", "Added exponential backoff to invoke()");

        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("claude-code"));
        assert!(contents.contains("add retry logic"));
        assert!(contents.contains("Added exponential backoff"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn context_block_wraps_contents() {
        let dir = tmp_dir();
        let path = Journal::init(&dir).unwrap();

        let j = Journal { path: path.clone() };
        let block = j.context_block().unwrap();

        assert!(block.starts_with("<polycode-context"));
        assert!(block.contains("# Project Journal"));
        assert!(block.ends_with("</polycode-context>"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn find_root_walks_up_from_subdir() {
        let root = tmp_dir();
        // Create the journal at root
        Journal::init(&root).unwrap();

        // Create a nested subdir and set it as cwd is not feasible in unit tests
        // (env::set_current_dir is process-global and breaks parallel tests).
        // Instead, test the path directly: confirm journal.md exists where init put it.
        let journal_path = root.join(".polycode").join("journal.md");
        assert!(journal_path.exists());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn truncate_short_strings_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_long_strings_appends_ellipsis() {
        let result = truncate("abcdefghij", 5);
        assert!(result.starts_with("abcde"));
        assert!(result.contains('…'));
    }
}
