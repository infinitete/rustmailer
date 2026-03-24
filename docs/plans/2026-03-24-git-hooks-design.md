# Git Hooks Design

**Date:** 2026-03-24

## Goal

为仓库增加原生 Git hooks，确保每次提交前自动执行前后端质量检查，并限制提交信息符合 Conventional Commits。

## Current State

- 仓库同时包含 `server/` Rust 后端与 `frontend/` React + TypeScript 前端。
- 目前没有任何已提交到仓库的 Git hooks。
- 前端已有 `npm run lint` 与 `npm run build`。
- 后端可使用 `cargo fmt --check` 与 `cargo test` 做提交前校验。

## Constraints

- 不引入 Husky、lefthook 或其他额外 hooks 管理依赖。
- hooks 必须直接由 Git 原生机制执行。
- 安装方式必须简单，开发者只需要执行一次初始化脚本。
- 校验失败时要给出明确错误提示，不能静默退出。

## Options Considered

### Option 1: Native repository hooks with `core.hooksPath`

把 hooks 脚本提交到仓库内，通过安装脚本将 Git `core.hooksPath` 指向仓库目录。

**Pros**
- 无额外依赖。
- 后端与前端命令都能直接执行。
- 配置透明，便于审查与维护。

**Cons**
- 每个开发者需要先执行一次安装脚本。

### Option 2: Husky + commitlint

在 Node 工具链里统一管理 hooks 与提交信息校验。

**Pros**
- 前端生态成熟。
- 社区已有大量现成配置。

**Cons**
- 仓库根目录目前不是 Node 项目。
- 会把纯 Git 问题转成额外的 JavaScript 依赖管理问题。

### Option 3: Lefthook

使用独立 hooks 管理器统一多语言项目校验。

**Pros**
- 配置集中，跨语言支持好。

**Cons**
- 需要团队额外安装工具。
- 对当前仓库规模来说收益不足。

## Recommended Approach

采用 **Option 1**。在仓库根目录新增 `.githooks/` 存放 `pre-commit` 与 `commit-msg`，并新增 `scripts/setup-git-hooks.sh` 负责设置 `core.hooksPath`。这样可以在不增加运行时依赖的前提下，直接复用现有 Rust 和 Node 命令。

## Architecture

### Hook installation

- 新增 `scripts/setup-git-hooks.sh`。
- 脚本在仓库根目录执行 `git config core.hooksPath .githooks`。
- 脚本输出明确提示，告知 hooks 已安装。

### `pre-commit`

- 使用 POSIX shell 编写，进入仓库根目录后按固定顺序运行：
  - `cd server && cargo fmt --check`
  - `cd server && cargo test`
  - `cd frontend && npm run lint`
  - `cd frontend && npm run build`
- 对前端依赖缺失做显式检查，例如 `frontend/node_modules` 不存在时直接报错。
- 任一命令失败都中止提交。

### `commit-msg`

- 读取提交信息第一行。
- 允许 `feat|fix|docs|style|refactor|test|chore|build|ci|perf|revert`，支持可选 scope 和 `!`。
- 对 Git 自动生成的 `Merge ...`、`Revert ...`、`fixup! ...`、`squash! ...` 做兼容，避免阻断正常 Git 工作流。

## File-Level Plan

- 新增 `docs/plans/2026-03-24-git-hooks-design.md`
  - 保存本次设计结论。
- 新增 `docs/plans/2026-03-24-git-hooks.md`
  - 写出实现步骤和验证方式。
- 新增 `.githooks/pre-commit`
  - 实现提交前质量检查。
- 新增 `.githooks/commit-msg`
  - 实现提交信息校验。
- 新增 `scripts/setup-git-hooks.sh`
  - 实现一键安装 hooks。
- 新增或修改仓库文档
  - 说明首次安装与使用方式。

## Error Handling

- hooks 脚本使用 `set -eu`，避免在失败后继续执行。
- 缺失依赖或找不到命令时，输出具体操作建议。
- 提交信息不合规时，输出合法示例，减少试错成本。

## Testing And Verification

本次主要通过直接执行 hooks 脚本验证：

- `./scripts/setup-git-hooks.sh`
- `.githooks/pre-commit`
- `.githooks/commit-msg <tempfile>`
- `git config --get core.hooksPath`

同时以脚本真实调用结果为准，确认：

- 代码质量检查链条可跑通。
- 非法提交信息被拒绝。
- 合法提交信息被接受。
