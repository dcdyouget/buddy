// S03-02：用 WebKit（与 v1 同一渲染引擎）独立计算每个颜色令牌，作为生成器的对照真值。
//
// 用法：swift scripts/theme/verify_webkit.swift <css 文件> <令牌名,逗号分隔>
// 输出（stdout）每行：`<light|dark> <--token> <r> <g> <b> <a>`（0..1 浮点）
//
// 做法：离屏 WKWebView 载入 CSS（去掉无法解析的 @import），在 <html> / <html class="dark">
// 两种状态下对探针元素设 `color: var(--token)`，读 getComputedStyle 的结果。

import AppKit
import WebKit

let args = CommandLine.arguments
guard args.count == 3, let css = try? String(contentsOfFile: args[1], encoding: .utf8) else {
    FileHandle.standardError.write("usage: verify_webkit.swift <css> <tokens>\n".data(using: .utf8)!)
    exit(2)
}
let tokens = args[2].split(separator: ",").map(String.init)
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
      probe.style.color = 'var(' + t + ')';
      out.push(mode + '\\t' + t + '\\t' + getComputedStyle(probe).color);
    }
  }
  return out.join('\\n');
})()
"""

final class Delegate: NSObject, WKNavigationDelegate {
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        webView.evaluateJavaScript(js) { result, error in
            if let error { FileHandle.standardError.write("\(error)\n".data(using: .utf8)!); exit(1) }
            for line in (result as? String ?? "").split(separator: "\n") {
                let parts = line.split(separator: "\t").map(String.init)
                guard parts.count == 3, let c = parseColor(parts[2]) else {
                    FileHandle.standardError.write("无法解析：\(line)\n".data(using: .utf8)!); exit(1)
                }
                print("\(parts[0]) \(parts[1]) \(c.0) \(c.1) \(c.2) \(c.3)")
            }
            exit(0)
        }
    }
}

/// 解析 `rgb(r, g, b)` / `rgba(r, g, b, a)` / `color(srgb r g b / a)`
func parseColor(_ s: String) -> (Double, Double, Double, Double)? {
    let nums = s.replacingOccurrences(of: "[^0-9.e\\- ]", with: " ", options: .regularExpression)
        .split(separator: " ").compactMap { Double($0) }
    if s.hasPrefix("color(srgb") {
        guard nums.count >= 3 else { return nil }
        return (nums[0], nums[1], nums[2], nums.count >= 4 ? nums[3] : 1)
    }
    if s.hasPrefix("rgb") {
        guard nums.count >= 3 else { return nil }
        return (nums[0] / 255, nums[1] / 255, nums[2] / 255, nums.count >= 4 ? nums[3] : 1)
    }
    return nil
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
