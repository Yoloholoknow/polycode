use crate::adapter::{self, TaskCategory, DEFAULT_CHAIN};

// ── Prompt classifier ─────────────────────────────────────────────────────────

/// Classify a prompt into a task category using keyword heuristics.
/// Ordered: first match wins. Mirrors the pattern of `adapter::classify_stderr`.
pub fn classify_prompt(prompt: &str) -> TaskCategory {
    let lower = prompt.to_lowercase();

    if lower.contains("bug")
        || lower.contains("fix")
        || lower.contains("error")
        || lower.contains("crash")
        || lower.contains("panic")
        || lower.contains("stack trace")
        || lower.contains("why does")
        || lower.contains("not working")
    {
        return TaskCategory::BugDebug;
    }

    if lower.contains("refactor")
        || lower.contains("rename")
        || lower.contains("clean up")
        || lower.contains("extract")
        || lower.contains("simplify")
        || lower.contains("restructure")
    {
        return TaskCategory::Refactor;
    }

    if lower.contains("review")
        || lower.contains("audit")
        || lower.contains("pr ")
        || lower.contains("look over")
        || lower.contains("feedback")
    {
        return TaskCategory::CodeReview;
    }

    if lower.contains("design")
        || lower.contains("architecture")
        || lower.contains("should i")
        || lower.contains("approach")
        || lower.contains("trade-off")
        || lower.contains("tradeoff")
        || lower.contains("plan")
    {
        return TaskCategory::Architecture;
    }

    if lower.contains("document")
        || lower.contains("docstring")
        || lower.contains("comment")
        || lower.contains("readme")
        || lower.contains("explain how to use")
    {
        return TaskCategory::Documentation;
    }

    if lower.contains("explain")
        || lower.contains("what is")
        || lower.contains("what does")
        || lower.contains("how does")
        || lower.contains("understand")
    {
        return TaskCategory::Explanation;
    }

    if lower.contains("research")
        || lower.contains("compare")
        || lower.contains("investigate")
        || lower.contains("options for")
    {
        return TaskCategory::Research;
    }

    if lower.contains("typo")
        || lower.contains("one-line")
        || lower.contains("one line")
        || lower.contains("small change")
        || lower.contains("quick")
        || lower.contains("tweak")
        || lower.contains("add a")
    {
        return TaskCategory::QuickEdit;
    }

    TaskCategory::Implementation
}

// ── Route plan types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RouteChoice {
    pub adapter_id: String,
    /// Routed model ID — None means adapter picks its default
    pub model: Option<String>,
    /// Human-friendly model name for --dry-run output
    pub model_display: String,
    pub score: i32,
    pub cooling_down: bool,
}

#[derive(Debug, Clone)]
pub struct RoutePlan {
    pub category: TaskCategory,
    pub ranked: Vec<RouteChoice>,
}

// ── Router ────────────────────────────────────────────────────────────────────

pub struct Router;

impl Router {
    /// Build a ranked route plan.
    ///
    /// - `forced_tool`  → single-entry plan (no fallback candidates).
    /// - `forced_model` → overrides model selection for every choice.
    /// - `is_cooling_down` → closure `|adapter_id| -> bool`; testable without a real DB.
    ///
    /// Scoring per choice:
    ///   +100   if any adapter model lists the category in `strengths`
    ///   -N     where N = chosen model's `cost_weight` (conserve expensive quota)
    ///   -10000 if cooling down (demoted but not removed — orchestrator still skips)
    ///
    /// Stable-sort descending by score; `DEFAULT_CHAIN` order is the tiebreak.
    pub fn plan(
        prompt: &str,
        forced_tool: Option<&str>,
        forced_model: Option<&str>,
        is_cooling_down: impl Fn(&str) -> bool,
    ) -> RoutePlan {
        let category = classify_prompt(prompt);

        let candidates: Vec<&str> = match forced_tool {
            Some(tool) => vec![tool],
            None => DEFAULT_CHAIN.to_vec(),
        };

        let mut choices: Vec<RouteChoice> = candidates
            .iter()
            .map(|&id| {
                let cooling = is_cooling_down(id);

                let (model_id, model_display, model_cost, has_strength_match) =
                    if let Some(m) = forced_model {
                        // User forced a model — skip strength analysis
                        (Some(m.to_string()), m.to_string(), 0i32, false)
                    } else if let Some(adapter) = adapter::by_id(id) {
                        let models = adapter.models();
                        if models.is_empty() {
                            (None, "<adapter default>".to_string(), 0, false)
                        } else {
                            let has_match =
                                models.iter().any(|m| m.strengths.contains(&category));

                            // Prefer models that match the category; among those, cheapest.
                            // If no match, fall back to the cheapest model overall.
                            let best = if has_match {
                                models
                                    .iter()
                                    .filter(|m| m.strengths.contains(&category))
                                    .min_by_key(|m| m.cost_weight)
                                    .unwrap()
                            } else {
                                models.iter().min_by_key(|m| m.cost_weight).unwrap()
                            };

                            (
                                Some(best.id.clone()),
                                best.display_name.clone(),
                                best.cost_weight as i32,
                                has_match,
                            )
                        }
                    } else {
                        // Unknown adapter — orchestrator will reject it later
                        (None, "<adapter default>".to_string(), 0, false)
                    };

                let strength_bonus = if has_strength_match { 100 } else { 0 };
                let cooldown_penalty = if cooling { -10_000 } else { 0 };
                let score = strength_bonus - model_cost + cooldown_penalty;

                RouteChoice {
                    adapter_id: id.to_string(),
                    model: model_id,
                    model_display,
                    score,
                    cooling_down: cooling,
                }
            })
            .collect();

        // Stable-sort descending; insertion order (DEFAULT_CHAIN) breaks ties.
        choices.sort_by(|a, b| b.score.cmp(&a.score));

        RoutePlan {
            category,
            ranked: choices,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::TaskCategory;

    // ── classify_prompt ───────────────────────────────────────────────────────

    #[test]
    fn classify_bug_debug() {
        assert_eq!(classify_prompt("fix the panic in parse_args"), TaskCategory::BugDebug);
        assert_eq!(classify_prompt("there is a bug in my loop"), TaskCategory::BugDebug);
        assert_eq!(classify_prompt("why does it crash on startup"), TaskCategory::BugDebug);
        assert_eq!(classify_prompt("this is not working"), TaskCategory::BugDebug);
    }

    #[test]
    fn classify_refactor() {
        assert_eq!(classify_prompt("refactor the auth module"), TaskCategory::Refactor);
        assert_eq!(classify_prompt("clean up this function"), TaskCategory::Refactor);
        assert_eq!(classify_prompt("extract the logging into its own file"), TaskCategory::Refactor);
        assert_eq!(classify_prompt("simplify this match statement"), TaskCategory::Refactor);
    }

    #[test]
    fn classify_code_review() {
        assert_eq!(classify_prompt("review this PR"), TaskCategory::CodeReview);
        assert_eq!(classify_prompt("audit the auth code"), TaskCategory::CodeReview);
        assert_eq!(classify_prompt("look over my changes"), TaskCategory::CodeReview);
        assert_eq!(classify_prompt("give feedback on this implementation"), TaskCategory::CodeReview);
    }

    #[test]
    fn classify_architecture() {
        assert_eq!(classify_prompt("design the caching layer"), TaskCategory::Architecture);
        assert_eq!(classify_prompt("what approach should i take for the API"), TaskCategory::Architecture);
        assert_eq!(classify_prompt("architecture of the router module"), TaskCategory::Architecture);
    }

    #[test]
    fn classify_documentation() {
        assert_eq!(classify_prompt("add a docstring to this function"), TaskCategory::Documentation);
        assert_eq!(classify_prompt("write readme for the project"), TaskCategory::Documentation);
        assert_eq!(classify_prompt("document the public API"), TaskCategory::Documentation);
    }

    #[test]
    fn classify_explanation() {
        assert_eq!(classify_prompt("explain how the journal works"), TaskCategory::Explanation);
        assert_eq!(classify_prompt("what does this code do"), TaskCategory::Explanation);
        assert_eq!(classify_prompt("how does the quota tracker work"), TaskCategory::Explanation);
        assert_eq!(classify_prompt("what is a trait object"), TaskCategory::Explanation);
    }

    #[test]
    fn classify_research() {
        assert_eq!(classify_prompt("research options for vector stores"), TaskCategory::Research);
        assert_eq!(classify_prompt("compare sled vs rocksdb"), TaskCategory::Research);
        assert_eq!(classify_prompt("investigate the performance of lancedb"), TaskCategory::Research);
    }

    #[test]
    fn classify_quick_edit() {
        // "fix a typo" would match BugDebug (first) — use prompts without BugDebug keywords
        assert_eq!(classify_prompt("correct a typo in the display string"), TaskCategory::QuickEdit);
        assert_eq!(classify_prompt("quick tweak to the output format"), TaskCategory::QuickEdit);
        assert_eq!(classify_prompt("add a semicolon at the end"), TaskCategory::QuickEdit);
    }

    #[test]
    fn classify_default_implementation() {
        assert_eq!(classify_prompt("implement the search feature"), TaskCategory::Implementation);
        assert_eq!(classify_prompt("build the new widget for the dashboard"), TaskCategory::Implementation);
        assert_eq!(classify_prompt("write a function that sorts users"), TaskCategory::Implementation);
    }

    // ── Router::plan ──────────────────────────────────────────────────────────

    #[test]
    fn plan_category_set_correctly() {
        let plan = Router::plan("fix the panic in parser", None, None, |_| false);
        assert_eq!(plan.category, TaskCategory::BugDebug);
    }

    #[test]
    fn plan_no_cooldown_strength_match_scores_positive() {
        let plan = Router::plan("fix the panic in parser", None, None, |_| false);
        // At least one adapter should have a positive score (strength match for BugDebug)
        assert!(
            plan.ranked.iter().any(|c| c.score > 0),
            "expected at least one positive-score choice for BugDebug"
        );
        // First choice must be positive (strength match)
        assert!(plan.ranked[0].score > 0);
    }

    #[test]
    fn plan_explanation_claude_first_with_haiku() {
        let plan = Router::plan("explain how the journal works", None, None, |_| false);
        assert_eq!(plan.category, TaskCategory::Explanation);
        // claude-code has Haiku with Explanation strength — uniquely high score
        assert_eq!(plan.ranked[0].adapter_id, "claude-code");
        assert!(
            plan.ranked[0].model_display.contains("Haiku"),
            "expected Haiku for Explanation, got: {}",
            plan.ranked[0].model_display
        );
    }

    #[test]
    fn plan_cooling_down_adapter_sinks_to_bottom() {
        let plan = Router::plan("fix the bug", None, None, |id| id == "claude-code");
        let last = plan.ranked.last().unwrap();
        assert_eq!(last.adapter_id, "claude-code");
        assert!(last.cooling_down);
        assert!(last.score < 0, "cooling-down adapter should have negative score");
    }

    #[test]
    fn plan_forced_tool_yields_single_entry() {
        let plan = Router::plan("do anything", Some("codex"), None, |_| false);
        assert_eq!(plan.ranked.len(), 1);
        assert_eq!(plan.ranked[0].adapter_id, "codex");
    }

    #[test]
    fn plan_forced_model_overrides_all_entries() {
        let plan = Router::plan("explain this", None, Some("my-custom-model"), |_| false);
        for choice in &plan.ranked {
            assert_eq!(
                choice.model.as_deref(),
                Some("my-custom-model"),
                "adapter {} should use forced model",
                choice.adapter_id
            );
            assert_eq!(choice.model_display, "my-custom-model");
        }
    }

    #[test]
    fn plan_all_adapters_covered() {
        let plan = Router::plan("implement a thing", None, None, |_| false);
        // Default chain has 4 adapters
        assert_eq!(plan.ranked.len(), 4);
    }
}
