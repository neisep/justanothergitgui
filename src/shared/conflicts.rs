/// The line terminator a conflicted file uses.
///
/// Every section text in this module is stored terminator-free (split with
/// [`str::lines`], which also strips a trailing `\r`), so the original style has
/// to be carried alongside and put back by [`ConflictData::compose`]. Without
/// it, resolving a single conflict in a CRLF file would rewrite every line in
/// that file to LF.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Eol {
    #[default]
    Lf,
    Crlf,
}

impl Eol {
    /// The terminator a text predominantly uses. One CRLF is taken as proof:
    /// mixed-ending files are rare, and matching the majority style beats
    /// silently normalising the whole file.
    pub fn detect(text: &str) -> Self {
        if text.contains("\r\n") {
            Self::Crlf
        } else {
            Self::Lf
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
        }
    }

    /// Rewrite every terminator in `text` to this one, whatever it used before.
    ///
    /// Text that reaches the model from outside — most of all a hand-typed
    /// resolution, since a multiline text box only ever inserts `\n` — has to be
    /// brought onto the file's own terminator before it is stored, or composing
    /// would splice bare LF lines into a CRLF file.
    pub fn normalize(self, text: &str) -> String {
        let lf = text.replace("\r\n", "\n");
        match self {
            Self::Lf => lf,
            Self::Crlf => lf.replace('\n', "\r\n"),
        }
    }
}

/// How the source file was formatted, so a resolved write matches it rather
/// than imposing a house style on the user's repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStyle {
    pub eol: Eol,
    /// Whether the file ended with a line terminator. Git tracks its absence
    /// ("\ No newline at end of file"), so adding one shows up as a change.
    pub trailing_newline: bool,
}

impl FileStyle {
    pub fn detect(text: &str) -> Self {
        Self {
            eol: Eol::detect(text),
            trailing_newline: text.ends_with('\n'),
        }
    }
}

impl Default for FileStyle {
    /// Matches how text files are normally written, for the rare caller with no
    /// source text to inspect.
    fn default() -> Self {
        Self {
            eol: Eol::default(),
            trailing_newline: true,
        }
    }
}

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

/// Ceiling on the longest-common-subsequence table, in cells.
///
/// The table is inherently `O(lines_a * lines_b)`, so a large conflicted file
/// would otherwise ask for gigabytes: 20k lines a side is 3.2 GB as `usize`,
/// and with `panic = "abort"` a failed allocation kills the app instead of
/// surfacing an error. 16Mi cells is 64 MiB as `u32` and covers any conflict a
/// person would work through by hand; past that, callers degrade to a coarser
/// answer rather than trying.
const MAX_LCS_CELLS: usize = 16 << 20;

/// Row-major suffix-length table for the longest common subsequence of two line
/// slices: `at(i, j)` is the LCS length of `a[i..]` and `b[j..]`.
///
/// One flat allocation of `u32` rather than a `Vec<Vec<usize>>` — a quarter the
/// bytes, one allocation instead of `n + 1`, and contiguous for the scan.
struct LcsTable {
    dp: Vec<u32>,
    stride: usize,
}

impl LcsTable {
    /// `None` when the table would exceed [`MAX_LCS_CELLS`].
    fn build(a: &[&str], b: &[&str]) -> Option<Self> {
        let (n, m) = (a.len(), b.len());
        let stride = m + 1;
        let cells = stride.checked_mul(n + 1)?;
        if cells > MAX_LCS_CELLS {
            return None;
        }

        let mut dp = vec![0u32; cells];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                dp[i * stride + j] = if a[i] == b[j] {
                    dp[(i + 1) * stride + j + 1] + 1
                } else {
                    dp[(i + 1) * stride + j].max(dp[i * stride + j + 1])
                };
            }
        }

        Some(Self { dp, stride })
    }

    /// Whether advancing on `a` keeps a subsequence at least as long as
    /// advancing on `b` would — the backtracking tie-break.
    fn prefer_a(&self, i: usize, j: usize) -> bool {
        self.dp[(i + 1) * self.stride + j] >= self.dp[i * self.stride + j + 1]
    }
}

/// Interleave two sides into a single ordered line sequence via a longest
/// common subsequence, tagging each line as `Common`, `Ours`, or `Theirs`.
/// This is the backbone of per-line conflict picking.
pub fn merge_segments(ours: &str, theirs: &str) -> Vec<MergeSegment> {
    let a: Vec<&str> = ours.lines().collect();
    let b: Vec<&str> = theirs.lines().collect();
    let (n, m) = (a.len(), b.len());

    let Some(table) = LcsTable::build(&a, &b) else {
        // Too large to align line by line. Present the sides one after the
        // other instead of allocating a multi-gigabyte table: picking still
        // works, only the interleaving is coarser.
        return a
            .iter()
            .map(|line| segment(SegmentOrigin::Ours, line))
            .chain(b.iter().map(|line| segment(SegmentOrigin::Theirs, line)))
            .collect();
    };

    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            out.push(segment(SegmentOrigin::Common, a[i]));
            i += 1;
            j += 1;
        } else if j >= m || (i < n && table.prefer_a(i, j)) {
            out.push(segment(SegmentOrigin::Ours, a[i]));
            i += 1;
        } else {
            out.push(segment(SegmentOrigin::Theirs, b[j]));
            j += 1;
        }
    }

    out
}

fn segment(origin: SegmentOrigin, text: &str) -> MergeSegment {
    MergeSegment {
        origin,
        text: text.to_string(),
    }
}

/// The matched index pairs of a longest common subsequence between two line
/// slices, in increasing order on both sides. Shared backbone of the 2-way
/// [`merge_segments`] and the 3-way [`diff3`].
///
/// Empty when the table would be oversized: with no anchors, [`diff3`] falls
/// back to reporting the whole file as one conflict.
fn lcs_pairs(a: &[&str], b: &[&str]) -> Vec<(usize, usize)> {
    let Some(table) = LcsTable::build(a, b) else {
        return Vec::new();
    };

    let (n, m) = (a.len(), b.len());
    let mut pairs = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if table.prefer_a(i, j) {
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
pub fn diff3(base: &str, ours: &str, theirs: &str, eol: Eol) -> Vec<ConflictPart> {
    let sep = eol.as_str();
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

    // The base lines both sides still carry, in increasing order. These are the
    // only candidates for the end of a change region, and because `lcs_pairs`
    // yields its matches in increasing order on both sides, `ak` and `bk` both
    // rise with the base index — so a cursor that only moves forward can never
    // skip an anchor a later region still wanted.
    let anchors: Vec<(usize, usize, usize)> = (0..o.len())
        .filter_map(|k| match (in_a[k], in_b[k]) {
            (Some(ak), Some(bk)) => Some((k, ak, bk)),
            _ => None,
        })
        .collect();

    let mut parts: Vec<ConflictPart> = Vec::new();
    let mut common: Vec<&str> = Vec::new();
    let (mut oi, mut ai, mut bi) = (0usize, 0usize, 0usize);
    let mut next_anchor = 0usize;

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
        while anchors
            .get(next_anchor)
            .is_some_and(|&(k, ak, bk)| k < oi || ak < ai || bk < bi)
        {
            next_anchor += 1;
        }
        let (o2, a2, b2) = anchors
            .get(next_anchor)
            .copied()
            .unwrap_or((o.len(), a.len(), b.len()));

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
                parts.push(ConflictPart::Common(common.join(sep)));
                common.clear();
            }
            parts.push(ConflictPart::Conflict {
                ours: a_slice.join(sep),
                theirs: b_slice.join(sep),
                resolution: ConflictChoice::default(),
            });
        }

        (oi, ai, bi) = (o2, a2, b2);
    }

    if !common.is_empty() {
        parts.push(ConflictPart::Common(common.join(sep)));
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
fn compose_picked(segments: &[MergeSegment], mask: &[bool], sep: &str) -> String {
    segments
        .iter()
        .zip(mask.iter())
        .filter(|(_, keep)| **keep)
        .map(|(segment, _)| segment.text.as_str())
        .collect::<Vec<_>>()
        .join(sep)
}

/// Join only the parts that carry text, so an empty side never contributes a
/// stray blank line to the result.
fn join_non_empty(parts: [&str; 2], sep: &str) -> String {
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(sep)
}

#[derive(Clone, Debug)]
pub struct ConflictData {
    pub path: String,
    /// Formatting of the source file, restored by [`Self::compose`].
    pub style: FileStyle,
    sections: Vec<ConflictPart>,
    /// Segments per section, keyed by the same index as `sections` and empty
    /// for `Common` ones.
    ///
    /// Built once in [`Self::new`]. They depend only on a conflict's `ours` and
    /// `theirs`, which never change once parsed — only the resolution does — so
    /// the cache cannot go stale, and keeping `sections` private is what
    /// guarantees that. The merge editor re-renders every frame and asks for
    /// these three times per conflict; recomputing the LCS there dominated
    /// frame time outright.
    segments: Vec<Vec<MergeSegment>>,
    /// Keep/drop mask per section, keyed the same way and empty for `Common`
    /// ones.
    ///
    /// Unlike the segments this *does* move with the resolution, so it is
    /// rebuilt by the two methods that can change one — which is cheap, because
    /// that happens on a click rather than on a frame. Deriving it on demand
    /// instead meant a `Vec<bool>` allocated three times per conflict per frame,
    /// for the result document and both input panes.
    masks: Vec<Vec<bool>>,
}

impl ConflictData {
    pub fn new(path: String, sections: Vec<ConflictPart>, style: FileStyle) -> Self {
        let segments: Vec<Vec<MergeSegment>> = sections
            .iter()
            .map(|section| match section {
                ConflictPart::Conflict { ours, theirs, .. } => merge_segments(ours, theirs),
                ConflictPart::Common(_) => Vec::new(),
            })
            .collect();

        let masks = sections
            .iter()
            .zip(segments.iter())
            .map(|(section, segments)| match section {
                ConflictPart::Conflict { resolution, .. } => mask_for(segments, resolution),
                ConflictPart::Common(_) => Vec::new(),
            })
            .collect();

        Self {
            path,
            style,
            sections,
            segments,
            masks,
        }
    }

    /// Read-only view of the ordered sections.
    pub fn sections(&self) -> &[ConflictPart] {
        &self.sections
    }

    /// Record the user's choice for one conflict. The only sanctioned mutation:
    /// it leaves `ours`/`theirs` — and therefore the cached segments — intact.
    ///
    /// A `Custom` resolution is brought onto the file's own terminator on the way
    /// in. It is the one choice whose text comes from outside the parsed file —
    /// an inline text box, which only ever inserts `\n` — so this is where that
    /// text stops being the editor's and starts being the file's.
    pub fn set_resolution(&mut self, section_index: usize, choice: ConflictChoice) {
        let choice = match choice {
            ConflictChoice::Custom(text) => ConflictChoice::Custom(self.style.eol.normalize(&text)),
            other => other,
        };

        if let Some(ConflictPart::Conflict { resolution, .. }) =
            self.sections.get_mut(section_index)
        {
            *resolution = choice;
        }

        self.refresh_mask(section_index);
    }

    /// Bring one section's cached mask back in step with its resolution.
    fn refresh_mask(&mut self, section_index: usize) {
        let Some(ConflictPart::Conflict { resolution, .. }) = self.sections.get(section_index)
        else {
            return;
        };

        let mask = mask_for(self.segments_at(section_index), resolution);
        if let Some(slot) = self.masks.get_mut(section_index) {
            *slot = mask;
        }
    }
    /// Render the sections back into the file's exact text, in the source
    /// file's own line terminator and with its final-newline state restored.
    /// The result is what gets written to disk verbatim.
    ///
    /// Resolved conflicts emit the chosen side(s); an `Unresolved` conflict
    /// emits the raw `<<<<<<<` / `=======` / `>>>>>>>` markers so it stays
    /// visible and editable in the result buffer. Unlike a writer, this never
    /// fails — it is used to seed the editable merge result every frame.
    pub fn compose(&self) -> String {
        let sep = self.style.eol.as_str();
        let mut pieces: Vec<String> = Vec::with_capacity(self.sections.len());

        for (index, section) in self.sections.iter().enumerate() {
            match section {
                // A `Common` run is emitted verbatim even when it is empty: an
                // empty one is a single blank line between two conflicts, and
                // dropping it would delete a real line from the file.
                ConflictPart::Common(text) => pieces.push(text.clone()),
                ConflictPart::Conflict {
                    ours,
                    theirs,
                    resolution,
                    ..
                } => {
                    let resolved = match resolution {
                        ConflictChoice::Ours => ours.clone(),
                        ConflictChoice::Theirs => theirs.clone(),
                        ConflictChoice::Both => join_non_empty([ours, theirs], sep),
                        ConflictChoice::Custom(text) => text.clone(),
                        ConflictChoice::Picked(mask) => {
                            compose_picked(self.segments_at(index), mask, sep)
                        }
                        ConflictChoice::Unresolved => format!(
                            "<<<<<<< Current (ours){sep}{ours}{sep}======={sep}{theirs}{sep}>>>>>>> Incoming (theirs)"
                        ),
                    };

                    // An empty resolution means the region was deleted — a
                    // whole-side deletion, or every line unticked. Emitting it
                    // would leave a blank line where the conflict used to be.
                    if !resolved.is_empty() {
                        pieces.push(resolved);
                    }
                }
            }
        }

        let mut content = pieces.join(sep);
        // An empty result stays a genuinely empty file: terminating nothing
        // would turn a deleted-everything merge into a one-blank-line file.
        if self.style.trailing_newline && !content.is_empty() {
            content.push_str(sep);
        }

        content
    }

    /// Cached segments for a section; empty for `Common` ones and out-of-range
    /// indices, both of which have no lines to pick.
    fn segments_at(&self, section_index: usize) -> &[MergeSegment] {
        self.segments
            .get(section_index)
            .map_or(&[], |segments| segments.as_slice())
    }

    /// Cached mask for a section; empty for `Common` ones and out-of-range
    /// indices, matching [`Self::segments_at`].
    fn mask_at(&self, section_index: usize) -> &[bool] {
        self.masks
            .get(section_index)
            .map_or(&[], |mask| mask.as_slice())
    }

    /// The line segments of a conflict section plus its current keep/drop mask,
    /// for rendering the per-line picker. `None` if the index is not a conflict.
    ///
    /// Both halves are borrowed from the cache, so calling this on every frame —
    /// which the merge editor does, three times per conflict — allocates nothing.
    pub fn conflict_segments(&self, section_index: usize) -> Option<(&[MergeSegment], &[bool])> {
        let Some(ConflictPart::Conflict { .. }) = self.sections.get(section_index) else {
            return None;
        };

        Some((self.segments_at(section_index), self.mask_at(section_index)))
    }

    /// Toggle whether one line of a conflict is kept, switching that conflict
    /// into `Picked` mode. `Common` lines are always kept and ignore toggles.
    pub fn toggle_segment(&mut self, section_index: usize, segment_index: usize) {
        let Some(ConflictPart::Conflict { .. }) = self.sections.get(section_index) else {
            return;
        };

        let mut mask = self.mask_at(section_index).to_vec();
        let segment = self
            .segments
            .get(section_index)
            .and_then(|segments| segments.get(segment_index));
        if let (Some(segment), Some(flag)) = (segment, mask.get_mut(segment_index))
            && segment.origin != SegmentOrigin::Common
        {
            *flag = !*flag;
        }

        self.set_resolution(section_index, ConflictChoice::Picked(mask));
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
    use super::{ConflictChoice, ConflictData, ConflictPart, Eol, FileStyle};

    /// These fixtures assert on how sections are joined, so they use a
    /// terminator-free style; the final-newline behaviour is covered on its own
    /// in `compose_restores_the_source_final_newline_state`.
    const RAW_LF: FileStyle = FileStyle {
        eol: Eol::Lf,
        trailing_newline: false,
    };
    const RAW_CRLF: FileStyle = FileStyle {
        eol: Eol::Crlf,
        trailing_newline: false,
    };

    /// The existing cases all predate CRLF support; keep them reading as before.
    fn diff3_lf(base: &str, ours: &str, theirs: &str) -> Vec<ConflictPart> {
        super::diff3(base, ours, theirs, Eol::Lf)
    }

    fn conflict(resolution: ConflictChoice) -> ConflictData {
        ConflictData::new(
            "file.txt".into(),
            vec![
                ConflictPart::Common("top".into()),
                ConflictPart::Conflict {
                    ours: "mine".into(),
                    theirs: "yours".into(),
                    resolution,
                },
                ConflictPart::Common("bottom".into()),
            ],
            RAW_LF,
        )
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
        let mut data = ConflictData::new(
            "f".into(),
            vec![ConflictPart::Conflict {
                ours: "a1\na2".into(),
                theirs: "b1".into(),
                resolution: ConflictChoice::Unresolved,
            }],
            RAW_LF,
        );
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
                ConflictPart::Conflict { ours, theirs, .. } => Some((ours.clone(), theirs.clone())),
                ConflictPart::Common(_) => None,
            })
            .collect()
    }

    #[test]
    fn diff3_auto_resolves_one_sided_edits() {
        // Ours changed nothing; theirs edited the middle line → take theirs, no
        // conflict at all.
        let sections = diff3_lf("a\nb\nc", "a\nb\nc", "a\nB\nc");
        assert!(conflict_parts(&sections).is_empty());
        assert_eq!(
            ConflictData::new("f".into(), sections, RAW_LF).compose(),
            "a\nB\nc"
        );

        // Symmetric: only ours edits → take ours.
        let sections = diff3_lf("a\nb\nc", "a\nB\nc", "a\nb\nc");
        assert!(conflict_parts(&sections).is_empty());
    }

    #[test]
    fn diff3_auto_keeps_one_sided_insertion() {
        // Theirs appended a line the base and ours never had → kept without a
        // conflict.
        let sections = diff3_lf("a\nb", "a\nb", "a\nb\nc");
        assert!(conflict_parts(&sections).is_empty());
        assert_eq!(
            ConflictData::new("f".into(), sections, RAW_LF).compose(),
            "a\nb\nc"
        );
    }

    #[test]
    fn diff3_flags_two_sided_edits() {
        // Both sides changed the same line differently → a real conflict.
        let sections = diff3_lf("a\nb\nc", "a\nOURS\nc", "a\nTHEIRS\nc");
        let conflicts = conflict_parts(&sections);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0], ("OURS".into(), "THEIRS".into()));
    }

    #[test]
    fn diff3_treats_modify_versus_delete_as_a_conflict() {
        // Ours deletes the line, theirs rewrites it — never silently drop one.
        let sections = diff3_lf("keep\nx\ntail", "keep\ntail", "keep\nX!\ntail");
        let conflicts = conflict_parts(&sections);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0], ("".into(), "X!".into()));
    }

    #[test]
    fn diff3_identical_edits_do_not_conflict() {
        let sections = diff3_lf("a\nb", "a\nZ", "a\nZ");
        assert!(conflict_parts(&sections).is_empty());
        assert_eq!(
            ConflictData::new("f".into(), sections, RAW_LF).compose(),
            "a\nZ"
        );
    }

    #[test]
    fn oversized_sides_degrade_instead_of_allocating() {
        // Past MAX_LCS_CELLS the quadratic table is refused. The sides must still
        // come back in full — coarsely separated rather than interleaved — so no
        // line is ever lost to the size guard.
        let side = (0..5000).fold(String::new(), |mut text, index| {
            text.push_str(&format!("line {index}\n"));
            text
        });
        let other = (0..5000).fold(String::new(), |mut text, index| {
            text.push_str(&format!("other {index}\n"));
            text
        });

        let segments = super::merge_segments(&side, &other);

        assert_eq!(segments.len(), 10_000);
        let ours = segments
            .iter()
            .filter(|segment| segment.origin == super::SegmentOrigin::Ours)
            .count();
        let theirs = segments
            .iter()
            .filter(|segment| segment.origin == super::SegmentOrigin::Theirs)
            .count();
        assert_eq!((ours, theirs), (5000, 5000));
    }

    #[test]
    fn segments_are_cached_not_recomputed() {
        // conflict_segments borrows from the cache: the returned slice must be
        // the same memory across calls, since the merge editor asks three times
        // per conflict per frame.
        let data = conflict(ConflictChoice::Ours);
        let (first, _) = data.conflict_segments(1).expect("conflict at index 1");
        let (second, _) = data.conflict_segments(1).expect("conflict at index 1");

        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn masks_are_cached_and_track_the_resolution() {
        // Same guarantee as the segments: the merge editor asks three times per
        // conflict per frame, so the mask must be borrowed, not rebuilt — and it
        // must still follow every resolution change.
        let mut data = conflict(ConflictChoice::Ours);
        let (_, first) = data.conflict_segments(1).expect("conflict at index 1");
        let (_, second) = data.conflict_segments(1).expect("conflict at index 1");
        assert!(std::ptr::eq(first, second));
        // ours "mine" / theirs "yours" share nothing: [Ours, Theirs].
        assert_eq!(first, [true, false]);

        data.set_resolution(1, ConflictChoice::Theirs);
        let (_, after) = data.conflict_segments(1).expect("conflict at index 1");
        assert_eq!(after, [false, true]);

        data.toggle_segment(1, 0);
        let (_, toggled) = data.conflict_segments(1).expect("conflict at index 1");
        assert_eq!(toggled, [true, true]);
        assert_eq!(data.compose(), "top\nmine\nyours\nbottom");
    }

    #[test]
    fn compose_restores_crlf_line_endings() {
        // A CRLF file must come back out as CRLF: resolving one conflict may not
        // silently rewrite every other line in the file.
        let sections = super::diff3(
            "a\r\nb\r\nc\r\n",
            "a\r\nB\r\nc\r\n",
            "a\r\nb\r\nc\r\n",
            Eol::Crlf,
        );
        let composed = ConflictData::new("f".into(), sections, RAW_CRLF).compose();

        assert_eq!(composed, "a\r\nB\r\nc");
        assert!(!composed.contains("\n\r"), "no stray bare LF: {composed:?}");
    }

    #[test]
    fn compose_restores_the_source_final_newline_state() {
        let sections = || vec![ConflictPart::Common("a\nb".into())];

        // Git tracks "no newline at end of file", so a file that had none must
        // not silently gain one.
        let bare = ConflictData::new("f".into(), sections(), RAW_LF);
        assert_eq!(bare.compose(), "a\nb");

        let terminated = ConflictData::new(
            "f".into(),
            sections(),
            FileStyle {
                eol: Eol::Lf,
                trailing_newline: true,
            },
        );
        assert_eq!(terminated.compose(), "a\nb\n");

        // CRLF files get their own terminator back, not a bare LF. Section text
        // already carries the file's separator internally (diff3 builds it with
        // the same Eol); compose only controls the joins between sections and
        // the final one.
        let crlf = ConflictData::new(
            "f".into(),
            vec![ConflictPart::Common("a\r\nb".into())],
            FileStyle {
                eol: Eol::Crlf,
                trailing_newline: true,
            },
        );
        assert_eq!(crlf.compose(), "a\r\nb\r\n");
    }

    #[test]
    fn compose_leaves_a_fully_emptied_file_empty() {
        // Everything deleted means an empty file, not a file holding one blank
        // line — even when the source was newline-terminated.
        let data = ConflictData::new(
            "f".into(),
            vec![ConflictPart::Conflict {
                ours: String::new(),
                theirs: "gone".into(),
                resolution: ConflictChoice::Ours,
            }],
            FileStyle {
                eol: Eol::Lf,
                trailing_newline: true,
            },
        );

        assert_eq!(data.compose(), "");
    }

    #[test]
    fn file_style_detect_reads_both_traits() {
        assert_eq!(
            FileStyle::detect("a\r\nb\r\n"),
            FileStyle {
                eol: Eol::Crlf,
                trailing_newline: true
            }
        );
        assert_eq!(
            FileStyle::detect("a\nb"),
            FileStyle {
                eol: Eol::Lf,
                trailing_newline: false
            }
        );
    }

    #[test]
    fn eol_normalize_rewrites_every_terminator() {
        assert_eq!(Eol::Crlf.normalize("a\nb\r\nc"), "a\r\nb\r\nc");
        assert_eq!(Eol::Lf.normalize("a\r\nb\nc"), "a\nb\nc");
        // Already in the target style: unchanged, and no doubled `\r`.
        assert_eq!(Eol::Crlf.normalize("a\r\nb"), "a\r\nb");
    }

    #[test]
    fn set_resolution_puts_a_custom_edit_on_the_files_terminator() {
        // The inline editor is a text box: it only ever produces `\n`. Storing
        // that verbatim would splice bare LF lines into a CRLF file, because
        // `compose` emits a custom resolution exactly as given.
        let mut data = ConflictData::new(
            "f".into(),
            vec![ConflictPart::Conflict {
                ours: "mine".into(),
                theirs: "yours".into(),
                resolution: ConflictChoice::Unresolved,
            }],
            RAW_CRLF,
        );
        data.set_resolution(0, ConflictChoice::Custom("one\ntwo".into()));

        let composed = data.compose();
        assert_eq!(composed, "one\r\ntwo");
        assert!(!composed.contains("\n\r"), "no stray bare LF: {composed:?}");
    }

    #[test]
    fn eol_detect_reads_the_dominant_terminator() {
        assert_eq!(Eol::detect("a\r\nb\r\n"), Eol::Crlf);
        assert_eq!(Eol::detect("a\nb\n"), Eol::Lf);
        assert_eq!(Eol::detect(""), Eol::Lf);
    }

    #[test]
    fn compose_drops_a_conflict_resolved_to_nothing() {
        // Ours deleted the line, theirs rewrote it. Accepting ours means the line
        // is gone — not replaced by a blank one.
        let sections = diff3_lf("keep\nx\ntail", "keep\ntail", "keep\nX!\ntail");
        let mut data = ConflictData::new("f".into(), sections, RAW_LF);
        set_only_conflict(&mut data, ConflictChoice::Ours);

        assert_eq!(data.compose(), "keep\ntail");
    }

    #[test]
    fn compose_drops_a_conflict_with_every_line_unticked() {
        // What the "Clear" button produces: keep only the (absent) common lines.
        let sections = diff3_lf("keep\nx\ntail", "keep\nOURS\ntail", "keep\nTHEIRS\ntail");
        let mut data = ConflictData::new("f".into(), sections, RAW_LF);
        set_only_conflict(&mut data, ConflictChoice::Picked(vec![false, false]));

        assert_eq!(data.compose(), "keep\ntail");
    }

    #[test]
    fn compose_keeps_a_blank_line_that_is_genuinely_common() {
        // An empty `Common` section is one real blank line between two conflicts.
        // It must survive the empty-piece filtering that the cases above rely on.
        let data = ConflictData::new(
            "f".into(),
            vec![
                ConflictPart::Conflict {
                    ours: "one".into(),
                    theirs: "1".into(),
                    resolution: ConflictChoice::Ours,
                },
                ConflictPart::Common(String::new()),
                ConflictPart::Conflict {
                    ours: "two".into(),
                    theirs: "2".into(),
                    resolution: ConflictChoice::Ours,
                },
            ],
            RAW_LF,
        );

        assert_eq!(data.compose(), "one\n\ntwo");
    }

    #[test]
    fn compose_both_skips_an_empty_side() {
        // "Both" on a delete-vs-modify conflict is just the surviving side.
        let sections = diff3_lf("keep\nx\ntail", "keep\ntail", "keep\nX!\ntail");
        let mut data = ConflictData::new("f".into(), sections, RAW_LF);
        set_only_conflict(&mut data, ConflictChoice::Both);

        assert_eq!(data.compose(), "keep\nX!\ntail");
    }

    /// Goes through `set_resolution` rather than reaching into `sections`, so the
    /// cached mask stays in step — the same guarantee production code relies on.
    fn set_only_conflict(data: &mut ConflictData, choice: ConflictChoice) {
        let index = data
            .sections()
            .iter()
            .position(|section| matches!(section, ConflictPart::Conflict { .. }))
            .expect("exactly one conflict in the fixture");
        data.set_resolution(index, choice);
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
