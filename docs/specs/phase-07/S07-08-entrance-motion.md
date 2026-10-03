# S07-08 入场动画与减弱动效

> 状态: `doing`
> Phase: 07
> 依赖: S07-02
> 阻塞: —（继续定位真实第三帧约 477ms，尚未满足验收）
> 退役设计文档: —

## 目标

按当前 v1 入场触发条件，Theme 时长 / 曲线，避免逐帧原生尺寸变更。

## 输入

- `docs/tasks/v2.0.0-gpui/07-shell.md`
- `src/components/shared/WindowEntrance.tsx`
- `src/components/shared/WindowEntrance.test.tsx`
- `src/styles/global.css`
- `crates/ui/src/accessibility.rs`

## 实现要点

- 按当前 v1 入场触发条件，Theme 时长 / 曲线，避免逐帧原生尺寸变更。
- 系统减弱动态跳过动画，核对再次显示 / 切页触发条件。

## 验收标准

- [ ] 实际帧 / 时间采样验证端点、时长、减弱动态分支；有效拦截。
- [ ] 真实系统设置证据不足时如实记录。
- [ ] 回归、有效拦截、本地实际渲染验收和提交门禁有记录；临时诊断还原。

## 证据

- 已接入 `shell/entrance.rs`、AppShell underlay 与 runtime 显隐；按当前 v1 `window-surface-morph` 分属性关键帧区间应用 Theme 的 200ms / EASE_STANDARD，背景混合 Theme primary 4% + surface，整体 opacity 含阴影。正文与原生窗口尺寸不参与动画。
- 已添加纯时间采样和减弱动态状态测试，以及复用主 runtime 的 T53：读取最近一次真实 render 的 frame / phase / elapsed，并比对 native bounds、普通切页、再次显示和展开页静态。197 项 UI 单测 rc=0（`/tmp/ui-s070708.log`）；`verify_entrance.py` 的 reduced-play / reduced-reset / duration / curve / keyframe-opacity / initial-scale 六项有效拦截并还原，rc=0（`/tmp/entrance-unit-interception.log`）。T53 与 show-trigger / expanded-static 两项 OS 变异未执行；桌面再次锁定，未计入完成。
- 当前 v1 的装饰 underlay 大部分被不透明 Composer / NoKey 面板覆盖；不新增上层炫光或内容形变。系统减弱动态另一状态如未实际切换，不以注入布尔值的单测冒充系统证据。

- 2026-10-04 首次 T53 rc=1（`/tmp/entrance-1004.log`）：原生 bounds / replay / 普通切页 / 展开静态通过，但轮询只有 3ms、26ms 两个动画帧，随后 200ms 终态，未满足 >=3 帧断言。保留原验收强度，改进平台帧采样后重测；不把此次失败当作通过。

- 真实平台帧栅栏重测仍失败（`/tmp/entrance-native-1004.log`）：visible/key/app_active/on_active_space 全 true，几何采样 27–57µs，但仅 6/15ms 与约 503ms 收到样本，第三次仍是早期 underlay，settled=false。强制 callback refresh 诊断亦失败（`/tmp/entrance-refresh-1004.log`），已移除临时诊断，不修改 200ms 时长与 >=3 帧断言。环境为 144Hz/60Hz 两块虚拟显示器，主屏截图超时；显示链路仅为待查假设，不宣称根因已证实。

- 最新完整串联 `/tmp/shell-full-clear-1004.log` 中 T45–T52 全 PASS，T53 仍约 502ms 停帧 / settled=false。系统截图命令虽超时，但初始化 NSApplication 后的 ScreenCaptureKit CLI 已能读取真实窗口图；因此不能据截图命令超时认定显示链路故障。另发现固定 16 帧上限在 144Hz 下不足 200ms，已改为真实 350ms 采样截止，并排除截止后才恢复的样本；继续定位，不减少 >=3 帧与终态要求。

- 主线程采样 `/tmp/entrance-mainthread-1004.txt` 与 `/tmp/entrance-profile-1004.log` 显示 callback 在 15ms 到达，前台任务直到 430ms 才恢复。样本主导于 Window::draw / Taffy 布局，Metal nextDrawable 仅少量样本；当前不能归因于显示器或 Metal。此采样会扰动性能，只用于定位，不作为时序验收通过证据。

- GPUI 内置 ZED_MEASUREMENTS 的 `/tmp/entrance-measure-1004.log` 确认前三帧 3.9 / 3.3 / 476.8ms。underlay 百分比几何改为等价 viewport 固定几何的诊断仍为 477.1ms（`/tmp/entrance-fixed-layout-1004.log` rc=1），已完整还原，不保留无收益改动。

- 补充终态帧请求的确定性回归，真实系统设置与时钟经内部参数隔离；`/tmp/ui-phase07-final-1004.log` 211/211 PASS。`/tmp/entrance-unit-final-1004.log` 纯逻辑 7/7 有效拦截并还原（含新增 followup-frame）；这不替代仍失败的真实 T53。

- 最小 AppKit A/B 定位：GPUI 的主窗口为 PopUp，默认 NSWindowAnimationBehaviorUtilityWindow。仅设置该专用窗口 animationBehavior=None 后 `/tmp/entrance-no-native-animation-1004.log` rc=0，实际动画帧稳定约 3–4ms，T53 timing / settled / no_page_replay / replay / expanded_static / router 全 PASS。原始 350ms 截止、>=3 帧、200ms 终态与几何断言未放宽；尚须 UtilityWindow 回退变异和无测量日志复测。

## 决策记录

最终验证补充：原生动画策略已移至 `native::PreparedMainWindow::apply_and_show`，在首次 `orderFrontRegardless` 前生效，`visibility.rs` 临时诊断已还原。`/tmp/entrance-gui-interception-1004.log` rc=0，native-animation / show-trigger / expanded-static 共 3/3 有效 FAIL 并还原；加纯逻辑 7/7，共 10/10。恢复后的 `/tmp/shell-full-final-1004.log` rc=0，T45–T53 完整串联全部 PASS，T53 最后读取 `(200, Settled, None)`，真实采样约 206ms。系统减弱动态开启状态未实际切换，其分支仅有确定性单测与变异证据，明确不冒充真实系统设置验收。

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| AppKit 自动显隐动画 | NSWindowAnimationBehaviorNone | GPUI PopUp 默认 UtilityWindow 的系统显隐动画与 AppShell 叠加，实测第三帧约 477ms；仅禁用系统自动动画后恢复正常帧。应用入场曲线、内容和原生尺寸保持原有定义，不 fork GPUI。 |
| 状态收尾时机 | 200ms 视觉结束即 settled | v1 的 260ms 是 DOM phase 清理保护；GPUI 按采样结束停止请求帧，不额外延长不可见终态。此实现差异单独登记。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：

## 备注

后续 spec 的能力不计入本项完成。
