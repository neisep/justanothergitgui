//! What the application remembers about a worktree, beyond what git reports.
//!
//! [`crate::shared::worktrees`] is git's answer to "which checkouts exist and
//! what is dirty". This module is the app's answer to "what is this checkout
//! *for*" — the task being attempted, who is working on it, where it started,
//! and how far review and tests have got.
//!
//! Nothing here is derived from the repository and nothing here runs: every
//! field is set by the user, except [`WorktreeMetadata::base_commit`], which the
//! app fills in when it creates a worktree. A later phase that actually runs
//! agents writes into these same fields rather than adding its own.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::shared::worktrees::LinkedWorktree;

/// Longest task text kept. Free text reaches a 300px sidebar and a tooltip, and
/// a pasted essay would wreck both; the cap is generous enough never to be met
/// by a real task description.
pub const MAX_TASK_LEN: usize = 512;
/// Longest agent name kept.
pub const MAX_AGENT_LEN: usize = 128;
/// Longest free-form note kept.
///
/// Four times the task's cap: notes are prose, and they reach a resizable
/// right-hand panel that scrolls, not a 300px sidebar row.
pub const MAX_NOTES_LEN: usize = 2048;

/// Metadata for every worktree of one repository, keyed by worktree name —
/// git's own stable handle, the same key [`crate::shared::worktrees::LinkedWorktree::name`]
/// carries.
///
/// A `BTreeMap` rather than a `HashMap` so the persisted file has a stable key
/// order and does not churn on every write.
pub type WorktreeMetadataMap = BTreeMap<String, WorktreeMetadata>;

/// How one worktree is named inside [`WorktreeMetadataMap`].
///
/// Linked worktrees are prefixed so they can never collide with the main
/// worktree, whose "name" is only its directory name — a linked worktree may
/// legitimately be called the same thing, and nothing in git forbids it.
pub fn storage_key(worktree: &LinkedWorktree) -> String {
    if worktree.is_main {
        "main:".to_string()
    } else {
        format!("wt:{}", worktree.name)
    }
}

/// Every field carries `#[serde(default)]`: this codebase has no version field
/// anywhere and handles format evolution purely by defaulting absent fields,
/// the way [`crate::settings::AppSettings`] does.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeMetadata {
    /// What is being attempted in this worktree, in the user's own words.
    #[serde(default)]
    pub task: String,
    /// Who or what is working on it. Free text on purpose — nothing validates or
    /// launches it, so no tool vocabulary is committed to before it has to be.
    #[serde(default)]
    pub agent: String,
    /// Full oid the worktree started from, recorded when the app created it.
    /// Empty for a worktree created outside the app, which the app cannot know
    /// the base of.
    #[serde(default)]
    pub base_commit: String,
    /// Whatever else the user wants to remember about this worktree, in prose.
    ///
    /// The states above answer "how far has this got"; this answers everything
    /// they cannot. Kept out of [`Self::task`] so the one-line summary the
    /// sidebar shows stays a summary.
    #[serde(default)]
    pub notes: String,
    /// Unix seconds the app created this worktree, or `0` when it did not.
    ///
    /// Stored as an instant rather than a formatted string so it can be shown
    /// relative to now ("3h ago") however long the worktree lives. `0` rather
    /// than `Option` because that is what an absent field defaults to, and the
    /// distinction "created before this field existed" is not worth a variant.
    #[serde(default)]
    pub started: i64,
    #[serde(default)]
    pub review: ReviewState,
    #[serde(default)]
    pub test: TestState,
}

impl WorktreeMetadata {
    /// Whether every field is still at its default.
    ///
    /// Such an entry carries no information, so it is neither persisted nor
    /// given a second line in the worktree list.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// The short form of [`Self::base_commit`] for display, or `None` when the
    /// app did not create this worktree.
    ///
    /// Cuts on a character boundary rather than a byte index: the value comes
    /// back off disk and a hand-edited non-ASCII one would otherwise panic.
    pub fn short_base_commit(&self) -> Option<&str> {
        let commit = self.base_commit.trim();
        if commit.is_empty() {
            return None;
        }

        let end = commit
            .char_indices()
            .nth(7)
            .map(|(index, _)| index)
            .unwrap_or(commit.len());
        Some(&commit[..end])
    }

    /// Trim the free-text fields to what the UI can carry. Applied on the way
    /// in, so nothing oversized is ever stored or rendered.
    pub fn clamped(mut self) -> Self {
        self.task = clamp_text(self.task, MAX_TASK_LEN);
        self.agent = clamp_text(self.agent, MAX_AGENT_LEN);
        self.notes = clamp_text(self.notes, MAX_NOTES_LEN);
        self
    }
}

/// Truncate on a character boundary, never mid-codepoint.
fn clamp_text(mut text: String, max: usize) -> String {
    if text.len() <= max {
        return text;
    }

    let end = text
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= max)
        .last()
        .unwrap_or(0);
    text.truncate(end);
    text
}

/// How far human review of this worktree's work has got.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    #[default]
    Unreviewed,
    NeedsReview,
    ChangesRequested,
    Approved,
}

impl ReviewState {
    /// Every variant, in the order the dialog offers them.
    pub const ALL: [Self; 4] = [
        Self::Unreviewed,
        Self::NeedsReview,
        Self::ChangesRequested,
        Self::Approved,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Unreviewed => "Unreviewed",
            Self::NeedsReview => "Needs review",
            Self::ChangesRequested => "Changes requested",
            Self::Approved => "Approved",
        }
    }

    /// Whether this state is worth a chip in the list. The default says nothing,
    /// so it earns no room in a 300px sidebar.
    pub fn is_noteworthy(self) -> bool {
        self != Self::Unreviewed
    }
}

/// What the tests for this worktree's work last reported. Nothing here runs
/// them; the user says what they saw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TestState {
    #[default]
    Unknown,
    Passing,
    Failing,
}

impl TestState {
    pub const ALL: [Self; 3] = [Self::Unknown, Self::Passing, Self::Failing];

    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Passing => "Passing",
            Self::Failing => "Failing",
        }
    }

    pub fn is_noteworthy(self) -> bool {
        self != Self::Unknown
    }
}

/// Both states deserialize by hand so an unrecognised value falls back to the
/// default instead of failing.
///
/// One file holds every repository, and the loader treats a parse error as
/// fatal — so a state written by a later build must not be able to make every
/// repository's notes unreadable at once. `#[serde(other)]` cannot express this:
/// serde only allows it on internally or adjacently tagged enums.
fn deserialize_state<'de, D, T>(
    deserializer: D,
    parse: fn(&str) -> Option<T>,
) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default,
{
    let raw = String::deserialize(deserializer)?;
    Ok(parse(&raw).unwrap_or_default())
}

impl<'de> Deserialize<'de> for ReviewState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize_state(deserializer, |raw| match raw {
            "unreviewed" => Some(Self::Unreviewed),
            "needs_review" => Some(Self::NeedsReview),
            "changes_requested" => Some(Self::ChangesRequested),
            "approved" => Some(Self::Approved),
            _ => None,
        })
    }
}

impl<'de> Deserialize<'de> for TestState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize_state(deserializer, |raw| match raw {
            "unknown" => Some(Self::Unknown),
            "passing" => Some(Self::Passing),
            "failing" => Some(Self::Failing),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::worktrees::LinkedWorktreeStatus;
    use std::path::PathBuf;

    fn worktree(name: &str, is_main: bool) -> LinkedWorktree {
        LinkedWorktree {
            name: name.into(),
            path: PathBuf::from("/tmp").join(name),
            branch: None,
            head_short_oid: String::new(),
            is_main,
            is_current: false,
            is_locked: false,
            lock_reason: None,
            status: LinkedWorktreeStatus::Clean,
        }
    }

    /// A linked worktree may be called the same thing as the repository folder,
    /// which is the only name the main worktree has.
    #[test]
    fn the_main_worktree_can_never_collide_with_a_linked_one() {
        assert_ne!(
            storage_key(&worktree("myapp", true)),
            storage_key(&worktree("myapp", false))
        );
        assert_eq!(storage_key(&worktree("anything", true)), "main:");
        assert_eq!(
            storage_key(&worktree("feature-auth", false)),
            "wt:feature-auth"
        );
    }

    /// A state a later build invented must cost one field, not the whole file.
    #[test]
    fn an_unrecognised_state_falls_back_instead_of_failing_the_parse() {
        let metadata: WorktreeMetadata =
            serde_json::from_str(r#"{"task":"kept","review":"merged","test":"flaky"}"#)
                .expect("an unknown state must not fail the parse");

        assert_eq!(metadata.task, "kept");
        assert_eq!(metadata.review, ReviewState::Unreviewed);
        assert_eq!(metadata.test, TestState::Unknown);
    }

    #[test]
    fn oversized_free_text_is_trimmed_on_a_character_boundary() {
        let metadata = WorktreeMetadata {
            task: "å".repeat(MAX_TASK_LEN),
            agent: "b".repeat(MAX_AGENT_LEN * 2),
            notes: "ö".repeat(MAX_NOTES_LEN),
            ..WorktreeMetadata::default()
        }
        .clamped();

        assert!(metadata.task.len() <= MAX_TASK_LEN);
        assert!(metadata.agent.len() <= MAX_AGENT_LEN);
        assert!(metadata.notes.len() <= MAX_NOTES_LEN);
        assert!(metadata.notes.chars().all(|c| c == 'ö'));
        // Trimming mid-codepoint would have panicked or produced invalid UTF-8.
        assert!(metadata.task.chars().all(|c| c == 'å'));
    }

    #[test]
    fn a_non_ascii_base_commit_does_not_panic() {
        let metadata = WorktreeMetadata {
            base_commit: "åäöåäöåäö".into(),
            ..WorktreeMetadata::default()
        };

        assert_eq!(metadata.short_base_commit(), Some("åäöåäöå"));
    }

    #[test]
    fn an_entry_is_empty_only_while_every_field_is_default() {
        let mut metadata = WorktreeMetadata::default();
        assert!(metadata.is_empty());

        metadata.task = "Add OAuth login".into();
        assert!(!metadata.is_empty());

        metadata = WorktreeMetadata::default();
        metadata.review = ReviewState::NeedsReview;
        assert!(!metadata.is_empty());

        metadata = WorktreeMetadata::default();
        metadata.test = TestState::Failing;
        assert!(!metadata.is_empty());

        metadata = WorktreeMetadata::default();
        metadata.notes = "worth remembering".into();
        assert!(!metadata.is_empty());

        metadata = WorktreeMetadata::default();
        metadata.started = 1_700_000_000;
        assert!(!metadata.is_empty());
    }

    /// The stored strings are part of the file format: changing them silently
    /// drops every saved state back to its default.
    #[test]
    fn states_serialize_as_snake_case_strings() {
        assert_eq!(
            serde_json::to_string(&ReviewState::NeedsReview).expect("serialize"),
            "\"needs_review\""
        );
        assert_eq!(
            serde_json::to_string(&ReviewState::ChangesRequested).expect("serialize"),
            "\"changes_requested\""
        );
        assert_eq!(
            serde_json::to_string(&TestState::Unknown).expect("serialize"),
            "\"unknown\""
        );
        assert_eq!(
            serde_json::from_str::<ReviewState>("\"approved\"").expect("deserialize"),
            ReviewState::Approved
        );
        assert_eq!(
            serde_json::from_str::<TestState>("\"failing\"").expect("deserialize"),
            TestState::Failing
        );
    }

    #[test]
    fn absent_fields_fall_back_to_defaults() {
        let metadata: WorktreeMetadata =
            serde_json::from_str(r#"{"task":"only a task"}"#).expect("deserialize");

        assert_eq!(metadata.task, "only a task");
        assert!(metadata.agent.is_empty());
        assert_eq!(metadata.review, ReviewState::Unreviewed);
        assert_eq!(metadata.test, TestState::Unknown);
        // Written before these two fields existed, and still readable.
        assert!(metadata.notes.is_empty());
        assert_eq!(metadata.started, 0);
    }

    /// Both new fields have to come back exactly as they went in: `started` is
    /// the only record of when a worktree began, and notes are the user's prose.
    #[test]
    fn notes_and_started_round_trip_through_the_file_format() {
        let original = WorktreeMetadata {
            notes: "Waiting on review.\n\nSecond paragraph.".into(),
            started: 1_700_000_000,
            ..WorktreeMetadata::default()
        };

        let encoded = serde_json::to_string(&original).expect("serialize");
        let decoded: WorktreeMetadata = serde_json::from_str(&encoded).expect("deserialize");

        assert_eq!(decoded, original);
    }

    #[test]
    fn unknown_fields_are_ignored_so_older_builds_can_read_newer_files() {
        let metadata: WorktreeMetadata =
            serde_json::from_str(r#"{"task":"t","future_field":42}"#).expect("deserialize");

        assert_eq!(metadata.task, "t");
    }

    #[test]
    fn the_base_commit_is_shortened_for_display_and_absent_when_unknown() {
        let mut metadata = WorktreeMetadata::default();
        assert_eq!(metadata.short_base_commit(), None);

        metadata.base_commit = "a1b2c3d4e5f6a7b8c9d0".into();
        assert_eq!(metadata.short_base_commit(), Some("a1b2c3d"));

        // A short oid must not be truncated past its own length.
        metadata.base_commit = "abc".into();
        assert_eq!(metadata.short_base_commit(), Some("abc"));
    }

    #[test]
    fn only_non_default_states_earn_a_chip() {
        assert!(!ReviewState::Unreviewed.is_noteworthy());
        assert!(ReviewState::Approved.is_noteworthy());
        assert!(!TestState::Unknown.is_noteworthy());
        assert!(TestState::Passing.is_noteworthy());
    }
}
