# Buddy — Project Instructions for Codex

## Project Overview

Buddy is a cross-platform (macOS / Windows) AI chat tool. Press a global hotkey → a lightweight frameless window pops up → chat with AI → click away to dismiss. Think Bob (the macOS translate app) but for AI chat.

- **Tech Stack**: Tauri 2 + React 18 + TypeScript + Tailwind CSS v4 + Framer Motion + Zustand
- **Package Target**: `< 10 MB`
- **Design Philosophy**: Zero-chrome frameless window, frosted glass, calm indigo-violet brand

## Document Index

| Document | Purpose | When to Read |
|----------|---------|-------------|
| `docs/tasks/v1.0.0/overview.md` | Task overview & dependency graph | **Always** — start here |
| `docs/tasks/v1.0.0/*.md` | Individual task specs (16 tasks) | When assigned a task |
| `docs/design/overview.md` | Architecture overview & key decisions | When needing context |
| `docs/design/design-tokens.md` | Colors, fonts, spacing, shadows | When writing UI code |
| `docs/design/pages-and-states.md` | 7 page specs + state machine | When building pages |
| `docs/design/component-mapping.md` | Design → React component map | When building components |
| `docs/design/rust-architecture.md` | Rust module layout & responsibilities | When writing Rust |
| `docs/design/rust-data-models.md` | Rust struct definitions | When writing Rust |
| `docs/design/ipc-contract.md` | invoke/listen contract | When connecting frontend ↔ backend |
| `docs/design/sse-and-api.md` | Streaming, fetch models, speed test | When writing API code |
| `docs/design/storage-design.md` | JSON file layout, chunk mechanism | When writing storage code |
| `docs/CONVENTIONS.md` | Coding rules for ALL agents | **Always** — read once, follow always |
| `docs/design/prototypes/` | HTML design prototypes (7 pages) | When checking visual reference |
| `docs/design/colors_and_type.css` | Brand CSS single source of truth | When verifying token values |

## Hard Constraints

1. **No traffic-light buttons** — zero chrome, no window controls, completely frameless
2. **Single brand color** — `#5B5FE9` (indigo-violet). State colors: `success/warning/error/info` only
3. **Radius scale** — only `4/8/12/16/9999` px
4. **No emoji icons** — use `lucide-react` exclusively
5. **Design tokens only** — never hardcode colors/shadows/spacing; always use CSS variables
6. **Window never resizes on page switch** — keep user-set dimensions
7. **Esc/click-outside closes window, does NOT stop streaming**
8. **Single conversation stream** — no multi-session UI (but storage layer should accept optional session IDs)
9. **API Key stored in plaintext JSON** — not system keychain (v0.1)
10. **All text in Chinese** (UI labels, hints, settings) — code comments may be English

## Quick Start (after scaffold)

```bash
npm install
npm run tauri dev    # Dev mode with hot-reload
npm run tauri build  # Production build
```

## File Organization

```
buddy/
├── src/                    # React frontend
│   ├── components/         # UI components (flat: one folder per component)
│   ├── stores/             # Zustand stores
│   ├── hooks/              # Custom hooks
│   ├── types/              # TypeScript types (mirror Rust models)
│   ├── App.tsx
│   └── main.tsx
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── commands.rs
│   │   ├── api.rs
│   │   ├── models.rs
│   │   ├── storage.rs
│   │   └── hotkey.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── docs/                   # Design docs + task specs + prototypes
│   ├── design/             # Architecture, tokens, pages, IPC, prototypes, CSS
│   │   └── prototypes/     # HTML design prototypes (7 pages)
│   ├── tasks/v1.0.0/       # Current version task specs (16 task files)
│   └── CONVENTIONS.md      # Coding rules for all agents
└── AGENTS.md               # ← this file
```
