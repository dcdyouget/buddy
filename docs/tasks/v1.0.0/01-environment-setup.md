# Task 01: Environment Setup

## 目标

在 macOS 开发机上安装完整的 Tauri 2 + React 开发环境，配置中国大陆镜像加速。

## 相关设计文档

无需阅读设计文档，本任务仅涉及工具链安装。

## 验收标准

- [ ] `rustc --version` 正常输出（stable 版本）
- [ ] `cargo --version` 正常输出
- [ ] `rustup show` 显示 `aarch64-apple-darwin` target 已安装
- [ ] `~/.cargo/config.toml` 已配置 tuna 镜像源
- [ ] `node --version` ≥ 22
- [ ] `npm config get registry` 返回 `https://registry.npmmirror.com`
- [ ] `cargo tauri --version` 正常输出（Tauri CLI 2.x）
- [ ] `xcode-select -p` 返回有效路径

## 开发工作

| ID | Task | Command / Steps |
|----|------|----------------|
| E01 | Install Xcode CLI | `xcode-select --install` |
| E02 | Install Rust | 清华镜像安装脚本 |
| E03 | Configure Cargo mirror | 写 `~/.cargo/config.toml` |
| E04 | Install Rust targets | `rustup target add aarch64-apple-darwin` |
| E05 | Verify Rust | `rustc --version && cargo --version && rustup show` |
| E06 | Install Node.js 22 | fnm 安装，配置国内镜像 |
| E07 | Configure npm mirror | `npm config set registry https://registry.npmmirror.com` |
| E08 | Install Tauri CLI | `cargo install tauri-cli --version "^2"` |
| E09 | Verify all | 检查所有工具版本 |

**Cargo mirror config (`~/.cargo/config.toml`):**

```toml
[source.crates-io]
replace-with = 'tuna'

[source.tuna]
registry = 'https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git'

[net]
git-fetch-with-cli = true
```

## 测试工作

本任务为环境安装，无自动化测试。验收标准中的版本检查即为验证方式。
