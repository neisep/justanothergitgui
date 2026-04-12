use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

const CONVENTIONAL_COMMIT_PREFIXES: [&str; 11] = [
    "build: ",
    "chore: ",
    "ci: ",
    "docs: ",
    "feat: ",
    "fix: ",
    "perf: ",
    "refactor: ",
    "revert: ",
    "style: ",
    "test: ",
];

const CONVENTIONAL_COMMIT_TYPES: [&str; 11] = [
    "build", "chore", "ci", "docs", "feat", "fix", "perf", "refactor", "revert", "style", "test",
];

static CONVENTIONAL_COMMITS_SUBJECT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(build|chore|ci|docs|feat|fix|perf|refactor|style|test|revert)(\([^)]+\))?(!)?: \S.*$",
    )
    .expect("valid conventional commits regex")
});

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitMessageRuleSet {
    #[default]
    Off,
    ConventionalCommits,
}

impl CommitMessageRuleSet {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::ConventionalCommits => "Conventional Commits",
        }
    }

    pub fn description(self) -> Option<&'static str> {
        match self {
            Self::Off => None,
            Self::ConventionalCommits => Some(
                "Require the first line to look like `feat: add search` or `fix(parser): handle empty input`.",
            ),
        }
    }

    pub fn prefixes(self) -> &'static [&'static str] {
        match self {
            Self::Off => &[],
            Self::ConventionalCommits => &CONVENTIONAL_COMMIT_PREFIXES,
        }
    }
}

pub fn default_initial_commit_message(ruleset: CommitMessageRuleSet) -> &'static str {
    match ruleset {
        CommitMessageRuleSet::Off => "Initial commit",
        CommitMessageRuleSet::ConventionalCommits => "chore: initial commit",
    }
}

pub fn validation_error(ruleset: CommitMessageRuleSet, message: &str) -> Option<String> {
    if message.trim().is_empty() {
        return None;
    }

    validate_for_submit(ruleset, message).err()
}

pub fn validate_for_submit(ruleset: CommitMessageRuleSet, message: &str) -> Result<(), String> {
    match ruleset {
        CommitMessageRuleSet::Off => Ok(()),
        CommitMessageRuleSet::ConventionalCommits => validate_conventional_commit(message),
    }
}

pub fn apply_prefix(ruleset: CommitMessageRuleSet, message: &mut String, prefix: &str) {
    match ruleset {
        CommitMessageRuleSet::Off => {}
        CommitMessageRuleSet::ConventionalCommits => apply_conventional_prefix(message, prefix),
    }
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
    if !CONVENTIONAL_COMMIT_PREFIXES.contains(&prefix) {
        return;
    }

    let (subject, body) = match message.split_once('\n') {
        Some((subject, body)) => (subject, Some(body)),
        None => (message.as_str(), None),
    };

    let subject = subject.trim_start();
    let remainder = conventional_subject_remainder(subject)
        .unwrap_or(subject)
        .trim_start();

    let mut new_subject = prefix.to_string();
    if !remainder.is_empty() {
        new_subject.push_str(remainder);
    }

    *message = match body {
        Some(body) => format!("{}\n{}", new_subject, body),
        None => new_subject,
    };
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
