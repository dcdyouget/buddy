# S07-12 窗口行为自检模式（探测静默 no-op）

> 状态: `blocked`
> Phase: 07
> 依赖: S07-02
> 阻塞: 真实 macOS 自检预检发现会话锁定；完整字段与原生补丁变异待解锁
> 退役设计文档: —

## 目标

复用原生探测，扩展对应已完成的层级 / 工作区 / 位置 / 显隐验证。

## 输入

- `docs/tasks/v2.0.0-gpui/07-shell.md`
- `docs/tasks/v2.0.0-gpui/research-log.md`
- `docs/specs/phase-00/S00-02-floating-panel.md`

## 实现要点

- 复用原生探测，扩展对应已完成的层级 / 工作区 / 位置 / 显隐验证。
- 中文说明 + PASS / FAIL + 非零失败退出码，明确平台支持，不能把无异常当通过。

## 验收标准

- [ ] 故意移除原生补丁时输出失败字段，恢复 PASS。
- [ ] macOS 实测和 Windows 待测边界清晰。
- [ ] 回归、有效拦截、本地实际渲染验收和提交门禁有记录；临时诊断还原。

## 证据

- `cargo run -q -p buddy-app --bin buddy -- --selfcheck-window`：在单实例 / engine / 产品窗口初始化前分流到专用诊断沙盒，不接热键、托盘或真实数据。锁屏预检输出 `BLOCKED S07-12`，rc=1（`/tmp/product-selfcheck.log`）；不是有效失败拦截。
- 诊断读取真实 style / shadow / opaque / collection / layer / radius / 首击 / workspace，并实际移动、恢复、隐藏和显示后读回；原生操作在退出 GPUI App 借用后执行，窗口与沙盒清理纳入结果。
- 判定函数 fixture 对各个错误字段逐一拒绝，`verify_lifecycle.py --case diagnostic-shadow` 已有效拦截（包含在 `/tmp/lifecycle-interception.log` 的 6/6 中）。此为判定层证据，native-shadow 移除原生补丁的完整 OS 路径尚待桌面解锁。
- Windows 明确输出未支持 / 待测并非零退出，留 Phase 09；没有把平台占位当作通过。

- 最终回归：`/tmp/s071112-final-summary.log` rc=0，210 UI 单测、chat / pages / app / markdown / streaming / settings、settings preferences、shell window 共 8 组预览全部 rc=0 且 PASS。完整 OS 验收不包含在该结果中。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：

## 备注

后续 spec 的能力不计入本项完成。
