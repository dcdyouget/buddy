# S04-09 复制内容的 v1 基准

| 文件 | 内容 |
|------|------|
| `copy-sample.md` | 用例源文本（段落、中文加粗、列表、任务项、多段引用、h1/h3/h4、硬换行、表格、代码块） |
| `v1-rendered.html` | v1 `StreamingMarkdown`（`v1-final`，`isStreaming=false`，等代码块懒加载完成）的真实渲染 HTML |
| `v1-selection.json` | 在 WKWebView 中选中整条消息后 `getSelection().toString()` 的结果（JSON 字符串） |

## 复现

```bash
# 1) v1 渲染 HTML：临时测试文件渲染 StreamingMarkdown 并写出 innerHTML（用完删除，不改 src/）
#    内容见 S04-09 spec「证据」段；用 v1 自带 vitest（npx vitest run <临时文件>）
# 2) 选区文本（v1 CSS 取自 v1-final）
git show v1-final:src/styles/global.css > /tmp/v1.css
swift scripts/v1-baseline/selection_text.swift /tmp/v1.css docs/evidence/s04-09/v1-rendered.html
# 3) v2 对照（真实拖选 + Copy，读剪贴板后恢复）
cargo run -p buddy-app --example markdown_preview -- --selftest   # 看 T07
```

## 注意

- 选区外包 `.message-bubble` 并带 v1 `MessageBubble.tsx` 的内联 `font-size:14px; line-height:1.6`。
  缺了它 WebKit 以 16px 计算，h3 后不再多空行 —— 首次测量即因此得到错误基准，已更正。
- `getSelection().toString()` 与 Cmd+C 写入的纯文本同出 WebKit 的 TextIterator；本基准以此代表 v1 的复制结果。
