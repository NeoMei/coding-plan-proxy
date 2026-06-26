# AGENTS.md — CodexProxy

Tauri v2 desktop app + React/Vite frontend + embedded Node.js proxy. The app lets Codex talk to Chinese Coding Plan APIs by translating OpenAI Responses → Anthropic Messages (or Chat Completions) at `127.0.0.1:15731`.

## Code retrieval

This repo is indexed by `codebase-memory`. When searching for symbols, call chains, usages, or architecture, **prefer `codebase-memory-mcp` tools over grep/glob/manual file walks**:

- `search_graph` — find functions, classes, routes, variables by name or natural-language query.
- `query_graph` — complex Cypher queries, aggregations, and quality audits.
- `trace_path` — follow callers, callees, or data flow.
- `get_code_snippet` — read a specific symbol after locating it with `search_graph`.
- `get_architecture` — high-level package/service/cluster overview.

If the index is stale (new files or large refactors), re-index with `codebase-memory-mcp_index_repository` in `full` mode before querying.

## Repo layout

- `src/` — React + TypeScript frontend (Vite, Tailwind v4, strict TS, `erasableSyntaxOnly`).
- `src-tauri/` — Rust Tauri backend; runs the proxy child process, SQLite DB, tray menu, Codex config writer.
- `proxy/` — Standalone Node.js proxy (`index.mjs`). Bundled into the app via `tauri.conf.json` bundle resources.
- `src-tauri/src/commands.rs` — All Tauri invoke commands the UI calls.
- `src-tauri/src/codex_config.rs` — Writes Codex `config.toml`, `models.json`, and auth file.
- `src/data/presets.ts` — Built-in provider presets (Kimi, GLM, DeepSeek, Volc, Bailian, OpenAI, Anthropic, Google).

## Dev commands

```bash
npm install                 # installs root deps only; proxy/ has no npm deps
npm run dev                 # Vite dev server on :5173
npm run tauri dev           # Tauri dev mode (starts Vite via beforeDevCommand)
npm run tauri build         # Production build (Vite build + Tauri bundle)
npm run build               # Vite production build only (also runs `tsc -b`)
npm run lint                # oxlint (no formatter configured)
```

Verify manually with `npm run tauri dev`. For Rust-only changes, run `cargo check` in `src-tauri/`.

## Type-checking

- `npm run build` runs `tsc -b`, which checks both `tsconfig.app.json` (src) and `tsconfig.node.json` (vite.config.ts) via project references.
- No typecheck script exists; use `npm run build` or `npx tsc -b` for verification.

## Proxy

- The proxy is plain Node.js stdlib (`http`, `fs`, `path`). It has **zero npm dependencies**.
- Run standalone: `cd proxy && node index.mjs` (or, from `proxy/`, `npm start` / `npm run dev` for watch mode).
- Default bind: `127.0.0.1:15731`. Override with `PROXY_PORT` / `PROXY_BIND` env vars.
- Configuration sources (in order): `CODING_PLAN_PROVIDERS` JSON env var, `~/.coding-plan-proxy.json`, `./proxy/config.json`, or `CODING_PLAN_*` env vars.
- Protocol detection: if `upstream` contains `/anthropic`, proxy speaks Anthropic Messages; otherwise Chat Completions.

## Rust backend notes

- Rust 1.77.2+ required; edition 2021.
- Bundles `proxy/index.mjs` as a resource and discovers it at runtime by walking up from `current_exe()`.
- Uses SQLite via `rusqlite` (bundled) for providers and settings.
- `tauri.conf.json` has `tray-icon` feature and tray menu built in `lib.rs`.
- Window close hides the app; use tray or Quit to exit.

## Common gotchas

- **No `npm test` / no test suite.** Lint and typecheck are the only automated checks.
- **No CI workflows** in this repo yet.
- **No formatter** (Prettier/Biome) is configured; `npm run lint` runs oxlint only.
- `erasableSyntaxOnly: true` means no `enum`, `namespace`, or parameter properties in TS.
- `verbatimModuleSyntax: true` means type-only imports must use `import type`.
- The React frontend uses `.tsx` files; imports must include `.tsx` extensions for Vite (`allowImportingTsExtensions`).
- Tailwind CSS v4 is loaded via `@tailwindcss/vite`; styles live in `src/index.css`.
