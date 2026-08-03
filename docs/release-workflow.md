# Buddy 发布与自动更新执行手册

> 更新时间：2026-08-03
> 适用平台：macOS Apple Silicon（`darwin-aarch64`）和 Windows x86_64（`windows-x86_64`）
> 制品存储：阿里云 OSS `buddy-release`
> 本文用于指导开发者或后续 Agent 检测环境、补齐发布能力并执行版本发布。

## 1. 发布目标

本项目只使用两个流程名称：

| 流程 | 作用 | 是否触发客户端升级 |
|---|---|---|
| Build 流程 | 检测环境，构建、签名并上传当前平台制品和 fragment | 否 |
| 发布流程 | 校验所有平台制品，生成并上传 `buddy/stable/latest.json` | 是 |

对 Agent 使用以下触发语句即可：

```text
执行 Buddy <版本号> 的 macOS ARM64 build 流程。
执行 Buddy <版本号> 的 Windows x86_64 build 流程。
执行 Buddy <版本号> 的发布流程，更新说明：<本次更新内容>。
```

Build 流程不得修改 `stable/latest.json`。发布流程以成功创建或更新 `stable/latest.json` 为完成标志，客户端随后即可检查并执行升级。

发布流程必须接收非空的中文更新说明，并写入 `latest.json` 的 `notes` 字段。短说明可以直接传入，长说明使用 UTF-8 文本文件：

```bash
npm run release:publish -- 1.1.0 --notes "新增自动更新，修复窗口唤起问题"
npm run release:publish -- 1.1.0 --notes-file ./release-notes/1.1.0.txt
```

应用发现更新后，应在用户确认安装前展示版本号和 `notes`；不得把发布说明仅写入 OSS 而不在客户端显示。

- macOS 只发布 Apple Silicon ARM64，不构建或支持 Intel Mac。
- Windows 只发布 x86_64，优先使用 NSIS 安装包。
- 新用户从 OSS 公共 HTTPS 地址下载安装包。
- 已安装用户通过 Tauri Updater 读取静态 `latest.json`，下载对应平台更新包并校验签名。
- ECS 当前不参与发布或更新；OSS 负责存储、公开下载和静态更新清单。
- 所有版本制品使用不可变版本目录；只有 `buddy/stable/latest.json` 会被覆盖。

固定地址：

```text
Bucket:          buddy-release
Region:          cn-beijing
OSS Endpoint:    https://oss-cn-beijing.aliyuncs.com
Public Base URL: https://buddy-release.oss-cn-beijing.aliyuncs.com
Updater URL:     https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/stable/latest.json
```

### 1.1 下载链接规范

新用户手动下载安装包时使用版本化链接：

| 平台 | 安装包 | 公共下载 URL 模板 |
|---|---|---|
| macOS Apple Silicon | DMG | `https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/releases/<version>/macos/aarch64/Buddy_<version>_aarch64.dmg` |
| Windows x86_64 | NSIS EXE | `https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/releases/<version>/windows/x86_64/Buddy_<version>_x86_64-setup.exe` |

例如 `1.1.0`：

```text
https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/releases/1.1.0/macos/aarch64/Buddy_1.1.0_aarch64.dmg
https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/releases/1.1.0/windows/x86_64/Buddy_1.1.0_x86_64-setup.exe
```

当前 Windows 发布方案选择 NSIS `.exe`，不生成 `.msi`。如果后续明确需要 MSI，必须同步修改 Windows 构建脚本、制品命名、上传路径和验证清单。

自动更新使用的链接与手动安装不同：

- macOS：`latest.json` 指向 `.app.tar.gz`，并携带对应 `.sig` 内容。
- Windows：`latest.json` 指向 NSIS `.exe`，并携带对应 `.exe.sig` 内容。
- `buddy/stable/latest.json` 是 Updater 元数据入口，不是 DMG 或 EXE 下载地址。
- 当前没有不含版本号的“最新版安装包固定链接”；网站下载按钮需要读取当前版本配置或在发布时更新为上述版本化 URL。

## 2. 当前实现状态

| 能力 | 状态 | 位置或说明 |
|---|---|---|
| Tauri Updater Rust/JS 依赖 | 已完成 | `package.json`、`src-tauri/Cargo.toml` |
| Updater 插件初始化 | 已完成 | `src-tauri/src/lib.rs` |
| Updater 权限 | 已完成 | `src-tauri/capabilities/default.json` |
| Updater 公钥和 HTTPS endpoint | 已完成 | `src-tauri/tauri.conf.json` |
| 生成 Updater 制品 | 已完成 | `bundle.createUpdaterArtifacts = true` |
| macOS ARM64 发布脚本 | 已完成 | `scripts/release-macos.sh` |
| 应用内手动检查、下载、安装更新 | 已完成 | `src/components/settings/UpdateSetting.tsx` |
| Windows x86_64 发布脚本 | **未实现** | 计划新增 `scripts/release-windows.ps1` |
| 合并并发布 `latest.json` | **未实现** | 计划新增 `scripts/publish-release.mjs` |
| macOS Developer ID 签名和公证 | 未配置 | 当前为 Ad-hoc 签名 `signingIdentity = "-"` |
| Windows 代码签名 | 未配置 | 安装包可能触发 SmartScreen |

在 Windows 发布脚本和 `latest.json` 发布脚本补齐，并完成旧版本真机升级验证前，不得宣称自动更新链路已经完成。

## 3. Agent 执行约束

1. 先检测并汇报环境，再修改、构建或上传。
2. 不得读取、打印或提交 AccessKey Secret、Updater 私钥内容或私钥密码。
3. 不得把 `~/.ossutilconfig`、`buddy.key`、证书或密码文件复制进仓库。
4. 不得清理、覆盖或回滚用户已有的未提交改动。
5. 未确认两个目标平台制品完整前，不得覆盖 `buddy/stable/latest.json`。
6. 不得构建或向清单加入 `darwin-x86_64`。
7. 不得移动已经推送的版本标签；标签指向错误时停止并向用户报告。
8. OSS 写入测试属于外部状态变更，执行前应取得用户授权并使用独立测试 Key，完成后清理。
9. 所有更新制品必须由同一份 Updater 私钥签名；该私钥与 Apple/Windows 系统代码签名证书不是同一种凭据。

## 4. 一次性环境检测

### 4.1 仓库与通用工具

在两台构建机器分别执行：

```bash
git status --short
git branch --show-current
git remote -v
node --version
npm --version
rustc --version
cargo --version
npm run tauri -- info
```

Agent 必须确认：

- 仓库 remote 指向预期的 Buddy 仓库。
- 两台机器最终构建的是同一个 commit 和同一个 tag。
- Node、npm、Rust、Cargo 和 Tauri CLI 可用。
- 正式发布时 `git status --porcelain` 必须为空。
- 不得仅因环境检测而自动升级依赖或工具链。

检查四处版本号一致：

```text
package.json
package-lock.json
src-tauri/tauri.conf.json
src-tauri/Cargo.toml
```

### 4.2 macOS ARM64 环境

```bash
test "$(uname -s)" = "Darwin"
test "$(uname -m)" = "arm64"
xcode-select -p
command -v node npm rustc cargo ossutil curl
ossutil version
test -f "$HOME/.tauri/buddy.key"
stat -f '%Sp %N' "$HOME/.tauri/buddy.key"
```

必须满足：

- `uname -m` 为 `arm64`。
- Xcode Command Line Tools 可用。
- `ossutil` 为 2.x，当前已验证版本为 2.3.0。
- Updater 私钥默认位于 `~/.tauri/buddy.key`，建议权限为 `600`。
- 不安装 `x86_64-apple-darwin` target，不执行 Intel Mac 构建。

检测 macOS 系统签名：

```bash
security find-identity -v -p codesigning
```

如果没有 `Developer ID Application`，Agent 应报告“只能生成 Ad-hoc 签名包，公网下载可能触发 Gatekeeper”，不得伪装成已完成正式签名和公证。

### 4.3 Windows x86_64 环境

在 PowerShell 执行：

```powershell
[Environment]::Is64BitOperatingSystem
$env:PROCESSOR_ARCHITECTURE
Get-Command git, node, npm, rustc, cargo, ossutil
node --version
npm --version
rustc --version
cargo --version
ossutil version
npm run tauri -- info
Test-Path "$env:USERPROFILE\.tauri\buddy.key"
```

必须满足：

- 64 位 Windows，目标为 `windows-x86_64`。
- 已安装 Visual Studio 2022 C++ Build Tools、Windows SDK 和 WebView2 Runtime。
- Updater 私钥是 Mac 上同一把私钥的安全副本，建议路径为 `%USERPROFILE%\.tauri\buddy.key`。
- 私钥不得通过 Git、公开网盘、OSS 公共 Bucket 或聊天发送。
- 如未配置 Windows 代码签名，Agent 应明确报告 SmartScreen 风险。

Windows 预检的最终依据不是“命令存在”，而是 Tauri NSIS `--skip-upload` 构建实际成功。

### 4.4 OSS 配置和权限

Mac 默认配置文件为 `~/.ossutilconfig`。只检查文件权限、配置项是否存在和值长度，不输出值：

```bash
stat -f '%Sp %N' "$HOME/.ossutilconfig"
awk -F= 'NF >= 2 { key=$1; value=substr($0,index($0,"=")+1); printf "%s=<已设置，长度 %d>\n", key, length(value); next } { print }' "$HOME/.ossutilconfig"
awk -F= '$1 == "region" { exit($2 == "cn-beijing" ? 0 : 1) }' "$HOME/.ossutilconfig"
ossutil ls oss://buddy-release/buddy/ --endpoint https://oss-cn-beijing.aliyuncs.com
```

禁止直接打印整个配置文件，也不要在配置未脱敏检查前执行可能输出错误配置值的命令。

当前发布脚本需要至少具备：

- Bucket 级：`oss:ListObjects`，资源 `acs:oss:*:*:buddy-release`。
- Object 级：`oss:GetObject`、`oss:PutObject`，资源 `acs:oss:*:*:buddy-release/buddy/*`。
- 发布脚本不需要 `oss:DeleteObject`；如果保留删除权限，Agent 应提示权限偏大。
- Bucket 必须是公共读、非公共写。

只有用户明确授权时才进行写入测试。建议使用独立测试对象，成功后立即删除：

```bash
ossutil cp /dev/null oss://buddy-release/buddy/.checks/ossutil-write-test \
  --endpoint https://oss-cn-beijing.aliyuncs.com \
  --force \
  --cache-control no-cache
curl --fail --head \
  https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/.checks/ossutil-write-test
ossutil rm oss://buddy-release/buddy/.checks/ossutil-write-test \
  --endpoint https://oss-cn-beijing.aliyuncs.com \
  --force
```

若 RAM 已移除删除权限，不要创建无法清理的测试对象，直接通过正式版本制品上传验证写权限。

### 4.5 Updater 配置检测

Agent 必须检查以下内容：

```bash
rg -n "createUpdaterArtifacts|pubkey|endpoints|plugin-updater|updater:default|process:allow-restart" \
  package.json src-tauri
rg -n "downloadAndInstall|plugin-updater|检查更新|check\\(" src desktop src-tauri/src
```

验收条件：

- `createUpdaterArtifacts` 为 `true`。
- `pubkey` 非空且是公钥内容，不是文件路径。
- endpoint 使用上文固定 HTTPS URL。
- Rust 插件已初始化，Updater 和重启权限已声明。
- 应用侧真实调用 `check()`、`downloadAndInstall()` 和 `relaunch()`。
- UI 文案为中文；无更新和网络失败不能导致应用启动失败。
- 应用启动时不得自动检查更新，只能由用户在设置页点击“检查更新”触发。

设置页更新状态必须遵循：

```text
未检查
  └─ 点击“检查更新” → 检查中
       ├─ 无更新 → 当前已是最新版本
       ├─ 有更新 → 显示新版本号、更新说明和“立即更新”按钮
       │              └─ 点击“立即更新” → 下载/安装中 → 重启应用
       └─ 失败 → 显示错误和“重新检查”按钮
```

Updater 签名校验不可关闭。

## 5. 每次发布的标准流程

以下以 `1.1.0` 为例，实际执行时替换版本号。

### 步骤 1：完成开发并更新版本

```bash
git pull --ff-only
npm run version:set -- 1.1.0
cargo check --manifest-path src-tauri/Cargo.toml
```

补充更新说明，使用 `git status --short` 审查变更，确认四处版本号一致，然后按实际文件明确暂存并提交：

```bash
git add package.json package-lock.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
# 继续逐个 git add 本版本需要提交的实际文件
git commit -m "chore(release): v1.1.0"
git push origin <当前分支>
```

不得使用 `git add .` 无审查地提交密钥、构建目录或无关改动。

### 步骤 2：两台机器验证同一候选提交

Mac：

```bash
git pull --ff-only
npm run release:mac -- 1.1.0 --skip-upload --allow-untagged
```

当前 macOS 脚本会执行：

1. `npm ci`。
2. 前端 Vitest。
3. Rust 测试。
4. `aarch64-apple-darwin` Tauri 构建。
5. 生成 DMG、`.app.tar.gz`、`.sig` 和 `darwin-aarch64.json`。

Windows 在发布脚本补齐后执行：

```powershell
git pull --ff-only
npm run release:windows -- 1.1.0 --skip-upload --allow-untagged
```

两边必须是相同 commit。任一平台失败时修复、提交并重新验证，不得提前打正式 tag。

### 步骤 3：创建并推送版本标签

```bash
git tag -a v1.1.0 -m "Buddy v1.1.0"
git push origin v1.1.0
```

验证：

```bash
git rev-parse HEAD
git rev-list -n 1 v1.1.0
git tag --points-at HEAD
```

三个结果必须表明当前 commit 与 `v1.1.0` 一致。

### 步骤 4：Mac 正式构建和上传

```bash
git checkout v1.1.0
npm run release:mac -- 1.1.0
```

脚本会安全提示输入 Updater 私钥密码，不应把密码写入仓库或命令行历史。

预期本地制品：

```text
.release/1.1.0/macos/aarch64/
├── Buddy_1.1.0_aarch64.dmg
├── Buddy_1.1.0_aarch64.app.tar.gz
├── Buddy_1.1.0_aarch64.app.tar.gz.sig
└── darwin-aarch64.json
```

预期 OSS 路径：

```text
buddy/releases/1.1.0/macos/aarch64/*
buddy/releases/1.1.0/manifests/darwin-aarch64.json
```

### 步骤 5：Windows 正式构建和上传

在 Windows PowerShell：

```powershell
git fetch --tags
git checkout v1.1.0
npm run release:windows -- 1.1.0
```

预期本地制品：

```text
.release/1.1.0/windows/x86_64/
├── Buddy_1.1.0_x86_64-setup.exe
├── Buddy_1.1.0_x86_64-setup.exe.sig
└── windows-x86_64.json
```

预期 OSS 路径：

```text
buddy/releases/1.1.0/windows/x86_64/*
buddy/releases/1.1.0/manifests/windows-x86_64.json
```

### 步骤 6：生成并发布最终更新清单

两个平台均成功后，才允许执行：

```bash
npm run release:publish -- 1.1.0
```

最终清单只能包含两个平台：

```json
{
  "version": "1.1.0",
  "notes": "本版本更新说明",
  "pub_date": "RFC 3339 时间",
  "platforms": {
    "darwin-aarch64": {
      "url": "https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/releases/1.1.0/macos/aarch64/Buddy_1.1.0_aarch64.app.tar.gz",
      "signature": "对应 .sig 文件的完整内容"
    },
    "windows-x86_64": {
      "url": "Windows 更新安装包 URL",
      "signature": "对应 .sig 文件的完整内容"
    }
  }
}
```

发布顺序必须是：

1. 下载并校验两个平台 fragment。
2. 校验版本号、平台名、URL、签名均非空。
3. 对两个制品 URL 执行 HTTPS HEAD，要求 2xx。
4. 先上传 `buddy/releases/1.1.0/latest.json`。
5. 公开下载并解析该版本清单。
6. **最后**覆盖 `buddy/stable/latest.json`，并设置 `Cache-Control: no-cache`。
7. 再次公开下载 stable 清单，确认版本和两个平台完全正确。

覆盖 `stable/latest.json` 是正式发布开关。在此之前，已安装用户不会看到新版本。

### 步骤 7：发布后验证

必须记录并验证：

- DMG 公共 URL 返回 2xx，可下载并安装。
- Windows 安装包公共 URL 返回 2xx，可下载并安装。
- `stable/latest.json` 返回 2xx、JSON 合法、版本正确。
- 清单仅包含 `darwin-aarch64`、`windows-x86_64`。
- 从低一版本的 ARM Mac 实机检查、下载、签名校验、安装和重启成功。
- 从低一版本的 Windows x86_64 实机完成相同验证。
- 新安装用户获得的是目标版本。
- OSS 上旧版本目录仍保留，版本制品没有被覆盖。

首个包含 Updater 的版本必须先通过正常安装分发；更早且没有更新检查逻辑的客户端无法自动升级。

## 6. 剩余实现与验收

### 6.1 应用内更新流程（已实现，待真机验收）

当前实现和验收要求：

- 在设置页新增独立的更新区域，显示当前版本和“检查更新”按钮。
- 应用启动和打开设置页时都不得自动检查更新。
- 只有用户点击“检查更新”后，才调用 `@tauri-apps/plugin-updater` 的 `check()`。
- 检查期间禁用按钮并显示“正在检查…”，防止重复请求。
- 无更新时显示“当前已是最新版本”。
- 发现更新后显示新版本号、`latest.json.notes` 更新说明和“立即更新”按钮。
- 用户点击“立即更新”后调用 `downloadAndInstall()`，显示下载/安装状态并防止重复操作。
- 安装成功后调用 `@tauri-apps/plugin-process` 的 `relaunch()`。
- 网络失败、清单错误、下载失败和签名失败时显示中文错误，并允许重新检查。
- 更新失败不得影响聊天、设置保存或应用下次启动。
- 添加状态和交互单元测试；至少完成一次旧版本到新版本的真实端到端测试。

参考：[Tauri Updater 官方文档](https://v2.tauri.app/plugin/updater/)。

### 6.2 Windows 发布脚本

新增 `scripts/release-windows.ps1` 和 `package.json` 的 `release:windows` 命令，并与 macOS 脚本保持以下行为一致：

- 校验 SemVer、四处项目版本、干净工作区和 `v<版本>` 标签。
- 支持 `--skip-upload`、`--allow-untagged`。
- 检测 Node、npm、Rust、Cargo、Tauri、ossutil 和 VS Build Tools。
- 从 `%USERPROFILE%\.tauri\buddy.key` 使用同一份 Updater 私钥。
- 执行 `npm ci`、前端测试和 Rust 测试。
- 只构建 Windows x86_64 NSIS，不构建 macOS x86 或其他平台。
- 收集 `.exe`、`.exe.sig`，生成 `windows-x86_64.json`。
- 上传版本制品和 fragment，版本制品使用 immutable cache，fragment 使用 no-cache。
- 上传后通过公共 HTTPS 验证下载。

### 6.3 最终清单发布脚本

新增 `scripts/publish-release.mjs` 和 `package.json` 的 `release:publish` 命令：

- 只接受合法 SemVer。
- 要求通过 `--notes` 或 `--notes-file` 提供非空更新说明，两者不得同时使用。
- 将更新说明写入 `latest.json` 的 `notes` 字段，并保持 UTF-8 和换行内容。
- 要求目标 tag 指向当前 commit。
- 从 OSS 获取 `darwin-aarch64.json` 和 `windows-x86_64.json`。
- 拒绝缺失、重复、空签名、版本不一致或未知平台。
- 拒绝 `darwin-x86_64`。
- 检查现有 stable 版本；目标版本不高于 stable 时停止。
- 生成 Tauri v2 静态 `latest.json`。
- 先发布版本化清单、验证，再更新 stable 清单。
- 输出最终 URL、版本、平台和校验结果，不输出任何凭据。

## 7. 停止发布的条件

出现任一情况，Agent 必须停止，不得更新 stable：

- 工作区有未确认的修改。
- Mac 不是 ARM64，或 Windows 不是 x86_64。
- 两台机器 commit/tag 不一致。
- 四处版本号不一致。
- tag 已存在但指向其他 commit。
- Updater 私钥缺失、密码错误或两台机器使用了不同私钥。
- 任一测试或平台构建失败。
- 任一平台 fragment、URL 或签名缺失。
- 制品公共 URL 不是 HTTPS 或无法返回 2xx。
- `latest.json` 包含 `darwin-x86_64` 或未知平台。
- 目标版本不高于已发布 stable 版本。
- 无法确认 stable 上传和回读结果一致。

## 8. 故障和回滚

- 更新 stable 前失败：修复后重新构建或上传；用户不受影响。
- 更新 stable 后发现问题：不要把 stable 简单改回旧版本，因为已升级用户不会自动降级。
- 正确处理方式：修复问题并发布更高补丁版本，例如从 `1.1.0` 发布 `1.1.1`。
- 不删除旧版本制品；需要调查时保留清单、签名、构建 commit 和日志。
- Updater 私钥丢失后，已发布客户端无法验证由新私钥签名的更新，必须安全备份原私钥。

## 9. Agent 完成后的汇报格式

每次执行至少汇报：

```text
版本：
Commit / Tag：
Mac 环境检测：通过/失败
Windows 环境检测：通过/失败
前端测试：
Rust 测试：
darwin-aarch64 制品及 URL：
windows-x86_64 制品及 URL：
latest.json URL 和版本：
旧版本升级验证：
系统代码签名状态：
遗留风险或阻塞：
```

不得在汇报中包含 AK/SK、私钥内容或私钥密码。
