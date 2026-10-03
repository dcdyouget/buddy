# S07-08 入场动画与减弱动效

> 状态: `blocked`
> Phase: 07
> 依赖: S07-02
> 阻塞: 解锁后 T53 平台帧在约 20ms 后停滞至约 500ms；真实帧数量 / 终态未满足，原因待定位。
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

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 状态收尾时机 | 200ms 视觉结束即 settled | v1 的 260ms 是 DOM phase 清理保护；GPUI 按采样结束停止请求帧，不额外延长不可见终态。此实现差异单独登记。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：

## 备注

后续 spec 的能力不计入本项完成。
