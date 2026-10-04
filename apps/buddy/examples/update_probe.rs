//! 自更新端到端探针（S08-03 证据）：不经界面，按产品同一调用链执行
//! `check → download → install [→ relaunch_after_exit]`，打印每一步结果。
//!
//! 用法（必须放进 `.app` 内运行，否则 install 会按开发构建拒绝）：
//!   1. 以较低版本号构建本探针，复制为 `<测试目录>/Buddy.app/Contents/MacOS/buddy`
//!   2. 运行该文件：`<测试目录>/Buddy.app/Contents/MacOS/buddy [--relaunch]`
//!   3. 观察：`<测试目录>/Buddy.app` 被替换为清单中的新版本
use std::time::Duration;

fn main() {
    let relaunch = std::env::args().any(|a| a == "--relaunch");
    let runtime = tokio::runtime::Runtime::new().expect("tokio");
    let code = runtime.block_on(async move {
        println!("当前版本：{}", buddy_update::current_version());
        println!("清单地址：{}", buddy_update::MANIFEST_URL);
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(30))
            .build()
            .expect("client");
        let release = match buddy_update::check(&client).await {
            Ok(Some(release)) => release,
            Ok(None) => {
                println!("检查结果：当前已是最新版本");
                return 0;
            }
            Err(e) => {
                println!("检查失败：{e}（{}）", e.detail());
                return 1;
            }
        };
        println!("发现新版本：{}（{} 字节）\n更新说明：{}", release.version, release.update.size, release.notes);
        let mut last = 0;
        let downloaded = match buddy_update::download(&client, &release, |done, total| {
            let percent = if total == 0 { 0 } else { done * 100 / total };
            if percent >= last + 25 || done == total {
                println!("下载进度：{percent}%");
                last = percent;
            }
        })
        .await
        {
            Ok(d) => d,
            Err(e) => {
                println!("下载失败：{e}（{}）", e.detail());
                return 1;
            }
        };
        println!("下载完成且 sha256 + 签名校验通过：{}", downloaded.archive.display());
        let installed = match tokio::task::spawn_blocking(move || buddy_update::install(&downloaded))
            .await
            .expect("join")
        {
            Ok(i) => i,
            Err(e) => {
                println!("安装失败：{e}（{}）", e.detail());
                return 1;
            }
        };
        println!("安装完成：{}", installed.app.display());
        if relaunch {
            match buddy_update::relaunch_after_exit(&installed) {
                Ok(()) => println!("已安排在本进程退出后启动新版本"),
                Err(e) => {
                    println!("安排重启失败：{e}");
                    return 1;
                }
            }
        }
        0
    });
    std::process::exit(code);
}
