// S04-09：v1（WebKit）中「选中整条助手消息」得到的文本 —— 即 Cmd+C 复制的纯文本
//
// 用法：swift scripts/v1-baseline/selection_text.swift <v1 global.css> <v1 渲染出的消息 HTML>
// 输出（stdout）：选区文本的 JSON 字符串（便于逐字节比对，换行等不可见字符原样可见）
//
// 做法：离屏 WKWebView 载入 v1 CSS 与消息 HTML（StreamingMarkdown 渲染结果，自带 `.ai-message-content`），
// 外包 `.message-bubble` 并带上 v1 `MessageBubble.tsx` 助手消息的内联样式（font-size 14px / line-height 1.6
// —— 这决定 WebKit 是否在段落 / 标题后多输出空行），以 Range.selectNodeContents 选中，读 `getSelection().toString()`。

import AppKit
import WebKit

let args = CommandLine.arguments
guard args.count == 3,
      let css = try? String(contentsOfFile: args[1], encoding: .utf8),
      let body = try? String(contentsOfFile: args[2], encoding: .utf8) else {
    FileHandle.standardError.write("usage: selection_text.swift <css> <html>\n".data(using: .utf8)!)
    exit(2)
}
let cleaned = css.split(separator: "\n", omittingEmptySubsequences: false)
    .filter { !$0.trimmingCharacters(in: .whitespaces).hasPrefix("@import") }
    .joined(separator: "\n")

let js = """
(() => {
  const content = document.getElementById('content');
  const range = document.createRange();
  range.selectNodeContents(content);
  const sel = window.getSelection();
  sel.removeAllRanges();
  sel.addRange(range);
  return JSON.stringify(sel.toString());
})()
"""

final class Delegate: NSObject, WKNavigationDelegate {
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        webView.evaluateJavaScript(js) { result, error in
            if let error { FileHandle.standardError.write("\(error)\n".data(using: .utf8)!); exit(1) }
            print(result as? String ?? "")
            exit(0)
        }
    }
}

let app = NSApplication.shared
let delegate = Delegate()
let web = WKWebView(frame: NSRect(x: 0, y: 0, width: 600, height: 2000))
web.navigationDelegate = delegate
web.loadHTMLString("<html><head><style>\(cleaned)</style></head><body><div class=message-bubble style='width:100%;font-size:14px;line-height:1.6'><div id=content>\(body)</div></div></body></html>", baseURL: nil)
app.run()
