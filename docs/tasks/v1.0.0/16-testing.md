# Task 16: Testing

## 目标

完成所有单元测试、组件测试、E2E 测试和手动测试。

## 相关设计文档

所有 `docs/design/` 文件。

## 验收标准

- [ ] Rust 单元测试全部通过（T01-T04）
- [ ] React 组件测试全部通过（T05-T07）
- [ ] E2E 流程全部通过（T08-T15）
- [ ] 手动 smoke test 全部通过（T16-T18）

## 测试工作

### Rust 单元测试

| ID | Task | Details |
|----|------|---------|
| T01 | Storage tests | Chunk rotation at 100, manifest consistency, paginated read across chunks |
| T02 | SSE parser tests | Single delta, multi-line chunk, `[DONE]`, empty data, malformed JSON |
| T03 | Config validation | Corrupted file recovery, missing fields default, round-trip read/write |
| T04 | API error mapping | 401 → Unauthorized, 429 → QuotaExceeded, 500 → ServerError, timeout → NetworkError |

### React 测试 (vitest + testing-library)

| ID | Task | Details |
|----|------|---------|
| T05 | Component render tests | All 20+ components render without crash |
| T06 | Store logic tests | Config load/save, chat append/finalize, UI page transitions |
| T07 | Streaming simulation | Mock listen events, verify token append |

### E2E 测试 (manual or Playwright)

| ID | Task | Flow |
|----|------|------|
| T08 | empty → noapikey | Fresh install → type → Enter → verify noapikey shown |
| T09 | Add provider | Noapikey → settings → add provider → fetch models → save → conversation |
| T10 | Conversation | Input → send → streaming → stop → continue |
| T11 | Model switch | Dropdown open → select → pill updates |
| T12 | Hotkey record | Settings → record → verify |
| T13 | Theme toggle | Settings → dark → CSS class + persistence |
| T14 | Window behavior | Esc close, blur close, streaming continues |
| T15 | Message persist | Send → close → reopen → messages there |

### 手动测试

| ID | Task | Details |
|----|------|---------|
| T16 | Windows smoke | Install MSI, frosted glass fallback, font, hotkey |
| T17 | Memory check | Idle < 80MB, streaming < 150MB |
| T18 | Cold start | Launch to visible < 500ms |
