# Buddy

Cross-platform (macOS / Windows) AI chat tool. Press a global hotkey → a light, frameless window pops up → chat with AI → click away to dismiss. Think Bob (the macOS translate app), but for AI chat.

> ⏵ **Status**: v1.0.0 — all 16 task specs complete. Single conversation stream, multi-provider (Anthropic / OpenAI-compatible), zero chrome.

---

## Tech stack

| Layer | Choice |
|---|---|
| Shell | Tauri 2 |
| Frontend | React 18 + TypeScript + Vite |
| Styling | Tailwind CSS v4 + framer-motion + design tokens |
| State | Zustand (3 stores: `configStore`, `uiStore`, `chatStore`) |
| Markdown | react-markdown + remark-gfm + prism-react-renderer |
| Icons | lucide-react (no emoji icons, per `CLAUDE.md` rule #4) |
| Backend | Rust (commands / providers / streaming / storage / hotkey / tray / window) |

Target package size: **< 10 MB**.

---

## Quick start

```bash
# Install
npm install

# Dev mode (HMR on frontend + auto-rebuild on Rust changes)
npm run tauri dev

# Production build
npm run tauri:build

# Run unit tests
npm run test:run
```

---

## Project layout

```
buddy/
├── src/                    # React frontend
│   ├── api/                # Tauri invoke wrappers (chat / config / storage / provider)
│   ├── components/         # UI: chat/, settings/, shared/
│   ├── hooks/              # useStreaming, useDragHandle
│   ├── pages/              # EmptyPage · NoApiKeyPage · ChatPage · SettingsPage
│   ├── stores/             # Zustand stores
│   ├── styles/global.css   # Design tokens (single source of truth for colors / radii)
│   ├── types/              # TypeScript types mirroring Rust models
│   ├── utils/              # thinkParser · windowResize · mock
│   ├── App.tsx             # Root component (router + theme + selected-text)
│   └── main.tsx            # React entry
└── src-tauri/              # Rust backend
    ├── src/
    │   ├── lib.rs          # App setup & plugin registration
    │   ├── commands.rs     # Tauri command handlers (IPC)
    │   ├── streaming.rs    # Unified event protocol (StreamEvent / StopReason)
    │   ├── storage.rs      # JSON-file persistence + chunk rotation
    │   ├── hotkey.rs       # Global shortcut lifecycle
    │   ├── tray.rs         # System tray (autostart, quit)
    │   ├── models/         # AppConfig · Message · CompatConfig · model_context
    │   ├── providers/      # Anthropic + OpenAI-compatible providers
    │   ├── platform/       # macOS vibrancy / Windows platform shims
    │   └── window/         # Event hooks + saved-position memory
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── capabilities/       # Tauri 2 permission grants
    └── icons/              # App & tray icons (macOS / Windows only)
```

---

## Hard design constraints (do not violate)

1. **No traffic-light buttons.** The window is **frameless**. No chrome, no window controls.
2. **Single brand color** `#5B5FE9` (indigo-violet). State colors only for `success / warning / error / info`.
3. **Radius scale** is exactly `4 / 8 / 12 / 16 / 9999` px.
4. **No emoji icons.** Use `lucide-react` exclusively.
5. **Design tokens only.** Never hardcode colors / shadows / spacing — always use CSS variables.
6. **Window never resizes on page switch.** User-set dimensions are preserved.
7. **Esc / click-outside closes the window — does NOT stop streaming.**
8. **Single conversation stream.** No multi-session UI (storage layer accepts optional session IDs for future use).
9. **API keys stored in plaintext JSON** under app data dir (v1 — not keychain).
10. **All UI text is Chinese.** Code comments may be English.

See `src/styles/global.css` for the canonical token values, and `src/components/` for usage patterns.

---

## Where to read more

| If you want to … | Read this |
|---|---|
| Understand the module layout | `src-tauri/src/lib.rs` setup block + `commands.rs` |
| Wire a new IPC handler | `src-tauri/src/commands.rs` + `src/api/*.ts` (invoke wrappers) |
| Add a streaming event | `src-tauri/src/streaming.rs` (`StreamEvent` enum) + `useStreaming.ts` |
| Add a model provider | `src-tauri/src/providers/` (impl `LlmProvider` trait) |
| Tweak styling | `src/styles/global.css` (tokens) + individual components |
| Tweak window behavior | `src-tauri/src/window/events.rs` |

---

## Conventions

- Match the comment density, naming, and idiom of surrounding code.
- UI labels, hints, settings copy → **Chinese**.
- Code comments → English is fine (existing code mixes Chinese module headers with English inline notes).
- Don't change `tsconfig.json` strictly settings; the codebase relies on `noUnusedLocals` + `noUnusedParameters`.
- Before adding a dependency, check whether the package already includes what you need (e.g. lucide-react ships its own `LucideIcon` type — no manual `.d.ts` needed).
