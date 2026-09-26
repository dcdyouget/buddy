# Task 15: Build & CI

## 目标

实现多平台编译和 CI/CD 流水线。

## 相关设计文档

- `docs/design/rust-architecture.md` — tauri.conf.json 配置
- `docs/design/overview.md` — 项目结构

## 验收标准

- [ ] macOS aarch64 DMG 可正常生成
- [ ] macOS x86_64 DMG 可正常生成（或 CI）
- [ ] Windows MSI 可通过 GitHub Actions 生成
- [ ] CI workflow 文件存在且可运行
- [ ] 应用图标在所有平台显示正常

## 开发工作

| ID | Task | Details |
|----|------|---------|
| B01 | macOS dev build | `npm run tauri build` → verify `.dmg` in `src-tauri/target/release/bundle/` |
| B02 | macOS x86_64 build | Cross-compile or CI job |
| B03 | Windows build | GitHub Actions `windows-latest` runner |
| B04 | CI workflow | `.github/workflows/build.yml` |
| B05 | App icon | Generate `.icns` + `.ico` from brand logo |

**CI workflow 结构：**

```yaml
name: Build
on: [push, workflow_dispatch]
jobs:
  build-macos-aarch64:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
      - run: npm ci
      - run: npm run tauri build -- --target aarch64-apple-darwin
      - uses: actions/upload-artifact@v4

  build-macos-x86_64:
    runs-on: macos-latest
    # Similar, with x86_64 target

  build-windows:
    runs-on: windows-latest
    # Similar, with x86_64-pc-windows-msvc target
```

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T16 | Manual: Windows smoke test | Install MSI on Win10/11, verify frosted glass, font, hotkey |

无自动测试。Build 任务通过 CI green 验证。
