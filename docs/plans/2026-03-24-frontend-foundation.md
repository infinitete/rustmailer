# Frontend Foundation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为 `frontend/` 接入路径别名、Tailwind CSS、shadcn-ui、`react-router-dom` 与 Redux，并保持现有页面内容基本不变。

**Architecture:** 通过 Vite 与 TypeScript 统一 `@` 别名，在入口处挂载 Router 与 Redux Provider，并用最小文件集完成 Tailwind 与 shadcn-ui 初始化。保留现有 `App` 页面主体，只把它放入可扩展的路由壳中。

**Tech Stack:** Vite 8, React 19, TypeScript 5, Tailwind CSS, shadcn-ui conventions, react-router-dom, @reduxjs/toolkit, react-redux

---

### Task 1: Install dependencies and prepare config targets

**Files:**
- Modify: `frontend/package.json`
- Create: `frontend/package-lock.json`

**Step 1: Update dependency manifest**

Add the required runtime and development packages for Tailwind CSS, shadcn-ui prerequisites, routing, and Redux.

**Step 2: Install dependencies**

Run: `cd frontend && npm install`
Expected: dependencies install successfully and `package-lock.json` is created or updated.

**Step 3: Verify dependency graph is usable**

Run: `cd frontend && npm ls --depth=0`
Expected: the newly added packages appear without missing peer dependency errors.

### Task 2: Configure aliases and style tooling

**Files:**
- Modify: `frontend/vite.config.ts`
- Modify: `frontend/tsconfig.app.json`
- Create: `frontend/components.json`
- Create: `frontend/postcss.config.js`

**Step 1: Configure Vite alias**

Add `@` to resolve to `src`.

**Step 2: Configure TypeScript alias**

Add `baseUrl` and `paths` so editor, type checker, and build all agree.

**Step 3: Add shadcn-ui metadata**

Create `components.json` using the alias and styling conventions that match the project layout.

**Step 4: Add PostCSS/Tailwind wiring**

Create the PostCSS config required by Tailwind in this Vite project.

**Step 5: Verify config compiles**

Run: `cd frontend && npm run build`
Expected: configuration files are accepted and the app builds.

### Task 3: Add runtime scaffolding for router, store, and shadcn helpers

**Files:**
- Modify: `frontend/src/main.tsx`
- Create: `frontend/src/router/index.tsx`
- Create: `frontend/src/store/index.ts`
- Create: `frontend/src/lib/utils.ts`
- Modify: `frontend/src/index.css`

**Step 1: Add `cn` utility**

Create the helper used by shadcn-ui based on `clsx` and `tailwind-merge`.

**Step 2: Create Redux store**

Create a minimal `configureStore` setup and export store types.

**Step 3: Create router entry**

Create a minimal browser router that renders the existing `App` at `/`.

**Step 4: Wire providers in `main.tsx`**

Wrap the app with Redux `Provider` and `RouterProvider`, switching imports to `@` aliases where appropriate.

**Step 5: Add Tailwind and theme globals**

Update `src/index.css` to load Tailwind and define the CSS variables expected by shadcn-ui while preserving existing app styles.

**Step 6: Verify runtime scaffolding**

Run: `cd frontend && npm run build`
Expected: the application still type-checks and bundles successfully.

### Task 4: Validate and polish

**Files:**
- Review: `frontend/src/App.tsx`
- Review: `frontend/src/App.css`
- Review: `frontend/src/index.css`

**Step 1: Run lint**

Run: `cd frontend && npm run lint`
Expected: ESLint passes without new errors.

**Step 2: Run final build**

Run: `cd frontend && npm run build`
Expected: production build passes.

**Step 3: Adjust any import or style regressions**

Make only the minimal fixes required to keep the current page working under the new infrastructure.
