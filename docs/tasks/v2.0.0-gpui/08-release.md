# Phase 08: 更新与发布

## 目标

重建自动更新链路与发布流程，替换 Tauri 的 `tauri-plugin-updater`。

## 相关文档

- `docs/release-workflow.md` — 现有 macOS ARM64 / Windows 发布流程、环境检查、OSS 更新器流程（**主要参考**）
- `docs/tasks/v2.0.0-gpui/research-log.md` §3.2 — Comet 自更新实现（806 行）
- `docs/tasks/v2.0.0-gpui/06-settings-ui.md` D21 — 更新 UI
- `docs/tasks/v2.0.0-gpui/01-skeleton.md` D11-D14 — 许可证声明（GPL 需提供源码获取途径）

## 验收标准

- [ ] 旧版本（Tauri 版）用户升级到 GPUI 版后能正常收到后续更新
- [ ] 检查更新 / 下载 / 安装 / 重启全流程可用
- [ ] 下载有完整性校验（sha256）
- [ ] 更新失败可回退，不产生损坏的安装
- [ ] 发布产物含 GPL 要求的源码获取说明
- [ ] 现有 OSS 上传链路复用或平滑迁移

## 现有状态

来自 `src-tauri/tauri.conf.json`：

| 项 | 当前值 |
|----|--------|
| 更新器 | `tauri-plugin-updater = "2.10.1"` |
| 公钥 | Ed25519 minisign pubkey 已配置 |
| 端点 | `https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/stable/latest.json` |
| Windows 安装模式 | `installMode: "passive"` |
| 产物 | `createUpdaterArtifacts: true` |
| macOS 最低版本 | `12.0` |
| 签名 | `signingIdentity: "-"`（临时签名） |

## 开发工作

### 8.1 更新器实现

| ID | Task | Details |
|----|------|---------|
| D01 | 选型 | 二选一：<br>A. 自行实现（参考 Comet `crates/update`，806 行）<br>B. 寻找无 Tauri 依赖的 Rust 更新器库 |
| D02 | 清单格式 | 决定是沿用 `latest.json`，还是迁移到带 per-artifact sha256 的 `manifest.json`（Comet 做法） |
| D03 | 端点迁移 | 复用现有 OSS bucket，或迁移到新 bucket；需保证旧版仍能读到旧端点 |
| D04 | 版本比较 | 语义化版本比较，含渠道（stable / beta）区分 |
| D05 | 下载 | 流式下载 + 进度回调（接 Phase 06 D21） |
| D06 | 完整性校验 | sha256 校验，失败即丢弃 |
| D07 | 签名校验 | 若沿用 minisign / Ed25519，需在 Rust 侧实现验签（Tauri 插件原先负责） |
| D08 | macOS 安装 | **app bundle 替换**：下载新 bundle → 原子替换 → 重启。参考 Comet 的 `MacApp` 安装路径 |
| D09 | Windows 安装 | 需对应 `installMode: passive` 的等价体验（静默安装 + 重启） |
| D10 | 失败回退 | 替换失败时恢复原 bundle；重启后能自愈 |
| D11 | 权限处理 | macOS `/Applications` 下可能需要提权；处理失败提示 |
| D12 | 静默检查 | 启动后延迟检查 + 定时检查（参考 Comet：初始延迟 20s，周期 6h，失败重试 30min） |
| D13 | 空闲时机 | 流式进行中不自动安装，等空闲（参考 Comet `IDLE_RECHECK`） |
| D14 | 渠道与开关 | 复现现有 `UpdateSetting` 的检查/自动更新开关 |

### 8.2 打包

| ID | Task | Details |
|----|------|---------|
| D15 | macOS 打包脚本 | 生成 `.app` + `.dmg`；替换现有 Tauri bundler |
| D16 | DMG 外观 | 复用现有 DMG 背景与布局（`docs/release-workflow.md`） |
| D17 | Windows 打包 | 生成安装包；保留 `package-windows` 等价脚本（Comet **缺失**此项，需自建） |
| D18 | 最低系统版本 | macOS 12.0 保持不变；确认 GPUI 是否抬高该门槛 |
| D19 | 架构 | macOS ARM64（现有）；是否保留 Intel 需决策 |
| D20 | 体积记录 | 记录与现有 7.2MB dmg 的对比 |

### 8.3 签名与公证

| ID | Task | Details |
|----|------|---------|
| D21 | macOS 签名 | 替换 `signingIdentity: "-"` 为正式证书（或确认继续临时签名） |
| D22 | 公证 | notarization 流程；GPUI 无 Tauri 的现成辅助 |
| D23 | 权限声明 | 确认是否需要 entitlement（全局热键、辅助功能、网络） |
| D24 | Windows 签名 | 现状确认与延续 |

### 8.4 CI

| ID | Task | Details |
|----|------|---------|
| D25 | 构建流水线 | 替换现有 Tauri CI；macOS runner 构建 |
| D26 | 版本号管理 | 复用现有 `scripts/set-version.mjs` 逻辑（`version:set`） |
| D27 | 产物上传 | 上传到 OSS 并更新清单（复用 `docs/release-workflow.md` 的链路） |
| D28 | 版本一致性守卫 | 参考 Comet CI：tag 与 Cargo.toml 版本不一致则失败 |
| D29 | 依赖缓存 | Rust 编译缓存，避免全量重建 |

### 8.5 GPL 合规产出

| ID | Task | Details |
|----|------|---------|
| D30 | 源码获取说明 | 应用内「关于」提供源码链接（GPL 要求） |
| D31 | 随包许可 | 打包时附带 `LICENSE` 与 `THIRD_PARTY_NOTICES.md` |
| D32 | vendor patch 归档 | 随发布附上对 zed `markdown.rs` 的 patch（GPL 对修改部分的要求） |
| D33 | 标签归档 | 每个发布版本对应可访问的源码 tag |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T01 | 更新全流程 | 手工构造旧版本 → 检查 → 下载 → 安装 → 重启 → 版本正确 |
| T02 | 断网/中途失败 | 下载中断、校验失败、替换失败的各自表现 |
| T03 | 旧版升级测试 | 从当前 Tauri 版本升级到 GPUI 版本 |
| T04 | 权限失败测试 | 无写权限目录下的安装表现 |
| T05 | 清单解析测试 | 格式错误 / 版本回退 / 多渠道 |
| T06 | 流式中更新测试 | 流式进行时自动更新应等待 |
| T07 | 打包产物冒烟 | 全新机器上安装并运行 |

## 备注

D08（macOS bundle 替换）与 D11（权限）是风险最高的两项：Tauri 的更新器替你做掉了这些，GPUI 路线需要自己实现。
建议在 Phase 07 完成后尽早启动本 Phase，因为「能否可靠自更新」会影响是否敢给现有用户推送升级。
