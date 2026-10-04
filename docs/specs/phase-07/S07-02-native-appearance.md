# S07-02 无装饰 / 去阴影 / acceptsFirstMouse

> 状态: `done`
> Phase: 07
> 依赖: S00-02
> 阻塞: —
> 退役设计文档: —

## 目标

在首次显示前把 macOS 主窗口改为无装饰、无系统阴影、16px 内容裁剪，并验证未聚焦时首次点击可进入控件。

## 输入

- `docs/tasks/v2.0.0-gpui/07-shell.md`
- `src-tauri/tauri.conf.json`
- `src-tauri/src/platform/macos.rs`
- `docs/specs/phase-00/S00-02-floating-panel.md`
- `docs/specs/phase-00/S00-04-backdrop.md`
- `docs/evidence/s00-02/window-patch.rs`
- `docs/evidence/s00-04/window-appearance.rs`

## 实现要点

- 主线程由 RawWindowHandle 取当前 NSWindow / GPUIView；只在连续同步调用中暂持 StrongPtr，不跨 await / 线程、不入实体。PopUp GPUIPanel 去掉 Titled / Closable / Miniaturizable，按配置补回 Resizable，保留 NonactivatingPanel。
- 关闭主窗口系统阴影；内容层以 Theme RADIUS_XL 的 16px 圆角裁剪，恢复 v1 level 0。仅补丁主窗口，模型菜单保留系统阴影。
- acceptsFirstMouse 为锁定 GPUI 内建 YES，探测实际 selector；不新增 fork / subclass。Windows 实测归 Phase 09。
- 依赖计划：UI 新增 raw-window-handle 0.6 直接依赖，闭包已有 0.6.2，不新增包；沿用 objc 0.2。本段先于依赖修改登记。
- GPUI 先创建并绘制隐藏窗口，退出 App 借用后执行原生补丁，再首次显示；AlreadyVisible guard 防止显示后补丁。合成后及间隔后读回真实属性。工作区完整策略归 S07-05。

## 验收标准

- [x] 实际 NSWindow 读回无标题栏 / 控件、可调整、无阴影、16px 圆角和裁剪。
- [x] 实际 selector 确认首次点击，真实输入覆盖点击处理；明确模拟事件与系统点击的证据区别。
- [x] 有效拦截抓到无装饰 / 阴影 / 裁剪故障，无法独立拦截的属性如实记录。
- [x] 回归、有效拦截、本地实际渲染验收和提交门禁有记录；临时诊断还原。

## 证据

| 项 | 可复现材料与结果 |
|----|------------------|
| 实现 | `crates/ui/src/shell/native.rs`：只读 prepare 保留原生对象，退出 GPUI 借用后同步 apply / show；错误返回中文，工厂失败删除窗口。主窗口专用补丁，不影响独立模型菜单。 |
| 真实读回 T46 | 与 S07-01 同一 `shell_preview --selftest`；立即 / 强制合成后 / 延迟 140ms 三次真实结果一致：styleMask=32904（0x8088，Resizable / Nonactivating / FullSizeContentView）、level=0、hasShadow=false、isOpaque=false、collectionBehavior=257、wantsLayer=true、cornerRadius=16、masksToBounds=true、GPUIView acceptsFirstMouse=true、isVisible=true、isKey=false。 |
| 首击输入 | `window.dispatch_event` 发送 MouseMove / MouseDown(first_mouse=true) / MouseUp，实际设置按钮由紧凑页进入设置；不是直接调用按钮回调。该输入结果不等同于独立 OS 首击投递证据。 |
| 真实帧 | 本地读取浅 / 深紧凑、对话、设置及用户尺寸帧；零原生标题栏 / 按钮，角区透明、主体为既有 Theme 实色。render_to_image 不含系统窗口阴影，系统阴影证据来自真实 hasShadow 读回。临时诊断还原，不提交图片。 |
| 拦截 | `python3 scripts/shell/verify_window.py`：22/23 有效拦截；原生标题栏、可调位、阴影、层级、圆角、裁剪、显示 7 项均真实 FAIL，提前显示 / 透明背景另有有效拦截。`native-focus` 漏检：把 orderFrontRegardless 改成 makeKeyAndOrderFront 后，真实 isKey 仍为 false、自测仍 PASS（脚本 rc=1）。扩充请求激活应用后再开第二窗口，`--case native-focus` 再跑仍 0/1，未将其计作通过；保留脚本以复现证据缺口。所有源码均恢复。 |
| 回归 / 门禁 | chat_preview、pages_preview、app_preview、markdown_preview、streaming_preview、settings_preview 的 --selftest，以及 settings_preview 的 --selftest-preferences、shell_preview 的 --selftest 共 8 组均 rc=0 且输出 PASS；167 UI 单测通过；`cargo check -q -p buddy-app` 无编译警告；暂存后 `scripts/gate.sh` rc=0（全部通过）。 |
| 自动化边界 | 原生不抢焦点读回为 false，但 native-focus 变异未造成真实 key 状态改变，**该项无独立自动化拦截证据**；不能据请求 App::activate 声称系统已激活。上游 GPUIView 内建 acceptsFirstMouse=YES 不在本仓库变异，该项无独立自动化拦截证据；真实未激活应用的 OS 鼠标投递也无独立自动化证据。layer wantsLayer 本身无独立拦截，圆角值 / masksToBounds 有独立拦截。Windows 无真实验收，交 Phase 09。 |

## 决策记录

补充验收（2026-10-04 复核）：上文 native-focus 无独立证据描述仅对应早期锁屏阶段。解锁后的 `/tmp/window-interception-current.log` 为 23/23，native-focus 子日志明确读回 key=true、T46 字段探测 false。最新 `/tmp/window-focus-final-1004.log` 也为 1/1 有效拦截并还原。GPUIView 内建首击、wantsLayer 自身及 Windows 的既有证据边界不因此扩大。

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |
| 原生调用时机 | 先隐藏创建 / 注册 GPUI 窗口，释放 App 借用后连续补丁 / 显示 | setStyleMask 同步触发 GPUI resize 回调，放在 open_window 回调内会打印 RefCell already borrowed；短暂 StrongPtr 保证同步回调期间对象存活，且不跨 await。调整后 T45/T46 无该 ERROR。 |
| 可调大小 | 按 Window::is_resizable 补回原生 Resizable 位 | 锁定 GPUI 的 titlebar=None 分支忽略 is_resizable，原生初值 0x8081；读回并非仅查 Rust 配置。false 配置保留不允许调整。 |
| 首击探测对象 | RawWindowHandle 对应的 GPUIView | acceptsFirstMouse 注册在 VIEW_CLASS；NSWindow.contentView 是外层 NSView（返回 false），GPUIPanel 不实现该 selector。实际 GPUIView 返回 YES，无需 fork 或 subclass。 |
| 层级 / 工作区 | 主窗口 level=0；保留 GPUI 的 collectionBehavior=257 | 当前 v1 普通窗口层级，不能沿用旧 Spike 的 level=3 或 PopUp 默认 101；AllSpaces / 全屏策略完整操作验收归 S07-05。 |
| 表面与阴影 | 不使用 vibrancy，透明原生窗口承载实色 Theme；主窗口去系统阴影 | v1 当前实现与既定用户决策。模型菜单保留已确认的独立窗口系统阴影。 |
| 依赖边界 | raw-window-handle 0.6 与既有 objc 0.2 | raw-window-handle 0.6.2 已在 GPUI 闭包内，Cargo.lock 仅 UI 依赖列表增加 1 行，无新包。 |

## 完成记录

- 日期：2026-10-02
- commit：`4a511e0`
- 设计文档处置：本项无独立退役段；窗口容器与 Window Configuration 由 S07-01 处理。

## 备注

后续 spec 的能力不计入本项完成。

### 发布后缺陷：无法输入文字（2026-10-04 修复）

- 现象：0.1.0 安装包中点击输入框有光标，但英文与中文输入法都无法输入。
- 根因：`apply_and_show` 修改 `styleMask` 后 AppKit 重建边框视图，把第一响应者重置为窗口本身（`GPUIPanel`）。
  诊断读回：`first_responder=GPUIPanel`、`[NSTextInputContext currentInputContext]=nil`；键盘事件仍到达 GPUI（`observe_keystrokes` 有记录），
  但 `replace_text_in_range` 从未被调用。此前的自动化输入测试直接向 GPUI 派发 `PlatformInput`，绕过了 AppKit 文本输入链，所以没有发现。
- 修复：补丁后 `makeFirstResponder:` 恢复 GPUI 视图；`NativeWindowSnapshot` 新增 `view_is_first_responder`，S07-12 自检 T12-01 要求其为真。
- 证据：HID 键盘事件（`CGEventPost`，ABC 布局）输入 `asd` 上屏；微信输入法拼音 `nihao` 显示带下划线的组字与候选框，空格上屏「你好」；
  Esc 隐藏后再唤起仍可输入。产品自检 `--selfcheck-window` rc=0（`文字输入=true`）；删除修复行后同一自检 rc=1（T12-01 FAIL），恢复后通过。
