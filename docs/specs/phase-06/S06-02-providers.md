# S06-02 Provider 卡片与新增流程

> 状态: `done`
> Phase: 06
> 依赖: S06-01, S02-01
> 阻塞: —
> 退役设计文档: `docs/design/pages-and-states.md` 添加 Provider 流程；`docs/design/component-mapping.md` AddProviderPanel / ProviderCard / FooterActions 部分（其他设置子项和窗口外壳保留）

## 目标

移植 Provider 预设选择、连接参数、拉取与选择模型、测速及添加后的配置持久化。

## 输入

- `src/components/settings/AddProviderPanel.tsx`
- `src/components/settings/ProviderCard.tsx`
- `src/stores/configStore.ts`
- `src/styles/global.css`
- `crates/engine/src/models/config.rs`

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S06-02-1 | 实现约束 | 以 v1 当前流程为准拆分小模块；engine 方法经 spawn_engine 执行。 |
| S06-02-2 | 实现约束 | URL / Key 校验与连接错误按 v1 文案显示；秘密仅写入配置 JSON。 |
| S06-02-3 | 实现约束 | 添加过程不能用中间配置切离设置页，成功后返回对话；失败保留输入。 |
| S06-02-4 | 实现约束 | 移植 v1 全部 11 项预设与自定义协议、Key 显隐、模型多选 / 上下文 / 图片与生图能力；Anthropic 不允许生图。 |
| S06-02-5 | 实现约束 | 按 v1 当前 configStore 单次保存完整配置，规范化 provider::model 身份；保留其他 provider、主题和路径配置。 |
| S06-02-6 | 实现约束 | 网络响应与请求时表单版本匹配，切换预设 / URL / 协议后不能导入旧响应；退出覆盖层后子控件立即停止输入。 |

## 验收标准

- [x] 真实输入与点击覆盖预设、连接参数、拉取有效 / 无效响应、模型选择及提交。
- [x] 沙盒真实 engine 配置往返、明文 Key 与异常边界测试及拦截验证通过。
- [x] 本地读取真实渲染核对侧滑面板与 Provider 卡片。

## 证据

| 项 | 证据 |
|----|------|
| 真实网络与输入 T37 | `NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example settings_preview -- --selftest-providers`：真实 engine adapter 连接动态端口回环 mock。通过真实滚轮、鼠标、Cmd+A / Cmd+V 走空字段静默、Key 遮罩初始 / 显示 / 恢复、两个模型全选、首模型测速与失败、空列表、401、非法 JSON、服务端已接收请求后切换目标并丢弃旧结果。每步 `window.refresh(); window.draw(cx).clear(cx)`。夹具收到额外 `HEAD /` 探测，只将 GET/POST 计为业务请求；不靠重复点击或重复删除把失败改成成功。 |
| 完整保存 T38 | 同一 `--selftest-providers`：真实多选、图片 / 生图能力与上下文菜单鼠标选择，菜单完整处于视口；单次 engine 保存后返回 Conversation，内存和 `get_config()` 往返一致，明文沙盒 Key 正确。再添加 OpenAI 时同名 alpha 分别存为 `custom-*::alpha` / `openai::alpha`，API 原始 ID 仍为 alpha，默认项取本次首模型。故意把沙盒 config.json 建成目录，失败后页面、内存与原磁盘配置不变，输入 / 选择保留；还原路径后同一草稿重试成功，随后退出面板并以真实输入核对隔离。 |
| Unicode 与单行 T39 | `NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example settings_preview -- --selftest-controls`：T36/T39 PASS。狭窄字段粘贴长中文 URL，水平滚动大于零、垂直滚动为零；Cmd+Left 返回行首和水平零位。真实粘贴 a中é、启用遮罩后左移 / Backspace 得到 aé，显示切换保留原文，禁用后 Cmd+V / Backspace 不改变内容。 |
| 键盘与二层返回 T40 | `--selftest-providers`：默认返回焦点、Tab/Enter 选择 DeepSeek、鼠标聚焦自定义、Tab/Shift+Tab 到协议 / 返回自定义；展开 Compat 后真实键盘将 Thinking / Max Tokens 改为 deepseek / max_completion_tokens，核对表单绑定；Shift+Tab 循环至取消并 Enter 退出，仅回到外层设置。T35 的原设置 / 会话 / 窗口尺寸回归由同一拦截脚本先跑基线。 |
| 纯逻辑测试 | `cargo test -q -p buddy-ui --lib settings::provider`：13 passed，覆盖全部 11 项预设与兼容参数、URL / Key / 协议版本失效、同 ID 新模型的旧测速丢弃、空 / 失败拉取保留此前成功模型、未选模型不可提交、自定义 Compat、Anthropic 禁生图、Provider 作用域 ID、重复模型 / 旧配置保留，以及 32768 上下文的精确选项值。 |
| 本地视觉读取 | 2026-10-01 临时启用 GPUI test-support，读取浅 / 深 760×640、560×640 的实际 `window.render_to_image()`，核对两列卡片与末行半宽、Key 密码字符、显隐 / 外链图标、能力勾选、上下文菜单向上完整展开、固定页脚与深色对比。尺寸变化等待平台回调并断言真实 viewport，未把未完成 resize 的旧帧作为窄窗口证据。诊断代码、Cargo features/lock 全部还原，图片不入库。视觉像素、系统窗口阴影和 IME 候选窗坐标无独立自动化证据；系统阴影仍归 Phase 07。 |
| 拦截验证 | `NO_PROXY=127.0.0.1,localhost python3 scripts/settings/verify_phase06.py`：45 项均已被行为失败拦截。原设置 14 项，加 Provider / Unicode / 单行 / 保存 / 焦点 / 菜单 / Compat 31 项；首轮 42 项有效，最后菜单项中断后复跑 `--case provider-menu-viewport` 得 1/1，新增 `--case provider-thinking-binding --case provider-max-tokens-binding` 得 2/2。每项正常基线先 PASS，破坏后命中 FAIL / 单测 FAILED 并非零退出，随后还原；编译错误、中断和调试时的基线失败均不算通过。 |
| 全量回归 | 2026-10-01 `cargo test -q -p buddy-ui --lib`：135 passed / 0 failed；按顺序运行 `chat_preview` / `pages_preview` / `app_preview` / `markdown_preview` / `streaming_preview` 的 `--selftest`，均 rc=0 且有 PASS，覆盖 T03–T34（含网络搜索、生图、附件和真实 engine 保存）。GUI 自测串行，避免平台剪贴板互相覆盖。 |
| 提交门禁 | 2026-10-01 按代理配置执行 `scripts/gate.sh > /tmp/gate.log 2>&1`：rc=0，输出 `gate: 全部通过`；纪律、图标来源、纪律拦截、workspace 全目标 / markdown lib / v1 编译通过。实现 commit `e5b85ca`。 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 单次完整保存 | Router 在同一 config_save 队列中合并候选配置，engine 写盘成功才发布并返回 Conversation | 当前 v1 configStore 已使用 addProviderWithModels 事务；原设计文档「分步保存」过时。写盘失败不能提前触发配置页面分类或丢草稿。 |
| 配置任务归属 | 实际工作独立 detach，Router 只保留完成信号；等待前一保存后读取最新配置 | 窗口实体销毁不应取消已经提交的保存；与模型选择共享串行队列，避免快照覆盖。窗口销毁时的保存持续性无独立自动化证据。 |
| 模型身份与兼容性 | 复用 engine 的 raw_model_id / scoped_model_id / normalize_model_ids，预设数据逐项移植 v1 | 同名原始模型可来自不同 Provider，不按 `::` 猜测；重复模型保留旧元数据，与当前 configStore 一致；明文 Key 仅进入沙盒 / 实际配置 JSON。 |
| 请求失效（偏离 v1） | URL / Key / 协议变化清空已拉取模型，以 revision 丢弃旧响应；重新打开表单递增版本 | v1 只在切换预设时清空模型，缺少请求版本校验；防止旧连接响应或旧模型被提交到新连接。 |
| 自定义 Compat（偏离 v1） | 首次仍保持 compat=None；用户修改 Thinking / Max Tokens 后写入兼容配置 | v1 两个 select 只有 defaultValue，无状态更新，实际保存无效；迁移时让可见选择真正生效。 |
| 最终 CSS 真值 | ProviderCard 横向、28px 字母块、12px padding；内容 padding 16 / gap 12；页脚字体 13px | global.css 的 class / !important 覆盖 JSX 原始纵向卡片、40px 字母块和更大的内容间距，按最终生效规则迁移，避免仅看 JSX 产生视觉偏离。 |
| 原生控件差异 | checkbox / select 使用 GPUI 自绘控件与菜单，单行 URL / Key 复用 TextArea 的不换行布局、水平滚动和 Unicode 密码偏移映射 | GPUI 不提供 HTML 原生控件；令牌、卡片布局与动作以 v1 当前代码为准。密码绘制与编辑各自保留原文坐标，不能用密码字符索引直接操作 Key。 |
| 非预设上下文 | 选项包含返回的精确 token 数，标签按 v1 格式显示；订阅保留整数值 | 32768 的标签为 33K，不应反向解析标签得到 33000；重新拉取模型时重建上下文选择器和订阅。 |
| 请求并发（偏离 v1） | 获取 / 测速使用同一 Busy 状态，保存时禁用字段；测速中显示进度 | v1 测速不置 loading，可能与获取并行；迁移时串行请求避免互相覆盖表单，增加等待反馈。 |
| 菜单视口 | 根据实际触发器边界，底部空间不足时向上展开 | 原生 HTML select 由系统处理方向；GPUI 自绘菜单必须显式处理，T38 核对完整菜单与真实鼠标选择。 |
| Key 获取链接 | 保留 v1 的「获取 Key」文字和图标，当前无跳转 | v1 href="#" 且 preventDefault，本就是 no-op；本 spec 不虚构厂商注册网址。 |
| 后续设置范围 | 当前外层已有模型表仍为只读，新增流程内可改多选 / 能力 / 上下文 | 已保存模型编辑归 S06-03；主题、热键、MCP 分别由其他 Phase 06 spec 承接。 |

## 完成记录

- 日期：2026-10-01
- commit：`e5b85ca`
- 设计文档处置：删除 `pages-and-states.md` 的添加 Provider 流程与过时分步保存说明；删除 `component-mapping.md` 的 AddProviderPanel / ProviderCard / FooterActions 映射，保留其他设置与窗口段落；已登记 `design-deletions.md`「已部分删减的文档」，事务与视觉取舍迁入本 spec。
