//! What a worktree review is measured against.
//!
//! Reviewing a worktree means answering "what was done here", which only means
//! anything relative to a starting point. Phase 2 records that point when the
//! app creates a worktree; for every other worktree — and most are created with
//! `git worktree add` in a terminal — it has to be derived, and the reviewer
//! needs to know which of the two they are looking at.

/// The commit a worktree's work is measured from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewBase {
    /// Full oid, so it can be looked up again without re-deriving it.
    pub oid: String,
    pub short_oid: String,
    pub source: ReviewBaseSource,
}

impl ReviewBase {
    /// How the header explains where the base came from.
    pub fn description(&self) -> &'static str {
        match self.source {
            ReviewBaseSource::Recorded => "recorded when the worktree was created",
            ReviewBaseSource::ForkPoint => "fork point from the main worktree",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewBaseSource {
    /// The exact commit the app branched from, out of the worktree's metadata.
    Recorded,
    /// Derived: where this worktree's branch diverged from the main worktree.
    ///
    /// Self-correcting in the sense that it is always the real divergence, but
    /// it is a point and not a branch — it does not move as the main worktree
    /// advances.
    ForkPoint,
}

/// How much a worktree has changed since its base, in one line.
///
/// Distinct from [`crate::shared::worktrees::LinkedWorktreeStatus`], which
/// counts *uncommitted* files in a checkout. This counts everything since the
/// base commit — committed, staged and unstaged together — which is the number
/// a reviewer wants and the other is not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReviewSummary {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}

impl ReviewSummary {
    /// `"8 files, +120 -45"`, or a plain sentence when nothing differs.
    ///
    /// Plus and minus are ASCII on purpose: the bundled fonts have no `\u{2212}`
    /// (the true minus sign), and a missing glyph paints a box that the headless
    /// tests — which scrape painted *text* — would happily accept.
    pub fn label(&self) -> String {
        if self.is_empty() {
            return "No changes since its base".into();
        }

        format!(
            "{} file{}, +{} -{}",
            self.files_changed,
            if self.files_changed == 1 { "" } else { "s" },
            self.insertions,
            self.deletions
        )
    }

    pub fn is_empty(&self) -> bool {
        self.files_changed == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_says_which_base_it_is() {
        let recorded = ReviewBase {
            oid: "a".repeat(40),
            short_oid: "aaaaaaa".into(),
            source: ReviewBaseSource::Recorded,
        };
        let derived = ReviewBase {
            source: ReviewBaseSource::ForkPoint,
            ..recorded.clone()
        };

        assert_ne!(recorded.description(), derived.description());
        assert!(derived.description().contains("fork point"));
    }

    #[test]
    fn the_summary_reads_as_one_line() {
        let many = ReviewSummary {
            files_changed: 8,
            insertions: 120,
            deletions: 45,
        };
        assert_eq!(many.label(), "8 files, +120 -45");

        let one = ReviewSummary {
            files_changed: 1,
            insertions: 3,
            deletions: 0,
        };
        assert_eq!(one.label(), "1 file, +3 -0");

        let none = ReviewSummary::default();
        assert!(none.is_empty());
        assert_eq!(none.label(), "No changes since its base");
    }

    /// A missing glyph renders as a box the painted-text tests would accept, so
    /// the signs have to stay inside ASCII.
    #[test]
    fn the_summary_uses_no_characters_the_bundled_fonts_lack() {
        let summary = ReviewSummary {
            files_changed: 2,
            insertions: 1,
            deletions: 1,
        };
        assert!(summary.label().is_ascii());
    }
}
