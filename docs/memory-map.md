# Memory Map

This document is the navigation index for the current codebase and the phase-1 refactor target.

## Layer rules

| Layer | Owns | May depend on | Must not depend on |
| --- | --- | --- | --- |
| `ui/` | egui rendering, widgets, view callbacks | `app/`, `shared/`, tiny pure formatting helpers | `git2`, `reqwest`, `keyring`, filesystem, browser, GitHub APIs |
| `app/` | app shell, routing, tab coordination, dialog state, worker polling, action dispatch | `core/`, `infra/`, `shared/`, `ui/` | direct business rules hidden inside large view methods |
| `core/` | pure business rules, validators, use-case orchestration, domain errors | `shared/` | egui, git/network/keychain/file IO |
| `infra/` | git, GitHub HTTP, keychain, filesystem, browser, config | `shared/` | egui |
| `shared/` | DTOs, request/response structs, enums, cross-layer errors | nothing local | app-specific state machines, UI code, IO |

## Current runtime flow

1. `main.rs` creates `app::GitGuiApp`.
2. `app.rs` defines the app root and shared app-owned state.
3. `ui/*` reads `AppState` and pushes `UiAction` values back into it.
4. `app/actions.rs`, `app/repo.rs`, `app/worker_events.rs`, `app/shell.rs`, and `app/dialogs.rs` own app-state transitions, while `ui/dialogs/*` renders dialog views and returns user intent.
5. `git_ops.rs` performs git, GitHub, keychain, filesystem, and browser work.
6. Results flow back into `AppState`, then `ui/*` renders the new snapshot.

## Current module map

| Current path | Current role | Main owned types / functions | Main dependencies | Phase-1 target |
| --- | --- | --- | --- | --- |
| `src/main.rs` | Native entry point | `main()` | `app` | Keep small; stays entry point |
| `src/app.rs` | App root and shared app-owned state | `GitGuiApp`, `RepoTab`, dialog state structs, `eframe::App` integration | `app/*`, `commit_rules`, `git_ops`, `logging`, `settings`, `shared`, `state`, `ui`, `worker`, `eframe::egui` | Keep shrinking into a thin module root |
| `src/app/helpers.rs` | App-local refresh and view-state helpers | `refresh_status`, selected-file sync, repo label/path helpers, UI-safe error text | `commit_rules`, `git_ops`, `logging`, `state`, `shared` | Later split into `refresh.rs` + small helper files if it grows again |
| `src/app/repo.rs` | Repo tab lifecycle and global dialog entrypoints | open repo, add tab, status routing, GitHub sign-in start | `git_ops`, `worker`, `settings`, `state` | Stable app coordinator seam |
| `src/app/actions.rs` | Synchronous `UiAction` dispatch | staging, commit, branch, tag, discard, PR action handlers | `git_ops`, `shared/actions`, `state` | Later point at narrower `core/*` seams instead of `git_ops` facade |
| `src/app/worker_events.rs` | Background task result handling | welcome worker + per-tab worker polling and state updates | `git_ops`, `worker`, `shared/github`, `state` | Stable worker-event boundary |
| `src/app/shell.rs` | Top-level shell rendering and keyboard routing | welcome screen, repo tabs, log window controller, shortcuts | `ui/*`, `commit_rules`, `state`, `shared/actions` | Good seam; keep dialog orchestration out of here |
| `src/app/dialogs.rs` | Dialog controllers and side-effect boundaries | settings save, file picker dispatch, publish/clone worker start, branch/tag/discard action dispatch, GitHub browser launch | `commit_rules`, `git_ops`, `settings`, `shared/actions`, `ui` | Optional later split into `app/dialogs/*.rs` if controller logic grows again |
| `src/ui/dialogs/*.rs` | Dialog rendering components | settings, clone, publish, branch, tag, cleanup, discard, auth, and log viewer windows | `egui`, app dialog/view state, shared prompt types, `ui::commit_panel` helper | Keep render-only; no IO, worker dispatch, file dialogs, or browser launch |
| `src/state.rs` | App-specific view state | `AppState`, `SelectedFile`, `CenterView`, `DragFile`, `BusyState` | `shared` | Split into `src/app/view_state.rs` |
| `src/shared/*.rs` | Cross-layer DTOs and action enums | `UiAction`, git summaries/previews, conflict models, GitHub auth/repo request-response types | `serde`, std | Keep growing as the stable boundary between `app`, `ui`, `worker`, and future `infra/core` modules |
| `src/git_ops.rs` | Thin compatibility facade | public wrappers and compatibility re-exports | `core/*`, `infra/*`, `shared` | Keep shrinking as `app` consumes `core` directly |
| `src/core/sync/service.rs` | Sync workflow orchestration | push, pull, reset-to-remote flows | `infra/git`, `infra/github`, `shared` | Good seam for future explicit request/error types |
| `src/core/tags/service.rs` | Tag workflow orchestration | tag validation, local create, remote push, rollback | `infra/git`, `infra/github`, `shared` | Stable service boundary |
| `src/core/publish/service.rs` | Publish workflow orchestration | initialize repo, stage/commit, create remote repo, add origin, push | `infra/git`, `infra/github`, `shared` | Stable service boundary |
| `src/infra/git/*` | Low-level git adapters | repository discovery/state, branch/tag helpers, clone, remotes, worktree/diff/conflict IO | `git2`, `shared` | Continue splitting only if individual files grow too large |
| `src/infra/github/*` | Low-level GitHub adapters | device auth, token persistence/verification, repo APIs, pull-request lookup, GitHub remote parsing | `reqwest`, `shared`, `git2` for remote inspection | Stable home for GitHub API work |
| `src/infra/system/*` | Environment adapters | browser opening, keychain entry access | `webbrowser`, `keyring` | Expand here for future filesystem/process adapters |
| `src/worker.rs` | Background execution wrapper for blocking operations | `Worker`, `WorkerTask`, `TaskResult` | `git_ops`, `shared/github`, std channels/threading | Move to `src/app/worker.rs`; later split task definitions from queue/polling if needed |
| `src/commit_rules.rs` | Commit message rules and scope inference | `CommitMessageRuleSet`, validation/suggestion helpers | `regex`, `serde`, `std::path` | Good candidate for `src/core/commit_message/*`; low-priority move |
| `src/settings.rs` | Settings persistence | `AppSettings`, load/save helpers | `serde`, filesystem, env, `commit_rules` | Move to `src/infra/config/settings.rs` |
| `src/logging.rs` | Log persistence and sanitization | `AppLogger`, `sanitize_log_text`, `summarize_for_ui` | filesystem, env | Move to `src/infra/logging.rs` |
| `src/ui/mod.rs` | Shared UI helpers | `show_inline_busy` | `egui` | Keep under `ui/` |
| `src/ui/file_panel.rs` | Unstaged/staged file lists and drag/drop | `show`, table/render/drop helpers | `AppState`, `UiAction` | Keep under `ui/`; later optional `ui/panels/files.rs` |
| `src/ui/commit_panel.rs` | Commit editor and prefix suggestions | `show`, `show_prefix_suggestions` | `AppState`, `UiAction`, `commit_rules` | Keep under `ui/`; may later depend on a smaller `core::commit_message` facade |
| `src/ui/diff_panel.rs` | Center view switcher, diff view, conflict resolution UI | `show`, diff/conflict render helpers | `AppState`, `CenterView`, `UiAction` | Keep under `ui/`; no IO should move here |
| `src/ui/history_panel.rs` | Commit history list and graph lane rendering | `show`, `draw_graph_lane` | `AppState` | Keep under `ui/` |
| `src/ui/bottom_bar.rs` | Status/footer bar | `show` | `AppState` | Keep under `ui/` |

## Hotspot leak map

These are the phase-1 seams to fix first.

| Leak | Why it hurts | First move |
| --- | --- | --- |
| `git_ops.rs` is still the worker/app entrypoint | The workflow logic is now in `core/*`, but `app.rs` and `worker.rs` still depend on the compatibility facade | Split app shell next and point it at narrower service seams |
| `app/dialogs.rs` still owns all dialog orchestration | Rendering is now in `ui/dialogs/*`, but app-side controller logic is still centralized | Split controllers by dialog family only if the file becomes a hotspot again |
| `worker.rs` still dispatches through `git_ops` | The request/result models are shared now, but the execution boundary still points at the compatibility facade | Repoint worker tasks at smaller infra/core entry points after the next split |
| `ui/*` currently depends on broad `AppState` | UI panels read a large mutable state bag, which encourages accidental coupling | Keep phase 1 on file layout first, then narrow panel inputs later |

## Phase-1 target tree

```text
src/
├── main.rs
├── app.rs
├── app/
│   ├── actions.rs
│   ├── dialogs.rs
│   ├── helpers.rs
│   ├── repo.rs
│   ├── shell.rs
│   └── worker_events.rs
├── core/
│   ├── commit_message/
│   ├── branches/
│   ├── tags/
│   ├── publish/
│   ├── pull_requests/
│   └── conflicts/
├── infra/
│   ├── git/
│   │   ├── repository.rs
│   │   ├── status.rs
│   │   ├── staging.rs
│   │   ├── branches.rs
│   │   ├── tags.rs
│   │   ├── history.rs
│   │   ├── remotes.rs
│   │   └── conflicts.rs
│   ├── github/
│   │   ├── auth.rs
│   │   ├── repos.rs
│   │   └── pulls.rs
│   ├── config/
│   │   └── settings.rs
│   ├── logging.rs
│   └── system/
│       ├── browser.rs
│       └── keychain.rs
├── shared/
│   ├── actions.rs
│   ├── git.rs
│   ├── github.rs
│   ├── repo.rs
│   ├── conflicts.rs
│   └── errors.rs
└── ui/
    ├── mod.rs
    ├── bottom_bar.rs
    ├── commit_panel.rs
    ├── diff_panel.rs
    ├── file_panel.rs
    ├── history_panel.rs
    └── dialogs/
        ├── branch.rs
        ├── cleanup_branches.rs
        ├── clone_repo.rs
        ├── discard.rs
        ├── github_auth.rs
        ├── log_viewer.rs
        ├── publish_repo.rs
        ├── settings.rs
        └── tag.rs
```

## Recommended extraction order

1. Move cross-layer DTOs out of `state.rs` and `git_ops.rs` into `shared/`.
2. Split `git_ops.rs` by IO concern: `infra/git`, `infra/github`, `infra/system`.
3. Introduce small `core/*` services only where orchestration or validation is real.
4. Split the remaining dialog/controller hotspots into smaller app or ui modules where that creates a cleaner boundary.
5. Keep `ui/*` render-only and let it emit actions instead of performing work.

## Do not move first

- `src/main.rs`
- `src/ui/*` panel internals, unless a dialog is being extracted from `app.rs`
- `src/commit_rules.rs`, `src/settings.rs`, `src/logging.rs` unless the hotspot split creates a clean path

## Naming guideline for new modules

Prefer small files with explicit intent:

- `service.rs` for orchestration
- `validator.rs` for rules
- `request.rs` / `response.rs` for boundary models
- `error.rs` for explicit failure types
- `mod.rs` only as a thin module index
