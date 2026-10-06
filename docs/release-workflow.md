# Buddy 发布与升级

适用：macOS Apple Silicon（`darwin-aarch64`）与 Windows 10/11 x64（`windows-x86_64`）。

## 固定地址

| 用途 | 地址 |
|---|---|
| 版本清单（客户端检查更新；产品介绍页读取最新安装包地址） | `https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/channels/stable.json` |
| 版本制品 | `https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/releases/<版本>/...` |
| GitHub Release（DMG 备用下载、源码） | `https://github.com/dcdyouget/buddy/releases` |

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
      "installer": { "url": ".../Buddy_0.1.2_x86_64.zip", "size": 0, "sha256": "…", "signature": "…" }
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
    windows/x86_64/Buddy_<版本>_x86_64.zip(.sig)         ← 含 buddy.exe 与许可的便携安装包
```

所有对象使用 `Cache-Control: no-cache`（浏览器向 OSS 校验 ETag，未变化时 304）。

## 发版

```bash
npm run release                                      # 交互输入版本号与更新说明
npm run release -- 0.2.0 --notes "修复 xxx；新增 yyy"
npm run release -- 0.2.0 --notes-file notes.txt --yes
```

`scripts/release/release.sh` 依次：环境与版本检查（新版本须高于线上）→ 写版本号并本地提交 → `scripts/gate.sh` 与全部测试 → 推送该精确提交并触发 GitHub Actions 的 Windows 构建 → 等待并下载该运行的未签名 EXE、ZIP → release 构建 → 组装 `Buddy.app`、更新包、DMG，并用 Mac 钥匙串中的同一更新私钥签名全部四个更新制品 → 上传版本目录并公网回读校验 → 确认 → 打标签并推送源码标签 → 覆盖 `channels/stable.json` → 创建附带两平台安装包的 GitHub Release（标记为 Latest）。

覆盖 `stable.json` 这一步让应用内更新看到新版本；之前任何一步失败都不影响用户，修复后重跑即可。
源码推送失败时不会切换更新清单；创建 Release 失败时，脚本会打印需要手动补做的命令。开始前脚本会检查 `gh` 已登录、GitHub 上的 `main` 没有本地缺少的提交。

其他选项：`--skip-tests`（重跑上传 / 发布时）、`--skip-publish`（只上传版本目录，不发布、不推送）、`--windows-dir <目录>`（改用本地未签名 Windows EXE 与 ZIP，跳过 Actions 下载）。

## Windows 构建与双平台发布

正式发布默认在 Mac 上执行，此路径在 Mac 本地使用更新私钥。脚本在写入版本号并通过本地测试后，先将该精确提交推送到 `main`，再以该完整 SHA 作为 `ref` 触发 `.github/workflows/windows.yml`。脚本只接受 `head_sha` 完全相同、由刚刚触发的 `workflow_dispatch` 运行产生的 `Buddy-windows-x86_64` 制品；它等待该运行成功后下载其中未签名的 EXE 和 ZIP。这样不会误用其它分支、旧提交或普通 push 检查的制品。

下载后的 `Buddy_<版本>_x86_64.exe` 和 `Buddy_<版本>_x86_64.zip` 在 Mac 上由与 macOS 更新包相同的私钥签名。脚本随后用 `buddy-update` 客户端内置公钥逐个验证两个 Windows 签名，才会生成清单、上传或切换 `stable.json`。`windows.yml` 从不接触更新私钥；未通过签名验证时发布停止，线上稳定通道保持不变。

生产发布也可走标签 CI：推送 `v<版本>` 会触发 `.github/workflows/release.yml`，它在受信任的 tag/手动调度环境中用 GitHub Secrets 对 macOS 与 Windows 制品签名。构建完成后先下载两个已签名制品到 Windows 发布机的 `.release/<版本>/macos/aarch64` 和 `.release/<版本>/windows/x86_64`，再运行 `scripts/release/publish-artifacts.ps1 -Version <版本> -NotesFile <更新说明>`。脚本以客户端内置公钥验证、上传 OSS 并回读校验、创建 GitHub Release，最后切换 `stable.json`；它必须在与版本标签相同的源代码提交上执行。该路径的私钥只供受信任的 release 工作流读取，普通 `windows.yml` 的 push、PR 和手动构建仍只生成未签名 Windows 制品。

下载命令为 `gh run download <运行ID> --name Buddy-darwin-aarch64-v<版本> --dir .release/<版本>/macos/aarch64`，Windows 包使用名称 `Buddy-windows-x86_64-v<版本>` 和目录 `.release/<版本>/windows/x86_64`。

网络受限或需要重跑时，`--windows-dir` 可替代自动下载。目录须只含同版本、尚未签名的 `Buddy_<版本>_x86_64.exe` 和 `Buddy_<版本>_x86_64.zip`；Mac 脚本仍会负责签名和客户端校验：

```bash
npm run release -- 0.2.0 --notes-file notes.txt --windows-dir /path/to/windows/x86_64
```

清单保持 `schema: 1`。同一 `version` 下所有平台包必须对应同一版本；不能保留旧 Windows 包却把顶层版本改成新版本。正式发布必须同时包含 Mac 与 Windows 包。

Windows 安装 ZIP 解压到用户可写目录即可。更新链路为：下载 `.exe` → 大小 / SHA-256 / minisign 校验 → 同目录暂存 → 退出进程 → 后台 PowerShell 替换旧 EXE 并重启。替换失败保留或恢复旧程序。开发构建与示例不自动更新；安装到不可写目录时会提示暂存失败，需改用用户目录。配置和历史消息独立保存在 `%APPDATA%\com.buddy.chat`。

`.github/workflows/windows.yml` 的 push 和 PR 仅做检查与测试；`workflow_dispatch` 接收精确 `ref`，才构建并上传包含未签名 EXE、ZIP 的 `Buddy-windows-x86_64` 制品。可通过 `target/release/buddy.exe --selfcheck-window` 在已解锁的 Windows 桌面验证窗口创建、隐藏、唤回和聚焦，测试数据目录与用户配置隔离。

这里只采用更新签名，未配置 Windows Authenticode 证书；首次运行可能出现 SmartScreen 提示。

## 凭据

| 凭据 | 位置 |
|---|---|
| 更新签名私钥 | `~/.tauri/buddy-v2.key`（公钥内置于 `crates/update/src/lib.rs` 的 `PUBLIC_KEY`）。**丢失后已发布客户端无法再验证任何新包**，私钥与密码须离线备份 |
| 私钥密码 | 钥匙串 `buddy-updater-key` / 账户 `buddy`（或环境变量 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`） |
| OSS AccessKey | `~/.ossutilconfig`（ossutil 2.x），需要 `buddy-release/buddy/*` 的 `GetObject`、`PutObject` |
| GitHub | `gh` 已登录且有 `dcdyouget/buddy` 的推送权限 |

## 出问题时

- 发布后发现问题：不要把 `stable.json` 改回旧版本（已升级的用户不会降级），修复后发布更高的修订版本。
- 不删除已发布的版本目录与 GitHub Release。
- 安装包为 ad-hoc 签名：新用户首次打开会被 Gatekeeper 拦截，需右键「打开」或在系统设置中允许。
