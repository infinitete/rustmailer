# Frontend Foundation Design

**Date:** 2026-03-24

## Goal

为 `frontend/` 接入前端基础设施，包括路径别名、Tailwind CSS、shadcn-ui、`react-router-dom` 与 Redux，同时保持现有页面内容与行为基本不变。

## Current State

- 前端基于 Vite + React 19 + TypeScript。
- `src/main.tsx` 直接渲染 `App`。
- `App.tsx` 仍是默认模板页面。
- 当前没有路径别名、路由、全局状态管理、Tailwind 或 shadcn-ui 配置。
- 当前没有单独的前端测试运行器。

## Constraints

- 不做业务页面重构。
- 不新增复杂 UI。
- 不把现有样式强制迁移成 Tailwind class。
- shadcn-ui 仅完成初始化和可用能力，不批量生成组件。

## Options Considered

### Option 1: Minimal infrastructure integration

只补齐依赖、配置和最小入口接线，保留现有 `App` 作为页面主体。

**Pros**
- 改动小，风险低。
- 符合“只接入基础设施”的范围。
- 后续可以继续在此基础上扩展业务目录结构。

**Cons**
- 路由和 Redux 目前只是基础壳层。
- shadcn-ui 不会展示完整的组件使用示例。

### Option 2: Infrastructure plus app-shell refactor

在接入基础设施的同时重构为 `router`、`pages`、`providers` 的标准目录。

**Pros**
- 结构更清晰，后续开发更顺手。

**Cons**
- 会显著改动当前模板页面，超出已确认范围。

### Option 3: Full UI-system reset around shadcn-ui

统一重搭样式体系、组件层与页面结构。

**Pros**
- 长期一致性更强。

**Cons**
- 改动过大，不符合当前目标。

## Recommended Approach

采用 **Option 1**。先完成基础设施接入，并把入口组织成后续可扩展的形态，但不主动重做现有页面。

## Architecture

### Path aliases

- 在 TypeScript 与 Vite 中统一配置 `@` 指向 `src/`。
- 后续业务代码、store、lib 与组件都通过别名导入。

### Tailwind CSS

- 引入 Tailwind CSS 与 PostCSS 配置。
- 在 `src/index.css` 中放入 Tailwind 指令与基础主题变量。
- 保留现有页面 CSS，避免一次性迁移样式。

### shadcn-ui

- 添加 `components.json`。
- 新增 `src/lib/utils.ts`，提供 `cn` 工具函数。
- 配置别名以满足 shadcn 约定。
- 不批量生成组件，只完成初始化所需基础。

### Router

- 使用 `react-router-dom` 的 `createBrowserRouter` + `RouterProvider`。
- 保持单路由配置，默认路由仍渲染当前 `App`。
- 将来可在不改入口结构的情况下继续加页面。

### Redux

- 使用 `@reduxjs/toolkit` + `react-redux`。
- 建立最小 `store`，初始不放具体业务 slice。
- 在入口挂载 `Provider`，保证全局状态能力已接通。

## File-Level Plan

- 修改 `frontend/package.json`
  - 增加运行时依赖与开发依赖。
- 修改 `frontend/vite.config.ts`
  - 配置 React 插件和 `@` 别名。
- 修改 `frontend/tsconfig.app.json`
  - 增加 `baseUrl` 与 `paths`。
- 新增 `frontend/components.json`
  - 写入 shadcn-ui 初始化配置。
- 新增 `frontend/postcss.config.*`
  - 配置 Tailwind PostCSS 插件。
- 视 Tailwind 版本需要新增对应配置文件。
- 修改 `frontend/src/main.tsx`
  - 接入 Redux Provider 与 RouterProvider。
- 新增 `frontend/src/router/index.tsx`
  - 定义最小路由。
- 新增 `frontend/src/store/index.ts`
  - 定义 Redux store。
- 新增 `frontend/src/lib/utils.ts`
  - 提供 `cn` 辅助函数。
- 修改 `frontend/src/index.css`
  - 加入 Tailwind 与 shadcn 基础主题。

## Error Handling

- 入口路由仅保留最小结构，不额外引入复杂错误页。
- 若依赖安装或构建阶段出现版本兼容问题，优先调整配置而不是扩大改动范围。

## Testing And Verification

由于当前项目没有前端测试运行器，且本次改动以配置和基础脚手架为主，本次验证以：

- `npm run build`
- `npm run lint`

为主，确保类型检查、打包与静态检查通过。
