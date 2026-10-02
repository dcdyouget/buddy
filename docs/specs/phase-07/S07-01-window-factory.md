# S07-01 窗口创建工厂与配置

> 状态: `doing`
> Phase: 07
> 依赖: S01-01
> 阻塞: —
> 退役设计文档: `docs/design/rust-architecture.md` §Window Configuration；`docs/design/component-mapping.md` 整份最后剩余窗口容器

## 目标

统一生产窗口工厂，初始 560×60、最小 360×60、可调整大小、透明原生承载 Theme 实色。

## 输入

- `docs/tasks/v2.0.0-gpui/07-shell.md`
- `src-tauri/tauri.conf.json`
- `src/utils/windowResize.ts`
- `src/App.tsx`
- `src-tauri/src/window/geometry.rs`
- `apps/buddy/examples/app_preview.rs`
- `docs/specs/phase-05/S05-18-page-state.md`

## 实现要点

- 统一生产窗口工厂，初始 560×60、最小 360×60、可调整大小、透明原生承载 Theme 实色。
- 装配真实 engine 和 PageRouter，初始化字体 / 图标 / HTTP / markdown / engine 桥接；产品使用默认数据目录，预览独立沙盒。
- 紧凑页展开到对话 750×500 / 设置 760×640，普通内容切换保留用户尺寸；设置从紧凑页进入后返回对话为 750×500。
- 沿用已接受的回紧凑页缩回 560×60；不新增磁盘尺寸字段。定位 / 底边锚定归 S07-06，生命周期归 S07-11。

## 验收标准

- [x] 生产主入口呈现真实页面，统一配置和页面尺寸策略有单测。
- [x] 真实窗口覆盖紧凑 / 对话 / 设置与用户调整后内容切换保留，关键分支有效拦截。
- [x] 回归、有效拦截、本地实际渲染验收和提交门禁有记录；临时诊断还原。

## 证据

| 项 | 可复现材料与结果 |
|----|------------------|
| 产品入口 | `apps/buddy/src/main.rs` 通过 `shell::init` / `open_main_window` 组装真实 PageRouter，默认数据目录与 v1 相同；不迁移、不复制历史。预览入口 `apps/buddy/examples/shell_preview.rs` 走同一工厂，只有数据使用独立沙盒。 |
| 单测 | `cargo test -q -p buddy-ui --lib`：167 passed；新增 6 项覆盖配置、中文非法尺寸诊断、逻辑像素转换与切页矩阵 / 用户尺寸保留。 |
| 真实窗口 T45 | `NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example shell_preview -- --selftest`：创建选项（focus / titlebar / kind / min / resizable）、初始 560×60，真实点击设置 760×640、点击返回对话 750×500，Cmd-V / Enter 发送并经历流式；用户 resize 900×700 后慢流 / 设置往返保持尺寸，配置失效回空态缩至 560×60，另一冷窗口紧凑页直接发送展开为 750×500。全部为真实输入和实际 window.bounds；强制帧使用 refresh / draw / clear。 |
| 本地实际渲染 | 临时 test-support 捕获并读取浅 / 深各 560×60、760×640、750×500、900×700 实际帧：正文 / 控件 / 图标可见，原生无标题栏，Theme 边框与圆角一致。诊断特性、Cargo.lock 增量和示例代码已还原；图片不提交。 |
| 拦截 | `python3 scripts/shell/verify_window.py`：22/23 有效拦截，其中 S07-01 的 15 项全部命中单测 FAILED 或真实 T45/T46 FAIL；覆盖三种尺寸、最小配置、可调 / focus / 提前显示 / 原生透明选项、两类非法尺寸、页面分派、订阅 resize、设置来源、回缩和用户尺寸保持。各变异 finally 恢复原始字节；无编译错误 / 超时伪拦截。剩余 1 项原生抢焦点未捕获，见 S07-02，不计入通过。 |
| 回归 / 门禁 | chat_preview、pages_preview、app_preview、markdown_preview、streaming_preview、settings_preview 的 --selftest，以及 settings_preview 的 --selftest-preferences、shell_preview 的 --selftest 共 8 组均 rc=0 且输出 PASS；`cargo check -q -p buddy-app` 无编译警告；暂存后 `scripts/gate.sh` rc=0（全部通过）。 |
| 自动化边界 | 未进行真实边缘拖拽来验证最小尺寸强制；配置与 WindowOptions 的最小值有自动证据，该项原生拖拽无独立自动化证据。系统显示 / 原生对象异常清理的所有错误分支无独立自动化证据。Windows 实测归 Phase 09。 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |
| 尺寸真值 | 560×60 / 750×500 / 760×640 | 依据当前 `tauri.conf.json` / `geometry.rs`；旧设计中的 460×78 过时。 |
| 用户尺寸 | 内容页之间不 resize，不新增落盘字段 | 当前 v1 的 `windowResize.ts` 仅在离开紧凑页时调整尺寸，配置中没有尺寸记忆；旧素材的磁盘持久化要求不成立。 |
| 已接受偏离 v1 | 从内容页回紧凑页缩回 560×60 | 用户在 S05-18 目检已确认；沿用统一外壳而非恢复 v1 的大窗口中央气泡。 |
| 设置入口差异 | 紧凑页进入设置 760×640，返回对话 750×500；对话页进入设置保留尺寸 | 当前 v1 的明确分支，已由用户确认。设置来源保存在单一窗口外壳中。 |
| 临时定位边界 | 默认屏幕居中，resize 保留左上角 | 目前统一工厂只管尺寸；v1 光标屏幕 / 每屏位置和底边锚定由 S07-06 承接，不能据当前预览声称已完成。 |
| 生命周期边界 | 主窗口创建与真实页面接入先行 | 热键 / 隐藏 / tray / 自启 / 单实例由各 S07-* 实现，本项不重复建立暂用系统注册。 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：

## 备注

后续 spec 的能力不计入本项完成。
