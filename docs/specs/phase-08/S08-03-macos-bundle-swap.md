# S08-03 macOS bundle 替换安装

> 状态: `done`
> Phase: 08
> 依赖: S08-02
> 阻塞: —
> 退役设计文档: —

## 目标

把已验证的更新包解压、校验后原子替换当前 `Buddy.app`，失败恢复原版本，并在旧进程退出后启动新版本。

## 输入

- `docs/tasks/v2.0.0-gpui/08-release.md` D08、D10、D11
- S07-11 单实例：新进程启动时若旧实例仍在，会把唤起转发给旧实例后退出

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| D08 | 替换 | `/usr/bin/tar` 解压 → 只允许一个 `.app` → 校验 bundle id、版本、可执行文件、`codesign --verify --deep --strict` → `ditto` 到同目录 `.Buddy.app.update-new` → 原包改名 `.update-old` → 新包改名为原名 → 删除旧包 |
| D10 | 回退 | 第二次改名失败时把 `.update-old` 改回原名；下次启动清理残留备份与下载缓存 |
| D11 | 权限 | 只有建立同目录暂存目录得到 `PermissionDenied` 才经 osascript 管理员授权执行同一序列；用户取消显示「已取消授权，更新未安装」 |
| — | 拒绝场景 | 开发构建（不在 `.app` 内）、DMG 卷、App Translocation 随机路径 |
| — | 重启 | 派生 `/bin/sh` 等待本进程 pid 退出（最多 60 秒）后 `open` 新应用 |

## 验收标准

- [x] 合法包通过；他人 bundle id、版本不符、签名后被篡改的包被拒绝
- [x] 替换后目标为新版本且不留备份；目录内恰好一个 `.app` 的约束成立
- [x] 线上旧版本 → 新版本真实升级并重启
- [x] 开发构建拒绝自更新

## 证据

| 项 | 证据 |
|----|------|
| 单测 | `macos_tests.rs` 6 项：合法包通过；`com.example.other`、版本 0.3.0 ≠ 0.2.0、签名后改写 `marker` 均被拒；`swap` 后 marker 为 new 且目录只剩 `Buddy.app`；单 `.app` 约束；单引号路径转义；`cargo test` 可执行文件 `current_app()` 返回开发构建错误 |
| 真实升级 | 在临时 worktree 以 `0.0.9` 构建 `examples/update_probe.rs`，用 `bundle-macos.sh` 装为 `~/BuddyUpdateTest/Buddy.app`（0.0.9）。运行输出：读取线上 `stable.json` → 发现 0.1.0（12208540 字节）及说明 → 下载 25/50/75/100% → 「sha256 + 签名校验通过」→「安装完成」。读回：Info.plist 0.1.0，`codesign --verify --deep --strict` 通过，`Contents/MacOS/buddy` 与发布构建 sha256 同为 `a7ee66aba065a198…`，目录内仅 `Buddy.app` |
| 真实重启 | 重新装回 0.0.9 后以 `--relaunch` 运行：探针退出后 `pgrep -x buddy` 得到 `~/BuddyUpdateTest/Buddy.app/Contents/MacOS/buddy`（真实 0.1.0 产品），6 秒后仍在运行；`~/Library/Caches/com.buddy.chat/updates` 已被启动清理删除；之后以 Apple Event 退出。测试目录、worktree 与构建目录已删除 |
| 证据边界 | 未在 root 拥有的 `/Applications` 实测管理员授权分支；未实测「新包改名失败」的回滚分支（无法在不改代码的情况下制造该失败）。两者只有代码路径，无独立证据 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 解压工具 | 系统 `/usr/bin/tar` | 不引入 tar / flate2 依赖；bsdtar 保留扩展属性 |
| 复制到同目录再改名 | `ditto` + 两次 `rename` | 缓存目录可能与安装目录不在同一卷；同目录 rename 才是原子的 |
| 提权判定 | 只看 `PermissionDenied` | 避免磁盘满等其他错误误弹管理员授权 |
| 重启方式 | 外部进程等旧进程退出再 `open` | 单实例锁会让「先启动新进程」变成唤起旧进程 |

## 完成记录

- 日期：2026-10-04
- commit：`11c84bd`、`ea4c8f9`
- 设计文档处置：无对应设计文档
