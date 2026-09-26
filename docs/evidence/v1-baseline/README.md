# v1 基线（S01-06）

> 基准版本：tag `v1-final`（`9cc244a`）。**不含截图 / 录屏**（2026-09-26 用户决策）：
> 视觉与行为验收由用户对照运行中的 v1 目检（`git worktree add <dir> v1-final && npm run tauri dev`）。
> 本目录只收录**从代码与实测得出**、可复核的对照材料。

| 文件 | 内容 | 再生方法 |
|------|------|---------|
| 本文件 | 页面 × 状态目检清单、窗口行为事实、性能基线、长会话样本说明 | 手写（事实均附代码出处） |
| [`tokens.md`](./tokens.md) | 设计令牌实测值（浅 / 深）+ 与 `design-tokens.md` 的差异 | `python3 scripts/v1-baseline/extract_tokens.py > docs/evidence/v1-baseline/tokens.md` |

---

## 1. 页面清单（`src/types/index.ts` `PageState`，共 6 个）

| 页面 | 进入条件（代码） | 逻辑尺寸（`src-tauri/src/window/geometry.rs` `logical_page_size`） | 主要组件 |
|------|----------------|------------------|---------|
| `empty` | 有可用模型、无会话 | **560 × 60（紧凑）** | `InputDock`、`ModelDropdown`、`GlassPanel` |
| `noapikey` | 无可用 API Key | **560 × 60（紧凑）** | `KeyRound` 图标 + 前往设置入口 |
| `conversation` | 有消息 | 750 × 500 | `MessageBubble`、`InputDock`、`ModelDropdown`、`ApprovalModal`、回到底部按钮 |
| `streaming` | 生成中 | 750 × 500 | 同上 + `LiveMessageBubble` |
| `settings` | 从任一页进入设置 | 760 × 640 | `ModelList`、`HotkeySetting`、`ThemeSetting`、`UpdateSetting` |
| `add-provider` | 设置内新增 Provider | 760 × 640 | `AddProviderPanel`（`SlideInPanel` 滑入） |

## 2. 各页状态（目检对照清单）

> 目检时逐项在 v1 与 v2 中复现并比较。浅色 / 深色各看一遍（设置 → 主题）。

| 页面 | 状态 | 如何复现 |
|------|------|---------|
| empty | 空闲 / 输入中 / 中文组字中 | 唤起窗口；输入文字；用拼音输入法组字 |
| empty | 模型下拉展开 | 点击模型名 |
| empty | 错误提示（`AlertCircle`） | 断网后发送 |
| noapikey | 默认 | 删除全部 Provider 的 Key |
| conversation | 纯文本 / markdown（标题、列表、表格、代码块） | 让模型输出对应内容 |
| conversation | 思考块折叠 / 展开（`ThinkSection`） | 使用带思考的模型 |
| conversation | 工具调用：进行中 / 成功 / 失败 / 延迟分组（`ToolSection`、`DeferredToolSection`） | 让模型读文件 / 搜索 |
| conversation | 联网搜索卡片（`WebSearchSection`） | 「搜索一下 …」 |
| conversation | 图片生成卡片（`GenerateImageSection`） | 支持生图的模型 |
| conversation | 提问卡片（`AskUserCard` / `QuestionPrompt`） | 模型调用 ask_user |
| conversation | 审批弹窗（`ApprovalModal`） | 让模型写文件 |
| conversation | 消息操作（复制 / 重试 / 编辑，`MessageActions`） | 悬停消息 |
| conversation | 附件图片（`AttachmentImage`，含「图片已删除」） | 粘贴图片发送；再删除附件文件 |
| conversation | 错误态：`401` / `429` / `5xx` / `network`（`uiStore.ts:23` `errorType`） | 错误 Key / 限流 / 断网 |
| conversation | 回到底部按钮（`ChevronDown`） | 上滑历史 |
| streaming | 流式渐显、停止按钮 | 发送长问题；生成中点停止 |
| settings | Provider 卡片、模型列表、热键录制、主题、更新 | 打开设置逐项查看 |
| add-provider | 预设选择 / 自定义 / 拉取模型 / 测速 | 设置 → 新增 Provider |

## 3. 窗口行为事实（Phase 07 必须对齐）

| 事实 | 出处 |
|------|------|
| **紧凑页（empty / noapikey）→ 内容页时展开一次**，以窗口**底边为锚**、水平居中，距工作区边缘 12px；内容页之间切换**保持用户尺寸**（即硬约束 6 的准确含义） | `src/utils/windowResize.ts`（`COMPACT_PAGES`）；`geometry.rs` `calculate_bottom_anchored_target_geometry`、`WINDOW_MARGIN_LOGICAL = 12` |
| **闲置 ≥ 10 分钟后再唤起，以紧凑形态打开** | `src-tauri/src/window/mod.rs` `COMPACT_AFTER_IDLE`、`mark_invoked_at` |
| 初始窗口：无装饰、透明、无阴影、`acceptFirstMouse`、全工作区可见、不抢焦点、不进任务栏、最小 360 × 60 | `src-tauri/tauri.conf.json` `app.windows[0]` |
| 实测窗口尺寸 **550 × 60**（配置写 560 × 60） | `winwait.swift` 读 `kCGWindowBounds`，5 次一致 → S07-01 需查明 10px 差 |
| 呼出 / 隐藏经事件 `buddy:window-will-show` / `buddy:window-will-hide` 通知前端播放动画 | `window/mod.rs`、`hotkey.rs:50` |
| 唤起时带入选中文本（macOS） | `platform/macos.rs:138` 事件 `selected-text` |

## 4. 设计令牌

见 [`tokens.md`](./tokens.md)。要点：

- 浅色 139 个变量、深色覆盖 46 个；`design-tokens.md` 声明的令牌**全部存在**，但代码另有 **82 个**文档未声明（代码块语法色、流式动画、工具卡片、中性阶 `--neutral-*` 等）。
- 展开 `var()` 后**真正不一致 8 处，全在表面色**：文档为半透明 `rgba`（毛玻璃时代的意图），代码为**不透明实色**（v1 本就是实色界面，S00-04）。**S03-02 迁移以代码为准。**
- `--font-sans` 首选字体为 **`'Fira Code'`（等宽）**、基础字重 **650** —— 这是 v1 的真实观感，S03-04 需忠实迁移而非「修正」。

## 5. 性能基线

方法：`scripts/v1-baseline/perf_baseline.sh <可执行文件> [轮数] [--long-session]`（v2 在 S10-05 用**同一脚本**）。
二进制：`npx tauri build --no-bundle` 产出的 `src-tauri/target/release/buddy`（20 MB；源码与 `v1-final` 的 `src/` `src-tauri/` 差异 0 文件）。
每轮使用全新沙盒 `HOME`（数据目录随之隔离，**不读写用户真实历史**，只复制 `config.json`）。
机器：macOS 26.4 arm64，2026-09-27。

| 指标 | 结果（5 轮） | 说明 |
|------|------------|------|
| 启动 → 首个窗口上屏 | **中位数 204 ms**（第 1 轮 939 ms，其余 193–214 ms） | 第 1 轮含新二进制首次加载；`winwait.swift` 按窗口列表判定，无需屏幕录制权限 |
| 主进程常驻内存（空闲 5s，`footprint`） | **26 MB**（26.0–27.0） | 稳定 |
| 主进程 + 本轮新起 WebKit 辅助进程 | 中位数 142 MB（79–165） | **噪声大**：上一轮辅助进程退出时序影响归属判断；只作量级参考 |
| 同上，数据目录含 1200 条长会话 | 中位数 137 MB，启动 198 ms（3 轮） | 与空历史无差异 —— v1 启动停在紧凑空页，**不加载历史** |

**未采集**（记录原因，交 S10-05）：流式帧率、长会话滚动帧率 —— 需在 UI 内插桩或录屏；v1 无插桩点，录屏已由用户决策取消。S10-05 在 v2 内置计数器测量，v1 侧以用户目检对照。

## 6. 长会话样本

`python3 scripts/v1-baseline/gen_long_session.py` → `target/v1-baseline/long-session/`（不入库，可再生）。
1200 条（user 514 / assistant 600 / tool 86）、12 个分块、881 KB；固定种子 `20260927`，两次生成内容 md5 相同（`76620bf5…`）。
覆盖：长段落、代码块、表格、列表、思考块、工具调用 + 结果、中英混排。
可被 engine 按 v1 格式完整读回：`cargo test -p buddy-engine --test storage_roundtrip -- --ignored` → `manifest total=1200 loaded=1200`。
