# Memory Map

This document is the navigation index for the current architecture after the app/core/infra refactor work that has already landed. It records what is true today, plus the main transitional seams that still exist on purpose.

## What is already true

- `src/app/ports.rs` is the app-facing entrypoint for repo reads, repo writes, welcome-worker flows, repo-worker flows, and GitHub auth persistence.
- `src/core/ports.rs` is split into focused traits (`GitBranchReadPort`, `GitRemoteSyncPort`, `GitTagPort`, etc.); `GitPort` and `GitHubPort` remain only as compatibility composition traits.
- `src/infra/core_ports.rs` implements those focused traits with `InfraGitPort` and `InfraGitHubPort`.
- `src/state.rs` groups `AppState` into focused substates: `RepoState`, `WorktreeState`, `InspectorState`, `CommitState`, `DialogState`, and `UiState`.
- `src/app/shell.rs` is no longer one giant toolbar method; the repo chrome is split across `RepoToolbarModel`, `show_repo_tabs_panel`, `show_repo_menu`, `show_repo_tab_strip`, `show_repo_toolbar_actions`, and smaller helpers.
- `src/infra/github/pulls.rs` resolves GitHub owner/repo from the repository's origin remote and uses an HTTPS-origin helper when deciding whether app GitHub auth is required.
- Several UI boundaries are narrowed already (`bottom_bar`, `file_panel`, `diff_panel`, `history_panel`), but some panels and dialogs still receive broad `AppState` access.
- `src/git_ops.rs` still exists as a compatibility shim. It is no longer the preferred seam for new work.

## Layer rules

| Layer | Owns | May depend on | Must not depend on |
| --- | --- | --- | --- |
| `ui/` | egui rendering, view structs, dialog output objects, user intent capture | `shared/`, app-owned view slices, tiny pure formatting helpers | `git2`, `reqwest`, keyring, filesystem, browser, worker threads, direct GitHub/git IO |
| `app/` | application composition, repo tab lifecycle, dialog orchestration, worker polling, action dispatch, app-facing facades in `app/ports.rs` | `core/`, `infra/`, `shared/`, `ui/`, `state`, `worker` | raw HTTP/git details spread through controllers, business rules hidden inside egui render callbacks |
| `core/` | use-case orchestration and remote/auth policy | `shared/`, `core::ports` | egui, `git2`, `reqwest`, keyring, filesystem/browser APIs |
| `infra/` | concrete git, GitHub, browser, keychain, and repository adapters; implementations of core ports | `shared/`, external IO crates | egui, app state, app controllers |
| `shared/` | DTOs, request/response structs, action envelopes, cross-layer enums | nothing local | app-specific mutable state, egui, IO |

Transitional note: `git_ops.rs` is a compatibility facade that still re-exports older entrypoints. Treat it as legacy glue, not as the target dependency direction.

## Current runtime flow

1. `main.rs` creates `app::GitGuiApp`.
2. `src/app.rs` owns the app root, welcome-level state, open repo tabs, and root dialog state that is not stored per-tab.
3. Each repo tab stores a `Repository`, a `RepoWorker`, and a grouped `AppState` from `src/state.rs`.
4. `src/app/shell.rs` renders the shell. It builds a `RepoToolbarModel`, renders repo chrome through smaller helpers, and calls into `ui/*` modules.
5. `ui/*` modules render from either focused view structs (`BottomBarView`, `FilePanelState`, `DiffPanelState`, `HistoryPanelView`) or, for older seams, `&mut AppState` (`commit_panel` and several dialog modules). They emit `UiAction` values or dialog outputs instead of performing IO directly.
6. `src/app/actions.rs` handles synchronous `UiAction` values through `AppRepoRead` and `AppRepoWrite`.
7. `src/app/repo.rs`, `src/app/dialogs.rs`, and `src/app/worker_events.rs` coordinate welcome flows, repo tab lifecycle, dialog submission, and worker result application through `AppWelcomeWorkerOps`, `AppRepoWorkerOps`, and `AppGitHubAuth`.
8. `src/app/ports.rs` translates app needs into low-level infra calls and core-service calls.
9. `src/core/*/service.rs` implements the orchestration logic using focused traits from `src/core/ports.rs`.
10. `src/infra/core_ports.rs` binds those traits to `infra/git/*`, `infra/github/*`, and `infra/system/*` implementations.
11. `src/worker.rs` still runs background welcome/repo tasks and feeds typed results back to `app/worker_events.rs`.
12. Updated `AppState` substates are rendered again by the shell and UI panels.

## Current layout snapshot

```text
src/
├── main.rs
├── app.rs
├── app/
│   ├── actions.rs
│   ├── dialogs.rs
│   ├── helpers.rs
│   ├── ports.rs
│   ├── repo.rs
│   ├── shell.rs
│   └── worker_events.rs
├── core/
│   ├── mod.rs
│   ├── ports.rs
│   ├── publish/
│   │   ├── mod.rs
│   │   └── service.rs
│   ├── sync/
│   │   ├── mod.rs
│   │   └── service.rs
│   └── tags/
│       ├── mod.rs
│       └── service.rs
├── infra/
│   ├── mod.rs
│   ├── core_ports.rs
│   ├── git/
│   │   ├── clone.rs
│   │   ├── commits.rs
│   │   ├── mod.rs
│   │   ├── remotes.rs
│   │   ├── repository.rs
│   │   └── worktree.rs
│   ├── github/
│   │   ├── auth.rs
│   │   ├── mod.rs
│   │   ├── pulls.rs
│   │   └── repos.rs
│   └── system/
│       ├── browser.rs
│       ├── keychain.rs
│       └── mod.rs
├── shared/
│   ├── actions.rs
│   ├── conflicts.rs
│   ├── diff.rs
│   ├── git.rs
│   ├── github.rs
│   └── mod.rs
├── ui/
│   ├── bottom_bar.rs
│   ├── commit_panel.rs
│   ├── commit_view.rs
│   ├── diff_panel.rs
│   ├── diff_view.rs
│   ├── file_panel.rs
│   ├── history_panel.rs
│   ├── mod.rs
│   └── dialogs/
│       ├── branch.rs
│       ├── cleanup_branches.rs
│       ├── clone_repo.rs
│       ├── discard.rs
│       ├── github_auth.rs
│       ├── log_viewer.rs
│       ├── mod.rs
│       ├── publish_repo.rs
│       ├── settings.rs
│       └── tag.rs
├── commit_rules.rs
├── git_ops.rs
├── logging.rs
├── settings.rs
├── state.rs
└── worker.rs
```

## Current module map

| Path | Current role | Already in place | Still transitional |
| --- | --- | --- | --- |
| `src/app.rs` | App root and top-level composition | Owns `GitGuiApp`, repo tabs, welcome worker, welcome dialogs, settings/log viewer state | Still a fairly heavy root module; welcome-only state can move out later if it grows again |
| `src/app/ports.rs` | App-facing facade layer | Splits app calls into `AppRepoRead`, `AppRepoWrite`, `AppWelcomeWorkerOps`, `AppRepoWorkerOps`, `AppGitHubAuth` | Some flows still call infra directly here instead of going through a more explicit request/response boundary |
| `src/app/shell.rs` | Shell rendering and toolbar coordination | Toolbar/menu/tab rendering is decomposed into smaller helpers and a `RepoToolbarModel` | Still mixes shell rendering with some app-state mutation and routing |
| `src/app/actions.rs` | Synchronous action handling | Uses app facades instead of routing everything through `git_ops.rs`; refreshes grouped substates after mutations | Still tied to `Repository` + broad tab context; not every action is shaped as a narrower use-case request |
| `src/app/repo.rs` | Repo tab lifecycle and welcome actions | Handles open/add-tab flows and welcome dialog entrypoints cleanly around grouped `AppState` | Root-level dialog state still lives in `app.rs` |
| `src/app/dialogs.rs` | Dialog controllers | Keeps file pickers, browser launches, worker starts, and persistence outside of render-only UI modules | Still centralized; branch/tag/discard/cleanup/publish/clone/settings flows share one controller file |
| `src/app/worker_events.rs` | Worker result application | Applies typed welcome/repo task results back into app/tab state | Worker/app seam is cleaner, but still app-owned instead of a narrower boundary module |
| `src/app/helpers.rs` | App refresh/reset helpers | Understands grouped substates and selected-file refresh behavior | Still a catch-all for app-local helpers |
| `src/state.rs` | App-owned view state | `AppState` is split into focused substates with `refresh_parts_mut()` for refresh plumbing | Some UI modules still take `&mut AppState` instead of only the substate they need |
| `src/worker.rs` | Background execution wrapper | Generic worker core plus typed welcome/repo task wrappers | Still dispatches through app-facing worker ops; not every worker/core seam is fully port-shaped |
| `src/core/ports.rs` | Core-side dependency boundary | Focused traits define read/sync/tag/bootstrap/worktree/GitHub capabilities; compatibility composition remains | New code should prefer focused traits, not the composed compatibility traits |
| `src/core/sync/service.rs` | Push/pull/reset orchestration | Auth policy for GitHub HTTPS remotes lives here and uses injected ports | Still a small cluster rather than a broader sync domain module |
| `src/core/tags/service.rs` | Tag workflow orchestration | Validates branch/tag rules, pushes via injected ports, rolls back failed remote pushes | Branch eligibility and tag suggestion helpers still partly live below the core service boundary |
| `src/core/publish/service.rs` | Publish workflow orchestration | Bootstraps repo, stages/commits if needed, creates remote repo, then reuses sync push logic | Still uses shared GitHub DTOs directly instead of a larger dedicated publish boundary module |
| `src/infra/core_ports.rs` | Concrete port adapters | `InfraGitPort` and `InfraGitHubPort` implement the focused core traits | Opens repositories per operation; acceptable for now, but still adapter glue rather than a richer gateway layer |
| `src/infra/git/*` | Low-level git adapters | Repository/worktree/remotes/clone/commits behavior is split by IO concern; `commits.rs` is read-only history diffing (commit vs first parent) | Some functions still power both new ports and the legacy `git_ops` shim |
| `src/infra/github/auth.rs` / `repos.rs` / `pulls.rs` | GitHub HTTP + auth adapters | Auth persistence, repo APIs, and PR prompt detection are separated; PR lookup derives owner/repo from the repo's origin remote | Still tightly coupled to current GitHub API shapes; no separate request/response modules yet |
| `src/infra/system/*` | Browser/keychain adapters | Keeps desktop side effects out of `app/` and `core/` | Likely stable as-is |
| `src/ui/bottom_bar.rs`, `file_panel.rs`, `diff_panel.rs`, `history_panel.rs`, `commit_view.rs` | Narrowed render modules | Already consume focused view/state structs instead of the whole `AppState` | Good pattern to copy elsewhere |
| `src/ui/diff_view.rs` | State-free diff renderers | Unified patch table, side-by-side panes, and the status badge shared by the file list and commit view | Only the commit view uses the side-by-side renderer so far; the Changes tab still shows the unified table |
| `src/shared/diff.rs` | Patch parsing and pairing | Unified-diff parsing and side-by-side pairing live outside `ui/`, with unit tests | — |
| `src/ui/commit_panel.rs` and `src/ui/dialogs/{branch,cleanup_branches,discard,tag}.rs` | Older UI seams | Still render-only and emit actions/output | Still take broad `AppState` access and are the next UI narrowing candidates |
| `src/shared/*` | Cross-layer contracts | Holds `UiAction`, git summaries, conflict models, auth/repo/PR DTOs | Stable boundary; keep app-specific state out |
| `src/git_ops.rs` | Compatibility facade | Re-exports old helpers and forwards service calls into the new core/infra structure | Still present for compatibility/tests; should keep shrinking until callers can stop depending on it |
| `src/commit_rules.rs`, `src/settings.rs`, `src/logging.rs` | Root-level support modules | Still work with the new structure without blocking the refactor | Remain root-level holdovers; move only when a clear home and consumer boundary emerges |

## Hotspot leak map

| Hotspot | What improved already | What still leaks / next realistic move |
| --- | --- | --- |
| `git_ops.rs` compatibility shim | Core logic already lives in `core/*`, and app code now mostly goes through `app/ports.rs` | Remaining callers/tests still keep the shim alive; continue deleting wrappers as callers migrate |
| `worker.rs` -> app/core seam | Worker execution is typed and separated into welcome vs repo tasks | Tasks still invoke `AppWelcomeWorkerOps` / `AppRepoWorkerOps` instead of a thinner worker-domain boundary |
| Broad `AppState` UI access | `bottom_bar`, `file_panel`, `diff_panel`, and `history_panel` already use focused view structs | `commit_panel` plus branch/tag/discard/cleanup dialogs still take `&mut AppState`; narrow these next |
| `app/dialogs.rs` central controller | Render-only dialog code already lives under `ui/dialogs/*` | Controller logic is still concentrated; split only if a specific dialog family becomes painful to maintain |
| `src/app.rs` as a large root | Core tab/app ownership is clearer than before | Welcome-only state, settings dialog state, and top-level dialog structs still sit in the root module |

## Recommended extraction order from here

1. Narrow the remaining UI boundaries that still take `&mut AppState` (`commit_panel`, `branch`, `tag`, `discard`, `cleanup_branches`).
2. Re-shape worker dispatch so background tasks depend on explicit app/core boundary types instead of app-specific worker-op facades.
3. Keep shrinking `src/git_ops.rs` until it is only the minimum compatibility surface or can be deleted outright.
4. Split `src/app.rs` only after the UI and worker seams are smaller; move welcome/dialog state out when that reduces real coupling instead of just moving code around.
5. Move root-level support modules (`settings.rs`, `logging.rs`, maybe parts of `commit_rules.rs`) only when a stable destination in `infra/` or `core/` is actually needed.

## Do not move first

- `src/main.rs`
- The already-narrowed UI modules (`bottom_bar`, `file_panel`, `diff_panel`, `history_panel`) unless a concrete feature forces a better shared view type
- `src/infra/core_ports.rs` trait wiring, unless the port surface changes again
- `src/settings.rs`, `src/logging.rs`, and `src/commit_rules.rs` unless a new boundary clearly needs them elsewhere

## Naming guideline for new modules

Prefer small files with explicit intent:

- `service.rs` for orchestration
- `ports.rs` for dependency boundaries
- `request.rs` / `response.rs` for boundary models when a flow gets larger
- `error.rs` for explicit failure types
- `mod.rs` only as a thin module index
