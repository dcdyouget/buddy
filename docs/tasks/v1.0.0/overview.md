# v1.0.0 Task Overview

> Version: v1.0.0 — 2026-06-27
> Scope: Full MVP with all 7 pages, 4 providers, streaming, settings, storage
>
> **Status:** 本目录是最初的任务拆分与验收基线，不代表当前实现清单。当前实现事实以 `docs/design/` 为准；实现已继续扩展 Provider 适配、thinking blocks、tool calling、审批/提问和 MCP 配置模型。

## Task List

| # | Task | IDs | Dependencies |
|---|------|-----|-------------|
| 01 | Environment Setup | E01-E09 | — |
| 02 | Project Scaffold | D01-D06 | 01 |
| 03 | Rust Models & Storage | D07-D09 | 02 |
| 04 | Rust API & SSE | D10-D13 | 02 |
| 05 | Rust IPC Commands | D14-D20 | 03, 04 |
| 06 | Window & Hotkey | D21-D27 | 05 |
| 07 | Frontend Infrastructure | D28-D31 | 02 |
| 08 | Shared Components | D32-D36 | 07 |
| 09 | Empty & NoApiKey Pages | D37-D38 | 08 |
| 10 | Conversation & Streaming Pages | D39-D40 | 08, 09 |
| 11 | Chat Components | D41-D45 | 10 |
| 12 | Settings & Add Provider Pages | D46-D50 | 08 |
| 13 | App Shell & Theme | D51-D53 | 09, 10, 12 |
| 14 | Animations | D54-D58 | 13 |
| 15 | Build & CI | B01-B07 | 14 |
| 16 | Testing | T01-T18 | 01-15 |

## Dependency Graph

```
01 Environment
    │
    ▼
02 Scaffold ─────────────────────────────┐
    │                                     │
    ├── 03 Rust Models & Storage          │
    ├── 04 Rust API & SSE                 │
    │    │                                │
    │    ▼                                │
    ├── 05 Rust IPC Commands              │
    │    │                                │
    │    ▼                                │
    ├── 06 Window & Hotkey                │
    │                                     │
    └── 07 Frontend Infra                 │
         │                                │
         ▼                                │
    08 Shared Components ◄────────────────┘
         │
         ├── 09 Empty & NoApiKey ──────┐
         │                             │
         ├── 12 Settings & AddProvider │
         │                             │
         └─────────────────────────────┤
              │                         │
              ▼                         ▼
         10 Conversation & Streaming   13 App Shell & Theme
              │                         ▲
              ▼                         │
         11 Chat Components ────────────┘
              │
              ▼
         14 Animations
              │
              ▼
         15 Build & CI
              │
              ▼
         16 Testing (parallel with development)
```

## Task Count

| Category | Count |
|----------|-------|
| Environment | 9 |
| Development | 58 |
| Build | 7 |
| Test | 18 |
| **Total** | **92** |

## Document Format

Each task document follows this structure:

```
# Task XX: Name

## 目标
What this task accomplishes

## 相关设计文档
Links to docs/design/ files the agent must read

## 验收标准
Checklist of conditions that must be met

## 开发工作
Specific task IDs and implementation details

## 测试工作
Specific test IDs and testing approach
```

## Context Map

| Task | Must Read |
|------|-----------|
| 01 | — (no design docs needed) |
| 02 | overview.md |
| 03 | rust-architecture.md, rust-data-models.md, storage-design.md |
| 04 | sse-and-api.md, ipc-contract.md |
| 05 | ipc-contract.md, rust-data-models.md |
| 06 | rust-architecture.md |
| 07 | rust-data-models.md (for TS types mirroring) |
| 08 | design-tokens.md, component-mapping.md |
| 09 | pages-and-states.md |
| 10 | pages-and-states.md, sse-and-api.md |
| 11 | pages-and-states.md, component-mapping.md |
| 12 | pages-and-states.md, component-mapping.md |
| 13 | pages-and-states.md, design-tokens.md |
| 14 | pages-and-states.md (streaming cursor spec) |
| 15 | rust-architecture.md |
| 16 | All design docs for context |
