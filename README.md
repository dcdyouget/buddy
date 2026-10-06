# Buddy

macOS / Windows 上的 AI 对话小工具：按全局快捷键（Mac 默认 ⌘J，Windows 默认 Ctrl+J）呼出一个无边框的轻量窗口，与 AI 对话，点外部或按 Esc 收起。

- 纯 Rust，界面基于 zed 的 [GPUI](https://github.com/zed-industries/zed)
- 支持 Anthropic 与 OpenAI 兼容接口的多家模型服务商、图片输入与生成、联网搜索、本地文件工具
- 应用内自动更新

## 下载

最新版本信息：`https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/channels/stable.json`
（Mac 安装包读取 `platforms["darwin-aarch64"].installer.url`，Windows 读取 `platforms["windows-x86_64"].installer.url`），
也可以从 [GitHub Releases](https://github.com/dcdyouget/buddy/releases) 下载。支持 Apple Silicon / macOS 12 及以上，以及 Windows 10 1703（Creators Update）及以上 / Windows 11 x64。

Windows 下载 `_setup.exe` 安装包：默认安装到 `%LOCALAPPDATA%\Programs\Buddy`，无需管理员权限；也提供便携 ZIP（解压到用户可写目录后运行 `buddy.exe`）。安装包与 ZIP 都已包含 Microsoft `VCRUNTIME140.dll`，不要求另装 Visual C++ 运行库；仍依赖 Windows 10 1703 起系统提供的 ICU `icuuc.dll`。在设置中可启用开机启动；配置与聊天记录存储在 `%APPDATA%\com.buddy.chat`。自动更新需要安装目录可写。

## 开发

```bash
cargo run -p buddy-app
cargo test --workspace --exclude buddy-markdown
```

Windows 构建需要 Rust MSVC 工具链、Visual Studio 2022 的「使用 C++ 的桌面开发」组件、Windows SDK、CMake 和 Ninja。可用 `powershell -ExecutionPolicy Bypass -File scripts/windows-dev.ps1` 自动加载 VS 开发环境并运行，`-Action build` 构建正式版，`-Action check` / `-Action test` 验证。原生界面不需要 Node 或 WebView2。发版在 GitHub Actions 的 Release 工作流中完成，本机不需要交叉编译或签名。

目录结构与注意事项见 [`AGENTS.md`](./AGENTS.md)，发版见 [`docs/release-workflow.md`](./docs/release-workflow.md)。

## 许可

分层许可：`crates/engine`、`crates/syntax`、`crates/update` 为 MIT；界面层（`crates/ui`、`crates/markdown`、`apps/buddy`）因依赖 zed 的 GPL 组件为 GPL-3.0-or-later。
第三方声明见 [`THIRD_PARTY_NOTICES.md`](./THIRD_PARTY_NOTICES.md)。
