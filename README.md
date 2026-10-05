# Buddy

macOS 上的 AI 对话小工具：按全局快捷键（默认 ⌘J）呼出一个无边框的轻量窗口，与 AI 对话，点外部或按 Esc 收起。

- 纯 Rust，界面基于 zed 的 [GPUI](https://github.com/zed-industries/zed)
- 支持 Anthropic 与 OpenAI 兼容接口的多家模型服务商、图片输入与生成、联网搜索、本地文件工具
- 应用内自动更新

## 下载

最新版本信息：`https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/channels/stable.json`
（其中 `platforms["darwin-aarch64"].installer.url` 为最新安装包）。仅支持 Apple Silicon，macOS 12 及以上。

## 开发

```bash
cargo run -p buddy-app
cargo test --workspace --exclude buddy-markdown
```

目录结构与注意事项见 [`AGENTS.md`](./AGENTS.md)，发版见 [`docs/release-workflow.md`](./docs/release-workflow.md)。

## 许可

分层许可：`crates/engine`、`crates/syntax`、`crates/update` 为 MIT；界面层（`crates/ui`、`crates/markdown`、`apps/buddy`）因依赖 zed 的 GPL 组件为 GPL-3.0-or-later。
第三方声明见 [`THIRD_PARTY_NOTICES.md`](./THIRD_PARTY_NOTICES.md)。
