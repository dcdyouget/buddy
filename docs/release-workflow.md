# Buddy 发布与升级

适用：macOS Apple Silicon（`darwin-aarch64`）与 Windows 10/11 x64（`windows-x86_64`）。

## 固定地址

| 用途 | 地址 |
|---|---|
| 版本清单（客户端检查更新；产品介绍页读取最新安装包地址） | `https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/channels/stable.json` |
| 版本制品 | `https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/releases/<版本>/...` |
| GitHub Release（安装包备用下载、源码） | `https://github.com/dcdyouget/buddy/releases` |

清单地址写死在已发布的客户端里（`crates/update/src/lib.rs` 的 `MANIFEST_URL`）：
bucket `buddy-release`（cn-beijing）不能删除、改名或换地域。换地址只能在新版本客户端中进行，并让旧地址继续发布到旧版本用户都升级为止。

产品介绍页按操作系统读取 `platforms["darwin-aarch64"].installer.url` 或 `platforms["windows-x86_64"].installer.url`，源码链接读 `source.url`（GitHub 上该版本的标签；GPL 要求提供源码，仓库须保持公开）。

## 清单格式（`schema: 1`）

```json
{
  "schema": 1,
  "version": "0.1.2",
  "pub_date": "2026-10-05T00:00:00Z",
  "notes": "中文更新说明",
  "platforms": {
    "darwin-aarch64": {
      "update":    { "url": ".../Buddy_0.1.2_aarch64.app.tar.xz", "size": 0, "sha256": "…", "signature": "…" },
      "installer": { "url": ".../Buddy_0.1.2_aarch64.dmg",        "size": 0, "sha256": "…", "signature": "…" }
    },
    "windows-x86_64": {
      "update":    { "url": ".../Buddy_0.1.2_x86_64.exe", "size": 0, "sha256": "…", "signature": "…" },
      "installer": { "url": ".../Buddy_0.1.2_x86_64_setup.exe", "size": 0, "sha256": "…", "signature": "…" },
      "portable":  { "url": ".../Buddy_0.1.2_x86_64.zip", "size": 0, "sha256": "…", "signature": "…" }
    }
  },
  "source": { "url": "https://github.com/dcdyouget/buddy/tree/v0.1.2" }
}
```

由 `scripts/release/manifest.mjs` 生成、`crates/update/src/manifest.rs` 解析；不兼容的格式变更必须提升 `schema`。

## OSS 目录

```
buddy/
  channels/stable.json                                  ← 发布开关，唯一会被覆盖的对象
  releases/<版本>/
    manifest.json
    macos/aarch64/Buddy_<版本>_aarch64.app.tar.xz(.sig)   ← 应用内更新包
    macos/aarch64/Buddy_<版本>_aarch64.dmg(.sig)          ← 安装包
    windows/x86_64/Buddy_<版本>_x86_64.exe(.sig)         ← 已签名原始 EXE 更新包
    windows/x86_64/Buddy_<版本>_x86_64_setup.exe(.sig)   ← NSIS 中文安装向导（默认下载）
    windows/x86_64/Buddy_<版本>_x86_64.zip(.sig)         ← 可选便携版，含运行库与许可
```

所有对象使用 `Cache-Control: no-cache`（浏览器向 OSS 校验 ETag，未变化时 304）。

## 发版

发版只走 GitHub Actions 的 **Release** 工作流，本机不需要编译、签名或上传：

1. 代码在本地提交并通过 `scripts/gate.sh`，推送到 `main`。
2. 在 **Actions → Release → Run workflow** 选 `main`，`mode` 选 `release`，填写新版本号（须高于线上版本）和中文更新说明。也可以用 CLI：

```bash
gh workflow run release.yml --ref main -f mode=release -f version=0.2.0 -f notes="更新说明"
```

3. 工作流向 `main` 提交版本号（`chore(release): v<版本>`），本地随后执行 `git pull --ff-only` 同步。

不要手动改版本号或创建标签，由工作流统一处理。`Cargo.toml`、`Cargo.lock`、`package.json`、`package-lock.json` 的版本号必须一致，否则准备阶段会失败。

### 工作流做什么

- **准备**：检查输入与 Secrets；在自建 runner 上探测到 OSS 的上传与公网 HEAD（探测文件写入 `buddy/releases/ci-check/`，不触碰稳定通道）；提交版本号，并原子推送 `main` 与不可变的 `v<版本>` 源码标签。
- **构建**：Mac ARM64 与 Windows x64 分别在标准 GitHub runner 上测试、构建、打包，用 `tauri signer`（只用它的签名命令，产物为 minisign 格式）签名全部五个发布制品。
- **发布**（自建 runner）：用客户端内置公钥再次验证五个制品，生成双平台清单，上传包、签名、更新说明和版本清单到 OSS，并通过公网 HEAD 检查可访问性和 Content-Length；然后创建仅附带 macOS DMG 和 Windows setup.exe 的 GitHub Release（标记为 Latest），最后覆盖 `channels/stable.json`。

整次发布串行锁定；构建、签名或上传失败时不会切换稳定通道，用户不受影响。普通 push 不会发布。

超过 8 MiB 的包通过 OSS 原生分片上传（4 MiB 分片、最多四路并发）；失败的分片上传会尝试中止，不会发布不完整包。上传后的公网 HEAD 检查以 `Accept-Encoding: identity` 请求——OSS 对声明支持 gzip 的请求会压缩 JSON / 文本并去掉 Content-Length。

### 失败后

- 优先使用 **Re-run failed jobs**，复用本次已签名制品继续发布。已存在的版本标签不会移动，已上传的包不会被不同内容覆盖。
- 修复发布脚本后要继续上传同一版本：`mode` 设为 `resume`，填写原版本号、原 Release 运行 ID（`build_run`）和更新说明。要求原两平台构建均成功、运行源码与公开版本标签匹配；不重建、不移动源码标签。

```bash
gh workflow run release.yml --ref main -f mode=resume -f version=0.2.0 -f build_run=123456789 -f notes="更新说明"
```

- 修复的是应用代码：使用更高的版本号重新发版。

### 发版 runner（自建）

GitHub 托管 runner 在海外，到北京 OSS 的跨境链路只有几十 KB/s 且经常卡死，因此**凡是访问 OSS 的任务（准备阶段的 OSS 探测、发布、`resume`）都跑在国内自建 runner 上**（`runs-on: [self-hosted, oss-publisher]`）；Mac / Windows 构建仍用托管 runner。自建 runner 离线时，发版会停在准备阶段排队等待。

| 项 | 值 |
|---|---|
| 机器 | 局域网服务器 `192.168.31.219`（Ubuntu 22.04，本机 SSH 别名 `219`） |
| 部署 | Docker：`/opt/buddy-runner/Dockerfile`（官方 `ghcr.io/actions/actions-runner` + gcc / gh / xz）与 `/opt/buddy-runner/compose.yaml`，容器 `buddy-runner`，`restart: unless-stopped` |
| 持久卷 | `buddy-runner-home` 挂到 `/home/runner`：runner 注册信息、Rust 工具链、Node、校验程序编译缓存（`CARGO_TARGET_DIR`） |
| 网络 | 访问 GitHub 走 Mac mini 代理 `192.168.31.162:7890`；`*.aliyuncs.com` 直连（`NO_PROXY`）。Node 的上传请求本身也不走环境变量代理 |
| GitHub 名称 / 标签 | `oss-publisher-219` / `oss-publisher` |

常用操作（在服务器上）：

```bash
cd /opt/buddy-runner
docker compose ps / docker compose logs -f        # 状态与日志
docker compose restart                            # 重启
# 重新注册（换机器或注册失效）：在本机取 token 后执行
#   gh api -X POST repos/dcdyouget/buddy/actions/runners/registration-token --jq .token
docker compose down && docker compose run --rm --no-deps runner ./config.sh --unattended \
  --url https://github.com/dcdyouget/buddy --token <token> --name oss-publisher-219 --labels oss-publisher --work _work --replace
docker compose up -d
```

仓库为公开仓库，自建 runner 有被外部 PR 利用的风险：仓库已设置 **Settings → Actions → General → Approval for running fork pull request workflows = Require approval for all external contributors**，外部 PR 的工作流须人工批准才会运行，审批时注意其是否修改了 `runs-on`。CI（`ci.yml`）不得使用自建 runner。

### Secrets

仓库 **Settings → Secrets and variables → Actions** 需要四个 Secrets：

| 名称 | 内容 |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | 更新签名私钥文件的完整内容，必须与已发布客户端公钥匹配 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 私钥密码（无密码则可为空） |
| `OSS_ACCESS_KEY_ID` | 阿里云 OSS AccessKey ID |
| `OSS_ACCESS_KEY_SECRET` | 对应 AccessKey Secret，需有 `buddy-release/buddy/*` 的上传、读取权限 |

更新签名私钥原件为 `~/.tauri/buddy-v2.key`（公钥内置于 `crates/update/src/lib.rs` 的 `PUBLIC_KEY`）。**丢失后已发布客户端无法再验证任何新包**，私钥与密码须离线备份。

### 缓存与费用

构建任务使用按平台共享的 Rust 缓存，并将 `RUSTUP_HOME` 隔离到 runner 临时目录，由 `dtolnay/rust-toolchain@1.95.0` 安装固定工具链；缓存首次填充时需要一次冷构建。发布与 `resume` 在自建 runner 上运行，工具链与校验程序编译产物保存在其持久卷中。

Buddy 为公开仓库，标准 GitHub 托管 runner 的运行分钟数免费；私有仓库有套餐分钟和存储额度，详见 [GitHub Actions 计费](https://docs.github.com/en/billing/concepts/product-billing/github-actions)。临时构建制品只保留 7 天，长期安装包位于 OSS 和 GitHub Release。

## CI

`.github/workflows/ci.yml` 在 push 到 `main` 与 PR（仅涉及代码、脚本、依赖或工作流时）上做 Windows 编译与测试。`workflow_dispatch` 接收精确 `ref`，手动构建并上传包含未签名 EXE、安装器和 ZIP 的 `Buddy-windows-x86_64` 制品，用于测试安装包，不参与发布。

Windows 普通 push / PR 只恢复共享缓存；完整发版或手动 Windows 打包成功后才保存同时包含 test / release 依赖的缓存，避免 debug-only 缓存占据不可覆盖的键。

## 发布包

清单保持 `schema: 1`。同一 `version` 下所有平台包必须对应同一版本；正式发布必须同时包含 Mac 与 Windows 包。

GitHub Release 仅附加 macOS DMG 和 Windows setup.exe；更新包、便携 ZIP 及签名只保存在 OSS。

Windows 默认下载 NSIS 的 `_setup.exe`：中文向导，默认安装到 `%LOCALAPPDATA%\Programs\Buddy`，无需管理员权限；添加开始菜单入口，可选创建桌面快捷方式，并登记在「已安装的应用」中。卸载只移除程序、快捷方式及对应注册项，保留 `%APPDATA%\com.buddy.chat` 中的配置和历史消息。`portable` 是可选 ZIP 下载；0.1.11 及更早的清单中 `installer` 为 ZIP，`resume` 恢复旧制品时兼容这一格式。

应用内更新始终读取 `update.url`：Mac 为 `.app.tar.xz`，Windows 为原始 EXE（不会把安装器当作更新程序）。Windows 更新链路：下载 `.exe` → 大小 / SHA-256 / minisign 校验 → 同目录暂存 → 退出进程 → 后台 PowerShell 替换旧 EXE 并重启；替换失败保留或恢复旧程序。开发构建与示例不自动更新；安装到不可写目录时会提示暂存失败。

只采用更新签名：Mac 安装包为 ad-hoc 签名，新用户首次打开会被 Gatekeeper 拦截，需右键「打开」或在系统设置中允许；Windows 未配置 Authenticode 证书，首次运行可能出现 SmartScreen 提示。

## 出问题时

- 发布后发现问题：不要把 `stable.json` 改回旧版本（已升级的用户不会降级），修复后发布更高的修订版本。
- 不删除已发布的版本目录与 GitHub Release。
