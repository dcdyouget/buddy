# Task 06: Window & Hotkey

## 目标

实现全局快捷键、系统托盘、窗口生命周期管理。

## 相关设计文档

- `docs/design/rust-architecture.md` — `lib.rs` 和 `hotkey.rs` 职责

## 验收标准

- [ ] 默认快捷键 `CmdOrCtrl+Shift+Space` 可唤出/隐藏窗口
- [ ] 运行时可通过 `save_config` 更换快捷键
- [ ] 系统托盘图标显示，菜单：设置 / 开机自启（复选）/ 退出
- [ ] 窗口失焦时自动隐藏（不关闭进程）
- [ ] 窗口位置在鼠标光标附近弹出
- [ ] 窗口超出屏幕边界时自动修正
- [ ] 窗口位置和尺寸持久化记忆
- [ ] 开机自启插件正常工作

## 开发工作

| ID | Task | File | Details |
|----|------|------|---------|
| D21 | Register default hotkey | `src-tauri/src/hotkey.rs` | On startup, register `CmdOrCtrl+Shift+Space` |
| D22 | Hotkey re-register | `src-tauri/src/hotkey.rs` | On config save, unregister old, register new |
| D23 | System tray | `src-tauri/src/lib.rs` | Tray icon + menu items |
| D24 | Auto-start plugin | `src-tauri/src/lib.rs` | Integrate `tauri-plugin-autostart` |
| D25 | Window positioning | `src-tauri/src/lib.rs` | On show: get cursor position, place window nearby, check screen bounds |
| D26 | Blur hide handler | `src-tauri/src/lib.rs` | `WindowEvent::Focused(false)` → `window.hide()` |
| D27 | Window state persist | `src-tauri/src/lib.rs` | Save/restore position and size |

## 测试工作

本任务通过 E2E 测试覆盖（Task 12 hotkey record, Task 14 window behavior）。
