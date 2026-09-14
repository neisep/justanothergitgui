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
}
