# Phase 10: 测试与双平台验收

## 目标

建立完整的测试体系，并给出双平台的最终验收基线。

与 Phase 02-09 并行推进，但**验收在各自 Phase 完成后执行**。

## 相关文档

- `AGENTS.md` — 硬约束 1-10（验收核心依据）
- `docs/design/pages-and-states.md` — 7 个页面 + 状态机（视觉验收基准）
- `docs/specs/phase-01/S01-06-v1-baseline.md` — **v1 视觉与行为基线**（视觉验收基准；`docs/design/prototypes/` 经审计确认不存在）
- `docs/design/design-tokens.md` — 令牌校验基准
- `docs/CONVENTIONS.md` — 编码规则

## 验收标准

- [ ] `buddy-engine` 单测全部通过
- [ ] 分层断言在 CI 中生效（engine 无 GPUI / 无 GPL）
- [ ] 硬约束 1-10 在双平台逐条有验证记录
- [ ] 7 个页面视觉对照通过（含原型比对截图）
- [ ] 关键交互路径有端到端覆盖
- [ ] 性能基线已建立并与旧 Tauri 版本对比
- [ ] 双平台差异清单已完成并有明确接受/修复结论

## 测试体系

### 10.1 分层测试策略

| 层 | 范围 | 工具 |
|----|------|------|
| L1 单元 | engine 各模块 | `cargo test`（现有测试基础上扩展） |
| L2 契约 | provider / SSE / 存储格式 | `cargo test` + 录制回放 |
| L3 组件 | UI 组件行为与布局 | GPUI `test-support` |
| L4 集成 | 页面级状态机 | GPUI `test-support` |
| L5 端到端 | 完整用户路径 | 手工 + 脚本 |
| L6 视觉 | 与原型对照 | 截图比对 |
| L7 性能 | 启动 / 滚动 / 内存 | 采集脚本 |

现有前端测试需迁移或废弃：

| 现有测试 | 行数 | 处理 |
|---------|------|------|
| `src/stores/chatStore.test.ts` | 455 | 逻辑迁到 engine 或 UI 层重写 |
| `src/stores/configStore.test.ts` | 143 | 同上 |
| `src/components/chat/StreamingMarkdown.test.tsx` | 270 | 由 Phase 04 的解析器测试替代 |
| `src/components/chat/ToolSection.test.tsx` | 229 | 重写为 GPUI 组件测试 |
| `src/components/chat/MessageBubble.test.tsx` | 147 | 重写 |
| `src/components/chat/InputDock.test.tsx` | 202 | 重写 |
| `src/components/chat/ThinkSection.test.tsx` | — | 重写 |
| `src/components/chat/GenerateImageSection.test.tsx` | — | 重写 |
| `src/components/chat/MessageActions.test.tsx` | — | 重写 |
| `src/components/settings/*.test.tsx` | — | 重写 |
| `src/components/shared/*.test.tsx` | — | 重写 |
| `src/hooks/useSmoothTextRenderer.test.tsx` | 174 | 废弃（逻辑消失） |
| `src/hooks/useSmoothWheelScroll.test.ts` | 205 | 废弃（GPUI 原生滚动） |
| `src/hooks/useDragHandle.test.ts` | — | 迁移到 Phase 07 |

## 开发工作

### 10.2 硬约束验证清单（核心）

| # | 硬约束 | 验证方式 | macOS | Windows |
|---|--------|---------|-------|---------|
| 1 | 无交通灯按钮，零装饰 | 截图 + 目视 | ☐ | ☐ |
| 2 | 单一品牌色 `#5B5FE9`，状态色仅 4 种 | 代码扫描（禁止硬编码色值） | ☐ | ☐ |
| 3 | 圆角仅 4/8/12/16/9999 | 代码断言 | ☐ | ☐ |
| 4 | 无 emoji 图标 | 源码扫描 | ☐ | ☐ |
| 5 | 只用设计令牌，无硬编码颜色/阴影/间距 | 代码扫描 + 评审 | ☐ | ☐ |
| 6 | 窗口切页不改变尺寸 | 逐页切换录屏 | ☐ | ☐ |
| 7 | Esc/点击外部关闭，**不中断流式** | 流式中关闭 → 重开验证 | ☐ | ☐ |
| 8 | 单会话流，无多会话 UI | 评审 | ☐ | ☐ |
| 9 | API Key 明文 JSON，不用钥匙串 | 文件检查 + 代码 | ☐ | ☐ |
| 10 | 全部文案中文 | 源码扫描 | ☐ | ☐ |

### 10.3 页面验收（对照 v1 基线）

7 个页面逐一验收，每页需：**v1 基线截图** vs 新实现截图对照 + 状态覆盖。

> 基线材料由 `S01-06` 采集至 `docs/evidence/v1-baseline/`。原计划对照 `docs/design/prototypes/`，但审计确认该目录不存在。

| ID | Task | Details |
|----|------|---------|
| D01 | 页面清单确认 | 按 `docs/design/pages-and-states.md` 列出全部页面 |
| D02 | 每页状态枚举 | 空态 / 加载 / 错误 / 完整态 |
| D03 | 基线对照 | 与 `docs/evidence/v1-baseline/`（待创建，由 S01-06 产出）的截图逐个比对 |
| D04 | 差异记录 | 无法一致处记录原因与是否接受 |
| D05 | 双平台对照 | 每页 macOS / Windows 各一份截图 |

### 10.4 关键路径端到端

| ID | Task | Details |
|----|------|---------|
| D06 | 首次启动 | 无配置 → 引导 → 添加 Provider → 首次对话 |
| D07 | 流式对话 | 发送 → 流式 → 完成 → 持久化 → 重启后可见 |
| D08 | 中断与恢复 | 流式中关闭窗口 → 重开 → 状态正确 |
| D09 | 工具调用 | 完整工具调用链 + 审批流 |
| D10 | 提问交互 | 模型提问 → 用户回答 → 继续 |
| D11 | 网络图片生成 | 生成 → 显示 → 保存 |
| D12 | 附件 | 粘贴/拖拽图片 → 发送 → 显示 |
| D13 | 长会话 | 1000+ 消息的加载、滚动、搜索 |
| D14 | 设置变更 | 改 Provider / 模型 / 热键 / 主题 → 重启后保留 |
| D15 | 更新流程 | 检查 → 下载 → 安装 → 重启 |
| D16 | 错误路径 | 无效 Key / 断网 / 超时 / 服务端错误 |

### 10.5 性能基线

采集口径必须与 `S01-06` 一致，否则无法对比。

| ID | Task | Details |
|----|------|---------|
| D17 | 冷启动耗时 | 进程启动 → 可响应热键 |
| D18 | 热键唤起延迟 | 按键 → 窗口可见 |
| D19 | 常驻内存 | 空闲 / 有会话 / 长会话三态 |
| D20 | 流式渲染帧率 | 长回复流式期间的帧率 |
| D21 | 大列表滚动帧率 | 1000 / 5000 条消息 |
| D22 | 包体积 | 与现有 7.2MB dmg 对比 |
| D23 | 对比报告 | 新旧实现逐项对比，明确提升与退化项（基准值来自 S01-06 采集的 v1 数据） |

### 10.6 稳定性

| ID | Task | Details |
|----|------|---------|
| D24 | 长时间运行 | 数小时常驻，内存不增长、句柄不泄漏 |
| D25 | 异常恢复 | 网络抖动 / 服务端 5xx / 流中断 的恢复 |
| D26 | 存储原子性 | 写入过程中被中断不损坏数据 |
| D27 | 崩溃处理 | 崩溃后可正常重启，状态可恢复 |
| D28 | 睡眠唤醒 | 多次睡眠/唤醒后功能正常 |

### 10.7 代码质量

| ID | Task | Details |
|----|------|---------|
| D29 | 分层断言 | CI 检查 engine 无 GPUI / 无 GPL（Phase 01 D15-D16） |
| D30 | 文件长度 | 遵守 `docs/CONVENTIONS.md` 的 ≤300 行规则 |
| D31 | 许可证扫描 | `cargo-deny` 或 `cargo-about`，报告与 `THIRD_PARTY_NOTICES.md` 一致 |
| D32 | 无 emoji / 无硬编码扫描 | 脚本化检查 |
| D33 | 警告清零 | `cargo clippy` 无警告 |

## 测试工作（本 Phase 自身的元测试）

| ID | Task | Details |
|----|------|---------|
| T01 | 交付清单 | 产出 `docs/evidence/acceptance-report.md`（**待创建**，由 S10-07 产出；放 `docs/evidence/` 而非 `tasks/`，因它是验收证据而非任务描述） |
| T02 | 已知限制 | 明确列出未完成项与不修的原因 |
| T03 | 回退预案 | 记录若上线后出问题如何回退到 Tauri 版本 |
| T04 | Windows 差异清单 | 汇总 `platform-differences.md` 的最终结论 |

## 备注

硬约束验证清单（10.2）是本 Phase 最重要的产物——它是**唯一能证明迁移没有丢东西**的依据。建议在 Phase 05 开始时就建立该表并持续填写，而非最后补。
