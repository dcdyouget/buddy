# global-hotkey 0.8.0 的 macOS 独占注册补丁

来源：crates.io 发布包 `global-hotkey 0.8.0`（上游 https://github.com/tauri-apps/global-hotkey）。归档 SHA-256：`8c386b0a4a70cb2d39fffd74480f985b6f0bfbcb934b6a6b6b7e630e448f242e`。

保留发布包的 `src/`、`examples/`、两个 Cargo manifest、README、CHANGELOG 与三份许可证文件；未带入上游 Cargo.lock / 自动化配置。许可证 `Apache-2.0 OR MIT`。Buddy 修改日期：2026-10-03。

唯一源码差异：`src/platform_impl/macos/mod.rs` 的 `RegisterEventHotKey` 使用 `kEventHotKeyExclusive (1 << 0)`，替代 options=0；公共 API、键码映射和其他平台保持原文。

原因：Apple SDK `CarbonEvents.h` 的 HotKeyOptions 说明，非独占注册在其他进程独占占键时仍返回成功但不收到事件。S07-03 独立 child 实测复现了这一行为；应用必须在保存前获得冲突错误并保留旧注册，不能静默保存失效热键。不是 GPUI fork。

复核：从上述 SHA-256 的发布归档解包，逐文件比较本目录的 `src/`；应仅有该常量、原因注释及一处实参差异。根 workspace 的 `[patch.crates-io]` 使用本副本，`exclude` 防止将上游示例及 dev-dependencies 加入 Buddy 工作区；v1 独立 workspace 不受影响。

验收：`shell_preview --selftest-behavior` 的 T47 独立 Carbon exclusive owner；`scripts/shell/verify_behavior.py` 的 `os-exclusive-registration` 将实参还原为 0，必须得到行为 FAIL。升级上游时重新检查其注册策略；若提供等价独占 API，应移除此补丁。
