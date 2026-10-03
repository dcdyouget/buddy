# S07-07 窗口拖动与选择隔离

> 状态: `done`
> Phase: 07
> 依赖: S07-02
> 阻塞: —
> 退役设计文档: —

## 目标

按 v1 拖动区交互实现，文字 / 控件不触发拖动。

## 输入

- `docs/tasks/v2.0.0-gpui/07-shell.md`
- `src/hooks/useDragHandle.ts`
- `src/hooks/useDragHandle.test.ts`
- `src/components/shared/GlassPanel.tsx`

## 实现要点

- 按 v1 拖动区交互实现，文字 / 控件不触发拖动。
- 核查既有对话页边缘条，补消息正文空白拖动。

## 验收标准

- [x] 实际位置变化验证空白可拖，文本可选复制、控件可点击、滚动不受影响。
- [x] 拖动和选择互斥分支有真实操作和有效拦截。
- [x] 回归、有效拦截、本地实际渲染验收和提交门禁有记录；临时诊断还原。

## 证据

- 首轮 `shell_preview --selftest-drag` rc=0（`/tmp/drag-baseline.log`）：T47–T49 建立并复用唯一 runtime 后，T52 通过真实 OS 左键序列验证边缘与 Markdown 行尾空白分别位移 45×35px、尺寸不变；字形拖选后 OS Cmd+C 精确得到「拖动选择验收文本」，窗口不移动；模型按钮实际打开菜单；OS 滚轮使 ListOffset 从 item_ix=60/0px 变为 45/10px，窗口保持不变。
- 已补消息行 padding、空 transcript、紧凑页、Composer、Settings / Provider 拖动区；退出动画中的设置页失活时立即释放拖动输入。扩展 T52 尚未通过锁屏预检，首轮 T52 不等于整项已验收。
- Markdown T08 使用真实绘制 bounds 验证空白、字形、链接、解码后的图片、代码复制按钮与只读任务符号隔离；`/tmp/markdown-drag-final.log` rc=0。`verify_markdown_drag.py` 4/4 有效拦截，`/tmp/markdown-drag-interception.log` rc=0；属于 GPUI 事件证据，OS 拖动与选区由 T52 承担。新增传播隔离块无独立自动化证据。
- pages / app 回归 rc=0（`/tmp/pages_preview-drag-fixed.log`、`/tmp/app-drag-inactive-fixed.log`）；T25 保留原时序验证退出设置后立即展开，T30 使用实际字形位置验证不拖动。`verify_drag.py --case inactive-settings` 1/1 有效拦截（`/tmp/drag-inactive-interception.log` rc=0）。vendor patch 从 zed 290cbcb 重建后逐文件一致。
- 待完成扩展 T52、4 项 OS 变异、实际渲染与最终完整 shell。2026-10-03 再测 `CGSSessionScreenIsLocked=1`、CGEventPost 权限为 true；锁屏失败不计有效拦截，也不视作产品失败。

- 2026-10-04 解锁后扩展 T52 rc=0（`/tmp/drag-1004.log`），空 transcript、边缘、正文空白、紧凑边缘、空输入、设置上/右边缘均真实位移 45×35；非空输入、字形选择、按钮与滚动隔离通过。首轮变异 3/5 有效（`/tmp/drag-interception-1004.log` rc=1）：body-blank 锚点重复未执行，scroll-preserved 删除辅助 wheel 回调仍由 ListState 自身滚动而漏检；两项待修正后重测，不计 PASS。

- 随后 `/tmp/drag-stack-1004.log` 及 `/tmp/drag-two-final-1004.log` 的当前基线 rc=1：各拖动及选区 / 模型按钮成功，但滚动 offset 未改变、设置返回失败。后者未进入变异，不计有效拦截；当前不把首轮 PASS 扩大为稳定完成，已增加输入前原生焦点诊断。

- `/tmp/drag-focused-1004.log` rc=1：scroll / Back 前原生 visible/key/app_active 全 true；滚轮仍不改变 offset，Back 点击后窗口被隐藏。单纯异步焦点假设已被证伪，尚未定位事件命中 / 平台呈现问题；没有重试事件或放宽断言。

- 遮挡定位：`/tmp/drag-realcap-1004.log` 的原生命中诊断显示主窗 visible/key/app_active / view alpha / hitTest 均正常，但 `(1020,437)` 滚轮与 `(923,315)` Back 点上方有系统窗口 `51898`，level=8、bounds=(920,272,260,219)。ScreenCaptureKit 实际截图确认是「Zeron 想访问其他 App 的数据」权限弹窗，解释滚轮被截获、Back 被外点监听隐藏；未操作该权限弹窗。
- T52 仅将专用测试主窗移到 `(40,100)` 后开始动作，不修改产品定位、鼠标序列或断言；`/tmp/drag-unobstructed-1004.log` rc=0，全部拖动、选择、控件、滚轮与返回断言 PASS。临时 AppKit / CGWindow 命中 FFI 已移除，保留原生焦点前置和实际渲染检查点。
- 已读取真实 ScreenCaptureKit 窗口图：`/tmp/buddy-shell-sck-1004.png` 紧凑页、`/tmp/buddy-t52-selected-1004.png` 字形选区、`/tmp/buddy-t52-settings-1004.png` 设置页。实色面板、圆角、中文输入 / 控件和高亮选区正常；图片未入库，未更改 Cargo / lock 或引入 render_to_image 特性。

- 修正锚点 / 滚轮变异后，`verify_drag.py --case body-blank --case scroll-preserved` rc=0、2/2 有效（`/tmp/drag-two-unobstructed-1004.log`）；与首轮 inactive-settings / native-drag / glyph-selection 的 3 项合计 5/5。前者仅 blank=false，后者确实 scroll=false，均 rc=1 后 finally 恢复源码；此前锚点未执行、基线失败和漏检保留原记录，不算有效拦截。

- 完整 `shell_preview --selftest` 首轮受 T51 留下的 800×600 尺寸影响，设置右侧点再次与系统弹窗重叠。设置区域也在动作前单独定位到 `(40,100)` 后，`/tmp/shell-full-clear-1004.log` 中 T45–T52 全部 PASS，T52 恢复后滚轮 / 返回 / 各拖动断言全真；整组 rc=1 是 S07-08 T53 约 502ms 停帧，未宣称全链路通过，也未削弱断言。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |

## 完成记录

- 日期：2026-10-04
- commit：`5fd70d5`（产品实现）、`a3d8b60`（真实 OS 验收与测试收尾）
- 设计文档处置：本项无独立设计段落，无需退役。
- 提交门禁：`/tmp/s0707-gate-1004.log` rc=0。本轮 210 UI 单测与 8 组预览回归见 S07-12，当前 T45–T52 完整串联通过；整组仍因 S07-08 T53 失败而 rc=1，未宣称全链路通过。

## 备注

后续 spec 的能力不计入本项完成。
