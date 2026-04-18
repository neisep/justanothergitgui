# Codebase Structure

## app/
High-level application state and routing.

## ui/
All egui components and screens.
No business logic allowed.

## core/
Business logic, domain rules, validation, errors.
Pure Rust, no UI or IO.

## infra/
Filesystem, git, config, external services.

## shared/
Common types, Result<T>, error definitions.

