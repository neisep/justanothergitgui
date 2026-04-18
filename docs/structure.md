# Codebase Structure

Quick map of the current architecture. For file-by-file detail, use `docs/memory-map.md`.

- `src/main.rs`: native entrypoint; starts `app::GitGuiApp`.
- `src/app.rs` + `src/app/`: app orchestration layer. Owns tab lifecycle, action dispatch, dialog coordination, worker polling, and the bridge between UI intent and backend work. `app/ports.rs` is the key seam: app-facing repo/GitHub helpers plus worker ops that call core services with infra adapters.
- `src/state.rs`: app/UI boundary state. `AppState` is grouped into focused sub-structs (`RepoState`, `WorktreeState`, `InspectorState`, `CommitState`, `DialogState`, `UiState`) so the app shell and panels can pass around narrower slices.
- `src/ui/`: egui rendering only. Panels increasingly consume focused panel/view structs such as `FilePanelState`, `DiffPanelState`, `HistoryPanelView`, and `BottomBarView`, then emit `UiAction`s instead of doing IO directly.
- `src/core/`: use-case services plus narrow ports. `core/ports.rs` defines the seams; `core/sync/service.rs`, `core/publish/service.rs`, and `core/tags/service.rs` hold orchestration and rules without egui or direct git/network/system calls.
- `src/infra/`: concrete adapters behind those seams. `infra/core_ports.rs` wires `InfraGitPort` and `InfraGitHubPort` to `infra/git/*`, `infra/github/*`, and `infra/system/*`.
- `src/shared/`: DTO/common-type layer shared across boundaries, mainly in `shared/actions.rs`, `shared/git.rs`, `shared/github.rs`, and `shared/conflicts.rs`.
- `src/worker.rs`: background execution for blocking repo and welcome-screen operations.

Typical path: `ui/` renders state and emits actions -> `app/` coordinates -> `core/` applies workflow rules through ports -> `infra/` talks to git/GitHub/system -> results come back through `state.rs` and `shared/*`.
