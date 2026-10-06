# Buddy

macOS / Windows 上的 AI 对话小工具：全局快捷键（Mac 默认 ⌘J，Windows 默认 Ctrl+J）呼出无边框浮动窗口，对话，点外部或 Esc 收起。
纯 Rust，界面基于 zed 的 GPUI。

## 目录

| 路径 | 内容 | 许可 |
|---|---|---|
| `apps/buddy` | 可执行入口；`bundle/macos` 为 Info.plist 模板与图标 | GPL-3.0 |
| `crates/ui` | 界面：窗口外壳（`shell`）、对话、设置、主题令牌（`theme_system/tokens.rs` 为外观唯一来源） | GPL-3.0 |
| `crates/engine` | 模型服务商、流式、存储、工具调用；不依赖任何 UI / GPL crate | MIT |
| `crates/markdown` | vendored zed markdown（改动见 `VENDOR.md` 与 `patches/`） | GPL-3.0 |
| `crates/syntax` | 代码高亮：内置 Python / Shell / SQL 语法，其余语言走 `generic` 通用高亮 | MIT |
| `crates/update` | 自更新：清单、下载、sha256 + minisign 校验、替换安装 | MIT |
| `vendor/global-hotkey` | 经 `[patch.crates-io]` 使用的 global-hotkey 修改版 | MIT / Apache-2.0 |
| `scripts/release` | 一键发版，见 `docs/release-workflow.md` | — |

## 常用命令

```bash
cargo run -p buddy-app                              # 开发运行
cargo test --workspace --exclude buddy-markdown     # 全部测试
scripts/gate.sh >/tmp/gate.log 2>&1; echo $?        # 提交前门禁（不要接管道，会吞掉退出码）
target/debug/buddy --selfcheck-window               # 原生窗口行为自检（需桌面已解锁）
npm run release -- <版本号> --notes "更新说明"        # 发版（OSS + 推送 GitHub + GitHub Release）
```

## 需要知道的事

- zed 依赖（gpui / theme / ui 等）全部锁定同一 git rev（根 `Cargo.toml`），升级时必须一起改；拉取 zed 的 git 依赖通常需要代理。
- 修改 `crates/markdown/src` 后必须按 `VENDOR.md` 重新生成 patch，否则门禁失败（GPL 要求记录修改）。
- 数据目录 Mac 为 `~/Library/Application Support/com.buddy.chat`，Windows 为 `%APPDATA%\com.buddy.chat`，与 v1 共用（标识 `com.buddy.chat` 不要改）；API Key 明文存在 `config.json`。
- 主窗口在显示前用 AppKit 改样式（无标题栏、无阴影、圆角）；改完必须让 GPUI 视图重新成为第一响应者，否则无法输入文字。自检 T12-01 会检查。
- v1（Tauri + React）已从仓库移除，可从标签 `v1-final` 找回。
