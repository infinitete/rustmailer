# rustmailer

Monorepo-style workspace containing:

- `server/`: Rust backend
- `frontend/`: React 19 + TypeScript + Vite frontend

## Git Hooks

This repository uses native Git hooks stored in `.githooks/`.

Install them once per local clone:

```bash
./scripts/setup-git-hooks.sh
```

The installer sets:

```bash
git config core.hooksPath .githooks
```

### `pre-commit`

Runs the following checks before every commit:

```bash
cd server && cargo fmt --check
cd server && cargo test
cd frontend && npm run lint
cd frontend && npm run build
```

If `frontend/node_modules` is missing, the hook fails immediately. Install frontend dependencies first:

```bash
cd frontend && npm install
```

### `commit-msg`

Validates the first line of each commit message using Conventional Commits.

Accepted examples:

```text
feat(frontend): add compose form
fix(server): handle missing smtp host
chore: update repository hooks
```

## Development

Backend:

```bash
cd server && cargo run
```

Frontend:

```bash
cd frontend && npm run dev
```
