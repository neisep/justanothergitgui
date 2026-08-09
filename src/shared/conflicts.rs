#[derive(Clone, Debug)]
pub enum ConflictPart {
    Common(String),
    Conflict {
        ours: String,
        theirs: String,
        resolution: ConflictChoice,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum ConflictChoice {
    #[default]
    Unresolved,
    Ours,
    Theirs,
    Both,
    /// A hand-typed resolution for this conflict (inline edit).
    Custom(String),
    /// Per-line selection: one flag per entry of [`merge_segments`], `true`
    /// meaning that line is kept in the result. Lets the user cherry-pick
    /// individual lines from either side.
    Picked(Vec<bool>),
}

impl ConflictChoice {
    /// Whether the user has settled this conflict (anything but `Unresolved`).
    pub fn is_resolved(&self) -> bool {
        !matches!(self, ConflictChoice::Unresolved)
    }
}

/// Which side a merged line came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentOrigin {
    /// Present on both sides — always kept.
    Common,
    /// Unique to the current (ours) side.
    Ours,
    /// Unique to the incoming (theirs) side.
    Theirs,
}

/// One line of a conflict, tagged with the side it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct MergeSegment {
    pub origin: SegmentOrigin,
    pub text: String,
}

/// Interleave two sides into a single ordered line sequence via a longest
/// common subsequence, tagging each line as `Common`, `Ours`, or `Theirs`.
/// This is the backbone of per-line conflict picking.
pub fn merge_segments(ours: &str, theirs: &str) -> Vec<MergeSegment> {
    let a: Vec<&str> = ours.lines().collect();
    let b: Vec<&str> = theirs.lines().collect();
    let (n, m) = (a.len(), b.len());

    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            out.push(MergeSegment {
                origin: SegmentOrigin::Common,
                text: a[i].to_string(),
            });
            i += 1;
            j += 1;
        } else if j >= m || (i < n && dp[i + 1][j] >= dp[i][j + 1]) {
            out.push(MergeSegment {
                origin: SegmentOrigin::Ours,
                text: a[i].to_string(),
            });
            i += 1;
        } else {
            out.push(MergeSegment {
                origin: SegmentOrigin::Theirs,
                text: b[j].to_string(),
            });
            j += 1;
        }
    }

    out
}

/// The matched index pairs of a longest common subsequence between two line
/// slices, in increasing order on both sides. Shared backbone of the 2-way
/// [`merge_segments`] and the 3-way [`diff3`].
fn lcs_pairs(a: &[&str], b: &[&str]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut pairs = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

/// Three-way merge of `ours` and `theirs` against their common ancestor
/// `base`, yielding the ordered section list for the merge editor.
///
/// Anchored on lines the base still shares with both sides, each change region
/// between anchors is resolved automatically when only one side touched it (or
/// both made the identical edit) and emitted as [`ConflictPart::Common`]; only
/// regions where the two sides diverge differently become
/// [`ConflictPart::Conflict`]. This is what lets the editor pre-resolve
/// one-sided edits instead of asking the user.
pub fn diff3(base: &str, ours: &str, theirs: &str) -> Vec<ConflictPart> {
    let o: Vec<&str> = base.lines().collect();
    let a: Vec<&str> = ours.lines().collect();
    let b: Vec<&str> = theirs.lines().collect();

    // For each base line, the index it maps to on each side (None if that side
    // dropped it), taken from the per-side longest common subsequence.
    let mut in_a = vec![None; o.len()];
    for (oi, ai) in lcs_pairs(&o, &a) {
        in_a[oi] = Some(ai);
    }
    let mut in_b = vec![None; o.len()];
    for (oi, bi) in lcs_pairs(&o, &b) {
        in_b[oi] = Some(bi);
    }

    let mut parts: Vec<ConflictPart> = Vec::new();
    let mut common: Vec<&str> = Vec::new();
    let (mut oi, mut ai, mut bi) = (0usize, 0usize, 0usize);

    while oi < o.len() || ai < a.len() || bi < b.len() {
        // Sitting on a line all three still agree on: pure common context.
        if oi < o.len() && in_a[oi] == Some(ai) && in_b[oi] == Some(bi) {
            common.push(o[oi]);
            oi += 1;
            ai += 1;
            bi += 1;
            continue;
        }

        // Otherwise collect the change region up to the next line common to all
        // three sides (or the end of every input if none remains).
        let (mut o2, mut a2, mut b2) = (o.len(), a.len(), b.len());
        for k in oi..o.len() {
            if let (Some(ak), Some(bk)) = (in_a[k], in_b[k])
                && ak >= ai
                && bk >= bi
            {
                (o2, a2, b2) = (k, ak, bk);
                break;
            }
        }

        let o_slice = &o[oi..o2];
        let a_slice = &a[ai..a2];
        let b_slice = &b[bi..b2];

        if a_slice == o_slice {
            // Ours left the base untouched here → theirs wins outright.
            common.extend_from_slice(b_slice);
        } else if b_slice == o_slice || a_slice == b_slice {
            // Theirs untouched, or both made the same edit → ours stands in.
            common.extend_from_slice(a_slice);
        } else {
            if !common.is_empty() {
                parts.push(ConflictPart::Common(common.join("\n")));
                common.clear();
            }
            parts.push(ConflictPart::Conflict {
                ours: a_slice.join("\n"),
                theirs: b_slice.join("\n"),
                resolution: ConflictChoice::default(),
            });
        }

        (oi, ai, bi) = (o2, a2, b2);
    }

    if !common.is_empty() {
        parts.push(ConflictPart::Common(common.join("\n")));
    }

    parts
}

/// The keep/drop mask a conflict currently implies over its [`merge_segments`].
/// `Common` lines are always kept; `Picked` returns its stored mask (when the
/// length still matches), and the other choices map to their obvious masks.
fn mask_for(segments: &[MergeSegment], resolution: &ConflictChoice) -> Vec<bool> {
    if let ConflictChoice::Picked(mask) = resolution
        && mask.len() == segments.len()
    {
        return mask.clone();
    }

    segments
        .iter()
        .map(|segment| match segment.origin {
            SegmentOrigin::Common => true,
            SegmentOrigin::Ours => {
                matches!(resolution, ConflictChoice::Ours | ConflictChoice::Both)
            }
            SegmentOrigin::Theirs => {
                matches!(resolution, ConflictChoice::Theirs | ConflictChoice::Both)
            }
        })
        .collect()
}

/// Join the kept lines of a segment list into merged text.
fn compose_picked(segments: &[MergeSegment], mask: &[bool]) -> String {
    segments
        .iter()
        .zip(mask.iter())
        .filter(|(_, keep)| **keep)
        .map(|(segment, _)| segment.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Clone, Debug)]
pub struct ConflictData {
    pub path: String,
    pub sections: Vec<ConflictPart>,
}

impl ConflictData {
    /// Render the sections back into a single merged text.
    ///
    /// Resolved conflicts emit the chosen side(s); an `Unresolved` conflict
    /// emits the raw `<<<<<<<` / `=======` / `>>>>>>>` markers so it stays
    /// visible and editable in the result buffer. Unlike a writer, this never
    /// fails — it is used to seed the editable merge result every frame.
    pub fn compose(&self) -> String {
        let mut content = String::new();

        for (index, section) in self.sections.iter().enumerate() {
            if index > 0 {
                content.push('\n');
            }
            match section {
                ConflictPart::Common(text) => content.push_str(text),
                ConflictPart::Conflict {
                    ours,
                    theirs,
                    resolution,
                    ..
                } => match resolution {
                    ConflictChoice::Ours => content.push_str(ours),
                    ConflictChoice::Theirs => content.push_str(theirs),
                    ConflictChoice::Both => {
                        content.push_str(ours);
                        content.push('\n');
                        content.push_str(theirs);
                    }
                    ConflictChoice::Custom(text) => content.push_str(text),
                    ConflictChoice::Picked(mask) => {
                        let segments = merge_segments(ours, theirs);
                        content.push_str(&compose_picked(&segments, mask));
                    }
                    ConflictChoice::Unresolved => {
                        content.push_str("<<<<<<< Current (ours)\n");
                        content.push_str(ours);
                        content.push_str("\n=======\n");
                        content.push_str(theirs);
                        content.push_str("\n>>>>>>> Incoming (theirs)");
                    }
                },
            }
        }

        content
    }

    /// The line segments of a conflict section plus its current keep/drop mask,
    /// for rendering the per-line picker. `None` if the index is not a conflict.
    pub fn conflict_segments(
        &self,
        section_index: usize,
    ) -> Option<(Vec<MergeSegment>, Vec<bool>)> {
        if let Some(ConflictPart::Conflict {
            ours,
            theirs,
            resolution,
            ..
        }) = self.sections.get(section_index)
        {
            let segments = merge_segments(ours, theirs);
            let mask = mask_for(&segments, resolution);
            Some((segments, mask))
        } else {
            None
        }
    }

    /// Toggle whether one line of a conflict is kept, switching that conflict
    /// into `Picked` mode. `Common` lines are always kept and ignore toggles.
    pub fn toggle_segment(&mut self, section_index: usize, segment_index: usize) {
        if let Some(ConflictPart::Conflict {
            ours,
            theirs,
            resolution,
            ..
        }) = self.sections.get_mut(section_index)
        {
            let segments = merge_segments(ours, theirs);
            let mut mask = mask_for(&segments, resolution);
            if let (Some(segment), Some(flag)) =
                (segments.get(segment_index), mask.get_mut(segment_index))
                && segment.origin != SegmentOrigin::Common
            {
                *flag = !*flag;
            }
            *resolution = ConflictChoice::Picked(mask);
        }
    }

    /// Number of conflict sections still left `Unresolved`.
    pub fn unresolved_count(&self) -> usize {
        self.sections
            .iter()
            .filter(|section| {
                matches!(
                    section,
                    ConflictPart::Conflict {
                        resolution: ConflictChoice::Unresolved,
                        ..
                    }
                )
            })
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::{ConflictChoice, ConflictData, ConflictPart};

    fn conflict(resolution: ConflictChoice) -> ConflictData {
        ConflictData {
            path: "file.txt".into(),
            sections: vec![
                ConflictPart::Common("top".into()),
                ConflictPart::Conflict {
                    ours: "mine".into(),
                    theirs: "yours".into(),
                    resolution,
                },
                ConflictPart::Common("bottom".into()),
            ],
        }
    }

    #[test]
    fn compose_emits_chosen_side_for_resolved_conflicts() {
        assert_eq!(
            conflict(ConflictChoice::Ours).compose(),
            "top\nmine\nbottom"
        );
        assert_eq!(
            conflict(ConflictChoice::Theirs).compose(),
            "top\nyours\nbottom"
        );
        assert_eq!(
            conflict(ConflictChoice::Both).compose(),
            "top\nmine\nyours\nbottom"
        );
    }

    #[test]
    fn compose_keeps_markers_for_unresolved_conflicts() {
        let composed = conflict(ConflictChoice::Unresolved).compose();
        assert!(composed.contains("<<<<<<<"));
        assert!(composed.contains("======="));
        assert!(composed.contains(">>>>>>>"));
        assert!(composed.contains("mine"));
        assert!(composed.contains("yours"));
    }

    #[test]
    fn compose_emits_custom_text_verbatim() {
        assert_eq!(
            conflict(ConflictChoice::Custom("hand edited".into())).compose(),
            "top\nhand edited\nbottom"
        );
    }

    #[test]
    fn unresolved_count_tracks_open_conflicts() {
        assert_eq!(conflict(ConflictChoice::Unresolved).unresolved_count(), 1);
        assert_eq!(conflict(ConflictChoice::Ours).unresolved_count(), 0);
        assert_eq!(
            conflict(ConflictChoice::Custom("x".into())).unresolved_count(),
            0
        );
    }

    #[test]
    fn merge_segments_interleaves_shared_and_unique_lines() {
        let segments = super::merge_segments("keep\nours", "keep\ntheirs");
        let tags: Vec<_> = segments
            .iter()
            .map(|segment| (segment.origin, segment.text.as_str()))
            .collect();
        assert_eq!(
            tags,
            vec![
                (super::SegmentOrigin::Common, "keep"),
                (super::SegmentOrigin::Ours, "ours"),
                (super::SegmentOrigin::Theirs, "theirs"),
            ]
        );
    }

    #[test]
    fn toggle_segment_picks_individual_lines_from_each_side() {
        // ours = [a1, a2], theirs = [b1] with no shared lines.
        let mut data = ConflictData {
            path: "f".into(),
            sections: vec![ConflictPart::Conflict {
                ours: "a1\na2".into(),
                theirs: "b1".into(),
                resolution: ConflictChoice::Unresolved,
            }],
        };
        // Segments order: Ours(a1), Ours(a2), Theirs(b1).
        data.toggle_segment(0, 0); // keep a1
        data.toggle_segment(0, 2); // keep b1
        assert_eq!(data.compose(), "a1\nb1");

        data.toggle_segment(0, 0); // drop a1 again
        assert_eq!(data.compose(), "b1");
    }

    fn conflict_parts(sections: &[ConflictPart]) -> Vec<(String, String)> {
        sections
            .iter()
            .filter_map(|section| match section {
                ConflictPart::Conflict { ours, theirs, .. } => {
                    Some((ours.clone(), theirs.clone()))
                }
                ConflictPart::Common(_) => None,
            })
            .collect()
    }

    #[test]
    fn diff3_auto_resolves_one_sided_edits() {
        // Ours changed nothing; theirs edited the middle line → take theirs, no
        // conflict at all.
        let sections = super::diff3("a\nb\nc", "a\nb\nc", "a\nB\nc");
        assert!(conflict_parts(&sections).is_empty());
        assert_eq!(
            ConflictData {
                path: "f".into(),
                sections
            }
            .compose(),
            "a\nB\nc"
        );

        // Symmetric: only ours edits → take ours.
        let sections = super::diff3("a\nb\nc", "a\nB\nc", "a\nb\nc");
        assert!(conflict_parts(&sections).is_empty());
    }

    #[test]
    fn diff3_auto_keeps_one_sided_insertion() {
        // Theirs appended a line the base and ours never had → kept without a
        // conflict.
        let sections = super::diff3("a\nb", "a\nb", "a\nb\nc");
        assert!(conflict_parts(&sections).is_empty());
        assert_eq!(
            ConflictData {
                path: "f".into(),
                sections
            }
            .compose(),
            "a\nb\nc"
        );
    }

    #[test]
    fn diff3_flags_two_sided_edits() {
        // Both sides changed the same line differently → a real conflict.
        let sections = super::diff3("a\nb\nc", "a\nOURS\nc", "a\nTHEIRS\nc");
        let conflicts = conflict_parts(&sections);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0], ("OURS".into(), "THEIRS".into()));
    }

    #[test]
    fn diff3_treats_modify_versus_delete_as_a_conflict() {
        // Ours deletes the line, theirs rewrites it — never silently drop one.
        let sections = super::diff3("keep\nx\ntail", "keep\ntail", "keep\nX!\ntail");
        let conflicts = conflict_parts(&sections);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0], ("".into(), "X!".into()));
    }

    #[test]
    fn diff3_identical_edits_do_not_conflict() {
        let sections = super::diff3("a\nb", "a\nZ", "a\nZ");
        assert!(conflict_parts(&sections).is_empty());
        assert_eq!(
            ConflictData {
                path: "f".into(),
                sections
            }
            .compose(),
            "a\nZ"
        );
    }

    #[test]
    fn conflict_segments_reports_mask_for_quick_choice() {
        let data = conflict(ConflictChoice::Theirs);
        let (segments, mask) = data.conflict_segments(1).expect("conflict at index 1");
        // ours "mine" and theirs "yours" share nothing → two segments.
        assert_eq!(segments.len(), 2);
        // Theirs choice keeps only the theirs line.
        let kept: Vec<_> = segments
            .iter()
            .zip(mask.iter())
            .filter(|(_, keep)| **keep)
            .map(|(segment, _)| segment.text.as_str())
            .collect();
        assert_eq!(kept, vec!["yours"]);
    }
}
