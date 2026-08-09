//! Unified-patch parsing and side-by-side pairing.
//!
//! Pure text handling: no egui, no git2. Consumers are the diff renderers in
//! `ui/`, which turn these rows into widgets.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
    HunkHeader,
    FileHeader,
    Note,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedDiffLine {
    pub old_line_number: Option<usize>,
    pub new_line_number: Option<usize>,
    pub kind: DiffLineKind,
    pub content: String,
}

/// One side of a side-by-side row: either a real line or padding opposite an
/// unpaired insertion/deletion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SideCell {
    Empty,
    Line {
        number: Option<usize>,
        kind: DiffLineKind,
        content: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideBySideRow {
    pub old: SideCell,
    pub new: SideCell,
}

/// A side-by-side document: full-width headers interleaved with paired rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SideBySideEntry {
    Header(String),
    Row(SideBySideRow),
}

pub fn parse_diff_rows(diff_content: &str) -> Vec<ParsedDiffLine> {
    let mut rows = Vec::new();
    let mut old_line_number = None;
    let mut new_line_number = None;

    for line in diff_content.lines() {
        let kind = classify_diff_line(line);

        if kind == DiffLineKind::HunkHeader
            && let Some((old_start, new_start)) = parse_hunk_header(line)
        {
            old_line_number = Some(old_start);
            new_line_number = Some(new_start);
        }

        let row = match kind {
            DiffLineKind::Context => {
                let old = old_line_number;
                let new = new_line_number;
                old_line_number = old_line_number.map(|line| line + 1);
                new_line_number = new_line_number.map(|line| line + 1);
                ParsedDiffLine {
                    old_line_number: old,
                    new_line_number: new,
                    kind,
                    content: line[1..].to_string(),
                }
            }
            DiffLineKind::Added => {
                let new = new_line_number;
                new_line_number = new_line_number.map(|line| line + 1);
                ParsedDiffLine {
                    old_line_number: None,
                    new_line_number: new,
                    kind,
                    content: line[1..].to_string(),
                }
            }
            DiffLineKind::Removed => {
                let old = old_line_number;
                old_line_number = old_line_number.map(|line| line + 1);
                ParsedDiffLine {
                    old_line_number: old,
                    new_line_number: None,
                    kind,
                    content: line[1..].to_string(),
                }
            }
            _ => ParsedDiffLine {
                old_line_number: None,
                new_line_number: None,
                kind,
                content: line.to_string(),
            },
        };

        rows.push(row);
    }

    rows
}

pub fn classify_diff_line(line: &str) -> DiffLineKind {
    if line.starts_with("@@") {
        DiffLineKind::HunkHeader
    } else if line.starts_with("diff --git")
        || line.starts_with("index ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
        || line.starts_with("rename from ")
        || line.starts_with("rename to ")
        || line.starts_with("new file mode ")
        || line.starts_with("deleted file mode ")
        || line.starts_with("similarity index ")
    {
        DiffLineKind::FileHeader
    } else if line.starts_with("\\ ") {
        DiffLineKind::Note
    } else if line.starts_with('+') {
        DiffLineKind::Added
    } else if line.starts_with('-') {
        DiffLineKind::Removed
    } else if line.starts_with(' ') {
        DiffLineKind::Context
    } else {
        DiffLineKind::Other
    }
}

pub fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    let mut parts = line.split_whitespace();
    if parts.next()? != "@@" {
        return None;
    }

    let old_range = parts.next()?;
    let new_range = parts.next()?;
    if parts.next()? != "@@" {
        return None;
    }

    Some((
        parse_hunk_range(old_range, '-')?,
        parse_hunk_range(new_range, '+')?,
    ))
}

fn parse_hunk_range(range: &str, expected_prefix: char) -> Option<usize> {
    let trimmed = range.strip_prefix(expected_prefix)?;
    trimmed.split(',').next()?.parse().ok()
}

/// Pair a unified patch into aligned old/new rows.
///
/// A run of removals directly followed by a run of additions is read as a
/// replacement and zipped line by line; whichever run is shorter is padded with
/// [`SideCell::Empty`] so both sides keep the same row count. Everything that is
/// not part of the text (hunk and file headers, `\ No newline` notes) becomes a
/// full-width [`SideBySideEntry::Header`].
pub fn to_side_by_side(rows: &[ParsedDiffLine]) -> Vec<SideBySideEntry> {
    let mut entries = Vec::new();
    let mut removed: Vec<&ParsedDiffLine> = Vec::new();
    let mut added: Vec<&ParsedDiffLine> = Vec::new();

    for row in rows {
        match row.kind {
            DiffLineKind::Removed => {
                // An addition run already started, so this removal belongs to a
                // new pairing group.
                if !added.is_empty() {
                    flush_pairs(&mut entries, &mut removed, &mut added);
                }
                removed.push(row);
            }
            DiffLineKind::Added => added.push(row),
            DiffLineKind::Context => {
                flush_pairs(&mut entries, &mut removed, &mut added);
                entries.push(SideBySideEntry::Row(SideBySideRow {
                    old: cell(row, row.old_line_number),
                    new: cell(row, row.new_line_number),
                }));
            }
            _ => {
                flush_pairs(&mut entries, &mut removed, &mut added);
                entries.push(SideBySideEntry::Header(row.content.clone()));
            }
        }
    }

    flush_pairs(&mut entries, &mut removed, &mut added);
    entries
}

fn flush_pairs<'a>(
    entries: &mut Vec<SideBySideEntry>,
    removed: &mut Vec<&'a ParsedDiffLine>,
    added: &mut Vec<&'a ParsedDiffLine>,
) {
    let pairs = removed.len().max(added.len());
    for index in 0..pairs {
        entries.push(SideBySideEntry::Row(SideBySideRow {
            old: removed
                .get(index)
                .map_or(SideCell::Empty, |row| cell(row, row.old_line_number)),
            new: added
                .get(index)
                .map_or(SideCell::Empty, |row| cell(row, row.new_line_number)),
        }));
    }

    removed.clear();
    added.clear();
}

fn cell(row: &ParsedDiffLine, number: Option<usize>) -> SideCell {
    SideCell::Line {
        number,
        kind: row.kind,
        content: row.content.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DiffLineKind, ParsedDiffLine, SideBySideEntry, SideCell, parse_diff_rows,
        parse_hunk_header, to_side_by_side,
    };

    #[test]
    fn parses_hunk_start_line_numbers() {
        assert_eq!(
            parse_hunk_header("@@ -14,3 +20,7 @@ fn render()"),
            Some((14, 20))
        );
    }

    #[test]
    fn assigns_old_and_new_line_numbers_to_diff_rows() {
        let rows = parse_diff_rows(concat!(
            "diff --git a/src/app.rs b/src/app.rs\n",
            "@@ -10,2 +10,3 @@\n",
            " line one\n",
            "-line removed\n",
            "+line added\n",
            "+line added too\n",
        ));

        assert_eq!(rows[0].kind, DiffLineKind::FileHeader);
        assert_eq!(rows[1].kind, DiffLineKind::HunkHeader);
        assert_eq!(rows[2].old_line_number, Some(10));
        assert_eq!(rows[2].new_line_number, Some(10));
        assert_eq!(rows[3].kind, DiffLineKind::Removed);
        assert_eq!(rows[3].old_line_number, Some(11));
        assert_eq!(rows[3].new_line_number, None);
        assert_eq!(rows[4].kind, DiffLineKind::Added);
        assert_eq!(rows[4].old_line_number, None);
        assert_eq!(rows[4].new_line_number, Some(11));
        assert_eq!(rows[5].new_line_number, Some(12));
    }

    fn line(
        kind: DiffLineKind,
        old: Option<usize>,
        new: Option<usize>,
        content: &str,
    ) -> ParsedDiffLine {
        ParsedDiffLine {
            old_line_number: old,
            new_line_number: new,
            kind,
            content: content.to_string(),
        }
    }

    fn old_content(entry: &SideBySideEntry) -> Option<&str> {
        match entry {
            SideBySideEntry::Row(row) => match &row.old {
                SideCell::Line { content, .. } => Some(content),
                SideCell::Empty => None,
            },
            SideBySideEntry::Header(_) => None,
        }
    }

    fn new_content(entry: &SideBySideEntry) -> Option<&str> {
        match entry {
            SideBySideEntry::Row(row) => match &row.new {
                SideCell::Line { content, .. } => Some(content),
                SideCell::Empty => None,
            },
            SideBySideEntry::Header(_) => None,
        }
    }

    #[test]
    fn pairs_a_one_for_one_replacement_on_the_same_row() {
        let rows = vec![
            line(DiffLineKind::Removed, Some(1), None, "before"),
            line(DiffLineKind::Added, None, Some(1), "after"),
        ];

        let entries = to_side_by_side(&rows);

        assert_eq!(entries.len(), 1);
        assert_eq!(old_content(&entries[0]), Some("before"));
        assert_eq!(new_content(&entries[0]), Some("after"));
    }

    #[test]
    fn pads_the_shorter_side_of_an_uneven_replacement() {
        let rows = vec![
            line(DiffLineKind::Removed, Some(1), None, "one"),
            line(DiffLineKind::Removed, Some(2), None, "two"),
            line(DiffLineKind::Removed, Some(3), None, "three"),
            line(DiffLineKind::Added, None, Some(1), "only"),
        ];

        let entries = to_side_by_side(&rows);

        assert_eq!(entries.len(), 3);
        assert_eq!(new_content(&entries[0]), Some("only"));
        assert_eq!(old_content(&entries[1]), Some("two"));
        assert_eq!(new_content(&entries[1]), None);
        assert_eq!(new_content(&entries[2]), None);
    }

    #[test]
    fn leaves_the_new_side_empty_for_pure_deletions() {
        let rows = vec![line(DiffLineKind::Removed, Some(7), None, "gone")];

        let entries = to_side_by_side(&rows);

        assert_eq!(entries.len(), 1);
        assert_eq!(old_content(&entries[0]), Some("gone"));
        assert_eq!(new_content(&entries[0]), None);
    }

    #[test]
    fn leaves_the_old_side_empty_for_pure_insertions() {
        let rows = vec![line(DiffLineKind::Added, None, Some(7), "fresh")];

        let entries = to_side_by_side(&rows);

        assert_eq!(entries.len(), 1);
        assert_eq!(old_content(&entries[0]), None);
        assert_eq!(new_content(&entries[0]), Some("fresh"));
    }

    #[test]
    fn repeats_context_lines_on_both_sides() {
        let rows = vec![line(DiffLineKind::Context, Some(4), Some(9), "shared")];

        let entries = to_side_by_side(&rows);

        assert_eq!(old_content(&entries[0]), Some("shared"));
        assert_eq!(new_content(&entries[0]), Some("shared"));
        match &entries[0] {
            SideBySideEntry::Row(row) => {
                assert!(matches!(
                    row.old,
                    SideCell::Line {
                        number: Some(4),
                        ..
                    }
                ));
                assert!(matches!(
                    row.new,
                    SideCell::Line {
                        number: Some(9),
                        ..
                    }
                ));
            }
            SideBySideEntry::Header(_) => panic!("expected a row"),
        }
    }

    #[test]
    fn keeps_headers_in_order_and_starts_a_new_group_after_them() {
        let rows = parse_diff_rows(concat!(
            "diff --git a/a.rs b/a.rs\n",
            "@@ -1,2 +1,2 @@\n",
            "-old\n",
            "+new\n",
            " tail\n",
        ));

        let entries = to_side_by_side(&rows);

        assert!(
            matches!(&entries[0], SideBySideEntry::Header(text) if text.starts_with("diff --git"))
        );
        assert!(matches!(&entries[1], SideBySideEntry::Header(text) if text.starts_with("@@")));
        assert_eq!(old_content(&entries[2]), Some("old"));
        assert_eq!(new_content(&entries[2]), Some("new"));
        assert_eq!(old_content(&entries[3]), Some("tail"));
    }

    #[test]
    fn separates_back_to_back_replacement_groups() {
        let rows = vec![
            line(DiffLineKind::Removed, Some(1), None, "a-old"),
            line(DiffLineKind::Added, None, Some(1), "a-new"),
            line(DiffLineKind::Removed, Some(2), None, "b-old"),
            line(DiffLineKind::Added, None, Some(2), "b-new"),
        ];

        let entries = to_side_by_side(&rows);

        assert_eq!(entries.len(), 2);
        assert_eq!(old_content(&entries[0]), Some("a-old"));
        assert_eq!(new_content(&entries[0]), Some("a-new"));
        assert_eq!(old_content(&entries[1]), Some("b-old"));
        assert_eq!(new_content(&entries[1]), Some("b-new"));
    }
}
