//! SVG 图标（硬约束 4：只用 SVG，不用 emoji）
//!
//! 图形取自 v1 所用的 Lucide（`lucide-react`，ISC，见 `THIRD_PARTY_NOTICES.md`）；
//! `streaming-star.svg` 为 Buddy 按 v1 `.streaming-next-star` 的 `clip-path` 多边形自绘。
//! 文件在 `crates/ui/assets/icons/`，编译期嵌入。GPUI 把 SVG 当作蒙版绘制，
//! 颜色取元素的 `text_color`（等价于 v1 的 `stroke="currentColor"`）。
//!
//! 使用前须 `Application::with_assets(buddy_ui::icons::Assets)`，否则图标静默不显示。

use gpui::{AssetSource, Pixels, Result, SharedString, Svg, prelude::*, svg};
use std::borrow::Cow;

/// 声明图标：变体名 => 文件名（`assets/icons/<文件名>.svg`）。
/// Lucide 图标用 `scripts/icons/lucide_svg.py <名字>` 从 v1 的 lucide-react 生成。
macro_rules! icons {
    ($($(#[$doc:meta])* $variant:ident => $file:literal,)*) => {
        /// 图标名（按需增加；每项须在 `assets/icons/` 有同名文件）
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum IconName {
            $($(#[$doc])* $variant,)*
        }

        impl IconName {
            const ALL: &[IconName] = &[$(IconName::$variant,)*];

            /// 资源路径
            pub fn path(self) -> &'static str {
                match self {
                    $(IconName::$variant => concat!("icons/", $file, ".svg"),)*
                }
            }

            fn bytes(self) -> &'static [u8] {
                match self {
                    $(IconName::$variant => include_bytes!(concat!("../assets/icons/", $file, ".svg")),)*
                }
            }
        }
    };
}

icons! {
    /// 复制（lucide `copy`）
    Copy => "copy",
    /// 对勾（lucide `check`）
    Check => "check",
    /// 错误提示（lucide `circle-alert`，v1 `AlertCircle`）
    CircleAlert => "circle-alert",
    /// 关闭（lucide `x`）
    Close => "x",
    /// 向下（lucide `chevron-down`）
    ChevronDown => "chevron-down",
    /// 设置（lucide `settings`）
    Settings => "settings",
    /// 模型（lucide `bot`）
    Bot => "bot",
    /// 发送（lucide `send`）
    Send => "send",
    /// 停止（lucide `square`）
    Square => "square",
    /// 添加图片（lucide `image-plus`）
    ImagePlus => "image-plus",
    /// 思考（lucide `brain`）
    Brain => "brain",
    /// 向右（lucide `chevron-right`）
    ChevronRight => "chevron-right",
    /// 参数（lucide `braces`）
    Braces => "braces",
    /// 完成（lucide `circle-check`，v1 `CheckCircle2`）
    CircleCheck => "circle-check",
    /// 准备中（lucide `circle-dashed`）
    CircleDashed => "circle-dashed",
    /// 执行结果（lucide `file-check-corner`，v1 `FileCheck2`）
    FileCheck => "file-check-corner",
    /// 编辑文件（lucide `file-diff`）
    FileDiff => "file-diff",
    /// 追加文件（lucide `file-output`）
    FileOutput => "file-output",
    /// 覆盖文件（lucide `file-pen-line`）
    FilePenLine => "file-pen-line",
    /// 创建文件（lucide `file-plus-corner`，v1 `FilePlus2`）
    FilePlus => "file-plus-corner",
    /// 读取文件（lucide `file-text`）
    FileText => "file-text",
    /// 浏览目录（lucide `folder-tree`）
    FolderTree => "folder-tree",
    /// 询问用户（lucide `circle-question-mark`，v1 `HelpCircle`）
    CircleHelp => "circle-question-mark",
    /// 执行中（lucide `loader-circle`，v1 `Loader2`）
    LoaderCircle => "loader-circle",
    /// 搜索（lucide `search`）
    Search => "search",
    /// 调用工具（lucide `wrench`）
    Wrench => "wrench",
    /// 失败（lucide `circle-x`，v1 `XCircle`）
    CircleX => "circle-x",
    /// 回到问题（lucide `arrow-up`）
    ArrowUp => "arrow-up",
    /// 流式四角星（Buddy 自绘，v1 `.streaming-next-star`）
    StreamingStar => "streaming-star",
}

/// 方形图标；颜色随父元素或自身的 `text_color`
pub fn icon(name: IconName, size: Pixels) -> Svg {
    svg().path(name.path()).size(size).flex_none()
}

/// Buddy 的资源源（目前只有图标）
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(IconName::ALL.iter().find(|i| i.path() == path).map(|i| Cow::Borrowed(i.bytes())))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(IconName::ALL.iter().map(|i| i.path()).filter(|p| p.starts_with(path)).map(Into::into).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_loads_as_svg() {
        for &i in IconName::ALL {
            let bytes = Assets.load(i.path()).unwrap().expect("资源应存在");
            assert!(std::str::from_utf8(&bytes).unwrap().starts_with("<svg"), "{:?}", i);
        }
        assert!(Assets.load("icons/missing.svg").unwrap().is_none());
        assert_eq!(Assets.list("icons/").unwrap().len(), IconName::ALL.len());
    }
}
