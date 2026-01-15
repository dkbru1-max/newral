Refactoring Notes

Goals
- Reduce code duplication and make the repo easier to navigate.
- Separate API handlers, business logic, and data access in each service.
- Add small, targeted tests and standardize code quality tooling.

Key changes
- Added shared crate `services/common` for tracing init, env parsing, listener binding, and shutdown handling.
- Split services into modules:
  - `app`: router construction
  - `handlers`: HTTP endpoints
  - `service`: business logic
  - `db`: SQL queries and data access
  - `models`: request/response types
  - `state`: shared application state
- Validator and scheduler services now isolate sandbox logic and AI checks into dedicated modules.
- Added unit tests for policy evaluation, sandbox filename validation, and helper utilities.
- Updated Makefile targets to run `cargo fmt`, `cargo test`, and `cargo clippy` per crate.
- Docker build context now points to repo root so service images can use `services/common`.

Open follow-ups
- Consider a Rust workspace at repo root to simplify `cargo` commands.
- Add lightweight integration tests to validate the full agent -> scheduler -> validator flow.
- Expand shared types and error handling into `services/common` as APIs stabilize.
