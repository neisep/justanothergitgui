# Review fixes

Scope: the four reproduced file-handling bugs from the project review.
Implementation is delegated to an agent with no conversation history; the parent
agent independently reviews the changes and verifies the original reproductions.

- [x] Preserve ignored files when cleaning untracked directories. Remove only
  eligible untracked files, preserve nested repositories, and report failures.
- [x] Show unstaged and staged file-type changes, and ensure Unstage All restores
  their index entries without changing working-tree contents.
- [x] Stage dangling symlinks as symlinks, distinguishing missing paths from
  filesystem errors.
- [x] Display untracked text-file contents before staging, keeping exact-path
  filtering and existing staged diffs intact.
- [x] Add focused regression tests and demonstrate they fail before the fixes.
- [x] Independently review the diff, rerun the reproductions, and run the full
  test suite.

Do not include UI redesign, unrelated refactoring, or changes to STRUCTURE.md.

Validation: eight regression tests failed against the original implementation.
After the fixes, all 159 tests pass with `cargo test --offline`, and
`git diff --check` passes. The parent independently reran the original
reproductions, including preservation of a nested repository inside an
untracked directory. Verification ran on Linux; no desktop interaction test
was performed.

# UX review improvements

Scope: the eight findings from the screenshot/code UX review. Preserve the prior
review fixes and existing user changes in documentation and screenshots.

- [x] UX1: Conflicted files use Resolve; ordinary staging (including drag,
  shortcut, and bulk) cannot bypass saving the merge preview.
- [x] UX2: Block saving while a custom conflict edit is unapplied, including
  an action-layer guard and regression coverage.
- [x] UX3: Shared conflict actions with explicit Use current / Use incoming /
  Keep both / Edit result / Reset resolution / Keep neither semantics; show
  that selected resolutions are not saved.
- [x] UX4: Show staged filenames and count beside an explicit Commit N files
  action; display blocking reasons inline.
- [x] UX5: Improve text/control contrast, primary-action visibility, and prevent
  truncated staging buttons.
- [x] UX6: Collapsible commit panel during conflict resolution, source branch
  identity where available, and previous/next conflict navigation.
- [x] UX7: Hide raw patch metadata behind Patch details by default; retain
  function context and line numbers, emphasize additions/deletions.
- [x] UX8: Filtered bulk actions visibly disclose their full scope/count.
- [x] Verification: focused behavior regressions, full test suite, build,
  formatting/diff checks, and rendered UI verification where supported.

Completed 2026-09-05. The parent implemented review/commit/readability changes,
one agent implemented merge fixes, and a second agent added independent widget
interaction tests. The parent reviewed the combined changes and ran native GUI
verification on isolated copies of the screenshot demo repositories.

Verification evidence:

- UX1: Real Git merge regression rejects ordinary Stage and Stage All while
  preserving conflict index entries and disk bytes. Unrelated individual files
  still stage normally. Resolve pointer test emits selection instead of staging.
  Bulk staging is deliberately blocked during conflicts with an explicit error.
- UX2: Draft-save guard tested in state and used by both UI/action handler.
  Actual pointer tests cover Edit -> disabled Save -> Apply -> enabled Save.
  Native editor inspection confirmed selected text seeds the custom draft.
- UX3: Regression covers every resolution seed, reset becoming unresolved,
  and Keep neither removing the region. Keep both preview matches saved text.
  Native selection from both sides shows the unsaved status and shared controls.
- UX4: Widget tests check staged filenames/count, review click target, visible
  blocking reasons, and disabled commit dispatch. Native review screenshot shows
  an unstaged diff alongside the separately identified staged README commit.
- UX5: Native inspection confirmed improved contrast and visible primary actions.
  Widget clipping tests cover Resolve, staged filename/actions, and two initial
  unstaged rows with production button padding. Startup theme-reset ordering and
  initial section height were corrected after native inspection.
- UX6: Pointer tests exercise Previous/Next scrolling and commit panel collapse/
  expansion with draft preservation. Source identities checked against a real
  merge. Viewport/clip tests run at 640 and 960 widths. Native testing caught and
  fixed a save row consuming the viewport and source panes overlapping Result.
- UX7: Widget test clicks Patch details and checks metadata hidden/expanded.
  Native screenshot confirms retained line numbers/context and +/- highlights.
- UX8: Pointer test checks total count and hidden-file disclosure before bulk
  dispatch. Native filtered screenshot confirms labels fit and Clear is readable.

Final checks: `cargo test --offline` (172 passed, including 13 new regressions),
`cargo build --offline`, `cargo fmt --all --check`, and `git diff --check` pass.
Native Linux verification used the final debug build at 1280x820. Saving the
mixed-line merge through the GUI was independently checked: working file equals
staged blob, selected values are present, and no conflict stages/markers remain.
Windows and macOS desktop behavior was not tested.

Screenshots: [review](docs/ux-review/review.png),
[merge](docs/ux-review/merge.png), [filtered files](docs/ux-review/filter.png).
Existing root screenshots and user documentation edits were preserved.
