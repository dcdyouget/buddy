// S01-06-9 / S10-05：冷启动计时工具（v1 与 v2 共用同一口径）
//
// 用法：swift scripts/v1-baseline/winwait.swift <可执行文件> [超时秒=20]
// 环境变量原样传给子进程（调用方用 HOME=<沙盒> 隔离数据目录）。
//
// 口径：`Process.run()` 前一刻 → 该进程拥有的第一个窗口出现在「屏幕上窗口列表」
// （CGWindowListCopyWindowInfo + optionOnScreenOnly，按 kCGWindowOwnerPID 匹配）。
// 只读取窗口元数据（所属进程 / 尺寸），**不需要屏幕录制权限**。
// 输出一行：`pid=<pid> window_ms=<毫秒> bounds=<w>x<h>`；进程保持运行，由调用方结束。

import CoreGraphics
import Foundation

let args = CommandLine.arguments
guard args.count >= 2 else {
    FileHandle.standardError.write("usage: winwait.swift <executable> [timeout_s]\n".data(using: .utf8)!)
    exit(2)
}
let timeout = args.count >= 3 ? Double(args[2]) ?? 20 : 20

let process = Process()
process.executableURL = URL(fileURLWithPath: args[1])
process.standardOutput = FileHandle.nullDevice
process.standardError = FileHandle.nullDevice

let start = DispatchTime.now()
do { try process.run() } catch {
    FileHandle.standardError.write("launch failed: \(error)\n".data(using: .utf8)!)
    exit(1)
}
let pid = process.processIdentifier

while true {
    let elapsed = Double(DispatchTime.now().uptimeNanoseconds - start.uptimeNanoseconds) / 1e6
    if elapsed > timeout * 1000 {
        print("pid=\(pid) window_ms=timeout")
        exit(1)
    }
    let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
    if let w = list.first(where: { ($0[kCGWindowOwnerPID as String] as? Int32) == pid }) {
        let b = w[kCGWindowBounds as String] as? [String: Any] ?? [:]
        let width = b["Width"] as? Double ?? 0
        let height = b["Height"] as? Double ?? 0
        print(String(format: "pid=%d window_ms=%.1f bounds=%.0fx%.0f", pid, elapsed, width, height))
        exit(0)
    }
    usleep(2_000)
}
