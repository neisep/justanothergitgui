use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

const CONVENTIONAL_COMMIT_TYPES: [&str; 11] = [
    "build", "chore", "ci", "docs", "feat", "fix", "perf", "refactor", "revert", "style", "test",
];

const GENERIC_SCOPE_DIRS: [&str; 10] = [
    ".github", ".vscode", "benches", "docs", "examples", "src", "target", "test", "tests", "vendor",
];

static CONVENTIONAL_COMMITS_SUBJECT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(build|chore|ci|docs|feat|fix|perf|refactor|style|test|revert)(\([^)]+\))?(!)?: \S.*$",
    )
    .expect("valid conventional commits regex")
});

static CUSTOM_SCOPE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._/-]*$").expect("valid custom scope regex")
});

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitMessageRuleSet {
    #[default]
    Off,
    ConventionalCommits,
}

pub trait CommitMessageRules: Send + Sync {
    fn display_name(&self) -> &'static str;
    fn description(&self) -> Option<&'static str>;
    fn default_initial_summary(&self) -> &'static str;
    fn validate(&self, message: &str) -> Result<(), String>;
    fn prefix_suggestions(
        &self,
        message: &str,
        inferred_scopes: &[String],
        custom_scopes: &[String],
    ) -> Vec<String>;
    fn apply_prefix(&self, message: &mut String, prefix: &str);
}

#[derive(Clone, Copy, Debug, Default)]
struct OffCommitMessageRules;

#[derive(Clone, Copy, Debug, Default)]
struct ConventionalCommitMessageRules;

static OFF_RULES: OffCommitMessageRules = OffCommitMessageRules;
static CONVENTIONAL_COMMITS_RULES: ConventionalCommitMessageRules = ConventionalCommitMessageRules;

impl CommitMessageRuleSet {
    pub fn runtime(self) -> &'static dyn CommitMessageRules {
        match self {
            Self::Off => &OFF_RULES,
            Self::ConventionalCommits => &CONVENTIONAL_COMMITS_RULES,
        }
    }

    pub fn display_name(self) -> &'static str {
        self.runtime().display_name()
    }

    pub fn description(self) -> Option<&'static str> {
        self.runtime().description()
    }
}

impl CommitMessageRules for OffCommitMessageRules {
    fn display_name(&self) -> &'static str {
        "Off"
    }

    fn description(&self) -> Option<&'static str> {
        None
    }

    fn default_initial_summary(&self) -> &'static str {
        "Initial commit"
    }

    fn validate(&self, _message: &str) -> Result<(), String> {
        Ok(())
    }

    fn prefix_suggestions(
        &self,
        _message: &str,
        _inferred_scopes: &[String],
        _custom_scopes: &[String],
    ) -> Vec<String> {
        Vec::new()
    }

    fn apply_prefix(&self, _message: &mut String, _prefix: &str) {}
}

impl CommitMessageRules for ConventionalCommitMessageRules {
    fn display_name(&self) -> &'static str {
        "Conventional Commits"
    }

    fn description(&self) -> Option<&'static str> {
        Some(
            "Require the first line to look like `feat: add search` or `fix(parser): handle empty input`.",
        )
    }

    fn default_initial_summary(&self) -> &'static str {
        "chore: initial commit"
    }

    fn validate(&self, message: &str) -> Result<(), String> {
        validate_conventional_commit(message)
    }

    fn prefix_suggestions(
        &self,
        message: &str,
        inferred_scopes: &[String],
        custom_scopes: &[String],
    ) -> Vec<String> {
        conventional_prefix_suggestions(message, inferred_scopes, custom_scopes)
    }

    fn apply_prefix(&self, message: &mut String, prefix: &str) {
        apply_conventional_prefix(message, prefix);
    }
}

pub fn default_initial_commit_summary(ruleset: CommitMessageRuleSet) -> &'static str {
    ruleset.runtime().default_initial_summary()
}

pub fn build_message(summary: &str, body: &str) -> String {
    let summary = summary.trim();
    let body = body.trim();

    match (summary.is_empty(), body.is_empty()) {
        (true, true) => String::new(),
        (false, true) => summary.to_string(),
        (true, false) => body.to_string(),
        (false, false) => format!("{summary}\n\n{body}"),
    }
}

pub fn validation_error(ruleset: CommitMessageRuleSet, message: &str) -> Option<String> {
    if message.trim().is_empty() {
        return None;
    }

    validate_for_submit(ruleset, message).err()
}

pub fn validate_for_submit(ruleset: CommitMessageRuleSet, message: &str) -> Result<(), String> {
    ruleset.runtime().validate(message)
}

pub fn apply_prefix(ruleset: CommitMessageRuleSet, message: &mut String, prefix: &str) {
    ruleset.runtime().apply_prefix(message, prefix)
}

pub fn parse_custom_scopes(input: &str) -> Result<Vec<String>, String> {
    let mut scopes = Vec::new();

    for raw_scope in input.split(',') {
        let scope = raw_scope.trim();
        if scope.is_empty() {
            continue;
        }

        if !CUSTOM_SCOPE_REGEX.is_match(scope) {
            return Err(format!(
                "Custom scopes must use letters, numbers, `.`, `_`, `-`, or `/`: `{}`",
                scope
            ));
        }

        push_unique(&mut scopes, scope.to_string());
    }

    Ok(scopes)
}

pub fn infer_commit_scopes<'a>(
    repo_root: Option<&Path>,
    changed_paths: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    let mut scope_counts = HashMap::new();

    for changed_path in changed_paths {
        if let Some(scope) = infer_scope_from_path(repo_root, changed_path) {
            *scope_counts.entry(scope).or_insert(0usize) += 1;
        }
    }

    let mut ranked_scopes = scope_counts.into_iter().collect::<Vec<_>>();
    ranked_scopes.sort_by(|(left_scope, left_count), (right_scope, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_scope.cmp(right_scope))
    });

    let mut scopes = ranked_scopes
        .into_iter()
        .map(|(scope, _)| scope)
        .collect::<Vec<_>>();

    if scopes.is_empty()
        && let Some(default_scope) = repo_default_scope(repo_root)
    {
        scopes.push(default_scope);
    }

    scopes
}

pub fn prefix_suggestions(
    ruleset: CommitMessageRuleSet,
    message: &str,
    inferred_scopes: &[String],
    custom_scopes: &[String],
) -> Vec<String> {
    ruleset
        .runtime()
        .prefix_suggestions(message, inferred_scopes, custom_scopes)
}

fn validate_conventional_commit(message: &str) -> Result<(), String> {
    let subject = message.lines().next().unwrap_or_default().trim_end();

    if CONVENTIONAL_COMMITS_SUBJECT_REGEX.is_match(subject) {
        Ok(())
    } else if conventional_subject_remainder(subject)
        .is_some_and(|remainder| remainder.trim().is_empty())
    {
        Err("Add a short summary after the commit type prefix.".into())
    } else {
        Err(
            "Commit message must start with a Conventional Commits prefix like `feat: add search` or `fix(parser): handle empty input`."
                .into(),
        )
    }
}

fn apply_conventional_prefix(message: &mut String, prefix: &str) {
    if !is_valid_conventional_prefix(prefix) {
        return;
    }

    let (subject, body) = match message.split_once('\n') {
        Some((subject, body)) => (subject, Some(body)),
        None => (message.as_str(), None),
    };

    let subject = subject.trim_start();
    let remainder = if prefix_matches_subject(prefix, subject) {
        ""
    } else {
        conventional_subject_remainder(subject)
            .unwrap_or(subject)
            .trim_start()
    };

    let mut new_subject = prefix.to_string();
    if !remainder.is_empty() {
        new_subject.push_str(remainder);
    }

    *message = match body {
        Some(body) => format!("{}\n{}", new_subject, body),
        None => new_subject,
    };
}

fn conventional_prefix_suggestions(
    message: &str,
    inferred_scopes: &[String],
    custom_scopes: &[String],
) -> Vec<String> {
    let subject = message.lines().next().unwrap_or_default().trim_start();
    if subject.is_empty() {
        return Vec::new();
    }

    let mut suggestions = Vec::new();
    for commit_type in CONVENTIONAL_COMMIT_TYPES
        .iter()
        .copied()
        .filter(|commit_type| subject.starts_with(commit_type) || commit_type.starts_with(subject))
    {
        for candidate in suggestion_candidates_for_type(commit_type, inferred_scopes, custom_scopes)
        {
            if prefix_matches_subject(&candidate, subject) {
                push_unique(&mut suggestions, candidate);
            }
        }
    }

    suggestions
}

fn suggestion_candidates_for_type(
    commit_type: &str,
    inferred_scopes: &[String],
    custom_scopes: &[String],
) -> Vec<String> {
    let mut candidates = vec![format!("{commit_type}: ")];
    let ranked_inferred_scopes = inferred_scopes.iter().take(3).cloned().collect::<Vec<_>>();

    if ranked_inferred_scopes.len() > 1 {
        candidates.push(format!(
            "{}({}): ",
            commit_type,
            ranked_inferred_scopes.join(",")
        ));
    }

    for scope in ranked_inferred_scopes {
        candidates.push(format!("{commit_type}({scope}): "));
    }

    for scope in custom_scopes {
        candidates.push(format!("{commit_type}({scope}): "));
    }

    let mut deduped = Vec::new();
    for candidate in candidates {
        push_unique(&mut deduped, candidate);
    }
    deduped
}

fn prefix_matches_subject(candidate: &str, subject: &str) -> bool {
    let trimmed_candidate = candidate.trim_end();
    candidate.starts_with(subject) || trimmed_candidate.starts_with(subject)
}

fn is_valid_conventional_prefix(prefix: &str) -> bool {
    if !prefix.ends_with(": ") {
        return false;
    }

    let probe = format!("{prefix}summary");
    CONVENTIONAL_COMMITS_SUBJECT_REGEX.is_match(&probe)
}

fn infer_scope_from_path(repo_root: Option<&Path>, changed_path: &str) -> Option<String> {
    let components = Path::new(changed_path)
        .components()
        .filter_map(component_name)
        .collect::<Vec<_>>();

    if components.is_empty() {
        return repo_default_scope(repo_root);
    }

    if let Some(marker_index) = components
        .iter()
        .position(|component| is_generic_scope_dir(component))
    {
        if let Some(scope) = components[..marker_index]
            .iter()
            .rev()
            .find(|component| !is_generic_scope_dir(component))
        {
            return Some((*scope).to_string());
        }

        if let Some(scope) = components[marker_index + 1..components.len().saturating_sub(1)]
            .iter()
            .find(|component| !is_generic_scope_dir(component))
        {
            return Some((*scope).to_string());
        }
    }

    if let Some(scope) = components
        .iter()
        .take(components.len().saturating_sub(1))
        .find(|component| !is_generic_scope_dir(component))
    {
        return Some((*scope).to_string());
    }

    repo_default_scope(repo_root)
}

fn component_name(component: Component<'_>) -> Option<&str> {
    match component {
        Component::Normal(name) => name.to_str(),
        _ => None,
    }
}

fn is_generic_scope_dir(component: &str) -> bool {
    GENERIC_SCOPE_DIRS.contains(&component)
}

fn repo_default_scope(repo_root: Option<&Path>) -> Option<String> {
    let repo_root = repo_root?;
    cargo_package_name(repo_root.join("Cargo.toml")).or_else(|| {
        repo_root
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
    })
}

fn cargo_package_name(cargo_toml_path: impl AsRef<Path>) -> Option<String> {
    let cargo_toml_path = cargo_toml_path.as_ref();
    let cargo_toml = fs::read_to_string(cargo_toml_path).ok()?;
    let mut in_package_section = false;

    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package_section = trimmed == "[package]";
            continue;
        }

        if !in_package_section {
            continue;
        }

        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };

        if key.trim() != "name" {
            continue;
        }

        let value = value.trim().trim_matches('"');
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }

    None
}

fn conventional_subject_remainder(subject: &str) -> Option<&str> {
    for prefix in CONVENTIONAL_COMMIT_TYPES {
        let Some(rest) = subject.strip_prefix(prefix) else {
            continue;
        };
        let rest = if rest.starts_with('(') {
            let scope_end = rest.find(')')?;
            &rest[scope_end + 1..]
        } else {
            rest
        };
        let rest = rest.strip_prefix('!').unwrap_or(rest);
        let rest = rest.strip_prefix(": ")?;
        return Some(rest);
    }

    None
}

fn push_unique(values: &mut Vec<String>, next_value: String) {
    if !values.iter().any(|value| value == &next_value) {
        values.push(next_value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_custom_scopes_and_dedupes_values() {
        let scopes = parse_custom_scopes("ui, settings, ui, worker/core").unwrap();

        assert_eq!(scopes, vec!["ui", "settings", "worker/core"]);
    }

    #[test]
    fn keeps_ruleset_serialization_compatible() {
        assert_eq!(
            serde_json::to_string(&CommitMessageRuleSet::ConventionalCommits).unwrap(),
            "\"conventional_commits\""
        );
        assert_eq!(
            serde_json::from_str::<CommitMessageRuleSet>("\"off\"").unwrap(),
            CommitMessageRuleSet::Off
        );
    }

    #[test]
    fn resolves_runtime_rules_from_selector() {
        let rules = CommitMessageRuleSet::ConventionalCommits.runtime();

        assert_eq!(rules.display_name(), "Conventional Commits");
        assert_eq!(rules.default_initial_summary(), "chore: initial commit");
    }

    #[test]
    fn rejects_invalid_custom_scope_values() {
        let error = parse_custom_scopes("ui, bad scope").unwrap_err();

        assert!(error.contains("bad scope"));
    }

    #[test]
    fn infers_scopes_from_changed_paths_and_ranks_them() {
        let repo_root = Path::new("/tmp/justanothergitgui");
        let scopes = infer_commit_scopes(
            Some(repo_root),
            [
                "src/ui/commit_panel.rs",
                "src/ui/diff_panel.rs",
                "src/settings.rs",
            ],
        );

        assert_eq!(scopes, vec!["ui", "justanothergitgui"]);
    }

    #[test]
    fn suggests_plain_and_scoped_prefixes_for_partial_subjects() {
        let suggestions = prefix_suggestions(
            CommitMessageRuleSet::ConventionalCommits,
            "fix",
            &["ui".into(), "settings".into()],
            &["worker".into()],
        );

        assert_eq!(
            suggestions,
            vec![
                "fix: ",
                "fix(ui,settings): ",
                "fix(ui): ",
                "fix(settings): ",
                "fix(worker): ",
            ]
        );
    }

    #[test]
    fn falls_back_to_plain_prefix_when_no_scope_matches_exist() {
        let suggestions =
            prefix_suggestions(CommitMessageRuleSet::ConventionalCommits, "fix", &[], &[]);

        assert_eq!(suggestions, vec!["fix: "]);
    }

    #[test]
    fn apply_prefix_replaces_partial_prefix_fragments() {
        let mut message = "fix(".to_string();

        apply_prefix(
            CommitMessageRuleSet::ConventionalCommits,
            &mut message,
            "fix(ui): ",
        );

        assert_eq!(message, "fix(ui): ");
    }

    #[test]
    fn apply_prefix_preserves_existing_summary_text() {
        let mut message = "fix: preserve the summary".to_string();

        apply_prefix(
            CommitMessageRuleSet::ConventionalCommits,
            &mut message,
            "fix(ui): ",
        );

        assert_eq!(message, "fix(ui): preserve the summary");
    }

    #[test]
    fn build_message_joins_summary_and_body_with_blank_line() {
        let message = build_message(
            "feat: add split commit UI",
            "The body explains why.\n\nIt keeps details optional.",
        );

        assert_eq!(
            message,
            "feat: add split commit UI\n\nThe body explains why.\n\nIt keeps details optional."
        );
    }

    #[test]
    fn build_message_omits_body_separator_when_body_is_empty() {
        let message = build_message("feat: add split commit UI", "   ");

        assert_eq!(message, "feat: add split commit UI");
    }
}
