# Repository Guidelines

## Project Structure & Module Organization
This repository currently has two top-level apps:

- `server/`: Rust backend crate. Entry point is `server/src/main.rs`; crate metadata lives in `server/Cargo.toml`.
- `frontend/`: React 19 + TypeScript + Vite client. App code is under `frontend/src/`, static assets under `frontend/public/` and `frontend/src/assets/`.

Keep backend and frontend changes isolated unless a feature requires both. Add new Rust modules under `server/src/`. Add new React components, styles, and assets near the feature they support.

## Build, Test, and Development Commands
- `cd server && cargo run`: start the Rust server locally.
- `cd server && cargo build`: compile the backend.
- `cd server && cargo test`: run Rust tests.
- `cd server && cargo fmt`: format Rust code before review.
- `cd frontend && npm install`: install frontend dependencies.
- `cd frontend && npm run dev`: start the Vite dev server.
- `cd frontend && npm run build`: type-check and produce a production build.
- `cd frontend && npm run lint`: run ESLint on TypeScript and TSX files.

## Coding Style & Naming Conventions
Follow the existing style in each app:

- Rust uses `rustfmt` defaults, 4-space indentation, `snake_case` for functions/modules, and `PascalCase` for types.
- TypeScript/TSX uses 2-space indentation, single quotes, and `PascalCase` component names such as `App.tsx`.
- Keep files focused. Prefer small modules over large mixed-responsibility files.

## Testing Guidelines
There is no committed test suite yet beyond scaffold commands, so new work should add tests with the feature when practical.

- Rust unit tests belong next to the module with `#[cfg(test)]`; integration tests can go in `server/tests/`.
- Frontend tests should use `*.test.ts` or `*.test.tsx` naming once a test runner is added.
- At minimum, run `cargo test` and `npm run lint` before opening a PR.

## Commit & Pull Request Guidelines
This repository has no commit history yet, so there is no established convention to copy. Start with short, imperative commit messages, preferably Conventional Commit style, for example `feat: add mailer config form`.

PRs should include a clear summary, affected area (`server` or `frontend`), test/verification notes, and screenshots for UI changes. Link related issues and note any follow-up work explicitly.
