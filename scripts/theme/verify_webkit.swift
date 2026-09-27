// S03-02：用 WebKit（与 v1 同一渲染引擎）独立计算每个颜色令牌，作为生成器的对照真值。
//
// 用法：swift scripts/theme/verify_webkit.swift <css 文件> <CSS 属性 | --raw> <令牌名,逗号分隔>
// 输出（stdout）每行：`<light|dark>\t<--token>\t<getComputedStyle 原始字符串>`，解析由调用方完成。
//
// 做法：离屏 WKWebView 载入 CSS（去掉无法解析的 @import），在 <html> / <html class="dark">
// 两种状态下对探针元素设 `<属性>: var(--token)`，读 getComputedStyle 的结果。

import AppKit
import WebKit

let args = CommandLine.arguments
guard args.count == 4, let css = try? String(contentsOfFile: args[1], encoding: .utf8) else {
    FileHandle.standardError.write("usage: verify_webkit.swift <css> <property> <tokens>\n".data(using: .utf8)!)
    exit(2)
}
let property = args[2]
let tokens = args[3].split(separator: ",").map(String.init)
let cleaned = css.split(separator: "\n", omittingEmptySubsequences: false)
    .filter { !$0.trimmingCharacters(in: .whitespaces).hasPrefix("@import") }
    .joined(separator: "\n")

let tokensJSON = String(data: try! JSONSerialization.data(withJSONObject: tokens), encoding: .utf8)!
let js = """
(() => {
  const probe = document.getElementById('probe');
  const out = [];
  for (const mode of ['light', 'dark']) {
    document.documentElement.className = mode === 'dark' ? 'dark' : '';
    for (const t of \(tokensJSON)) {
      let v;
      if ('\(property)' === '--raw') {
        // 自定义属性本身的计算值（WebKit 解析后的原始 token 串）
        v = getComputedStyle(document.documentElement).getPropertyValue(t).replace(/\\s+/g, ' ').trim();
      } else {
        probe.style.setProperty('\(property)', 'var(' + t + ')');
        v = getComputedStyle(probe).getPropertyValue('\(property)');
      }
      out.push(mode + '\\t' + t + '\\t' + v);
    }
  }
  return out.join('\\n');
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
let web = WKWebView(frame: NSRect(x: 0, y: 0, width: 10, height: 10))
web.navigationDelegate = delegate
web.loadHTMLString("<html><head><style>\(cleaned)</style></head><body><div id=probe>x</div></body></html>", baseURL: nil)
DispatchQueue.main.asyncAfter(deadline: .now() + 20) {
    FileHandle.standardError.write("超时\n".data(using: .utf8)!); exit(1)
}
app.run()
