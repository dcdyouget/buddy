//! SVG 图标（界面只用 SVG 图标，不用 emoji）
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
/// 新增 Lucide 图标：从 lucide.dev 下载对应 SVG（24×24、`stroke="currentColor"`）放入 `assets/icons/`。
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
    /// 浅色外观（lucide `sun`）
    Sun => "sun",
    /// 深色外观（lucide `moon`）
    Moon => "moon",
    /// 模型（lucide `bot`）
    Bot => "bot",
    /// 发送（lucide `send`）
    Send => "send",
    /// 停止（lucide `square`）
    Square => "square",
    /// 添加图片（lucide `image-plus`）
    ImagePlus => "image-plus",
    /// 添加模型（lucide `plus`）
    Plus => "plus",
    /// 显示 Key（lucide `eye`）
    Eye => "eye",
    /// 隐藏 Key（lucide `eye-off`）
    EyeOff => "eye-off",
    /// 生成图片（lucide `image`）
    Image => "image",
    /// 下载图片（lucide `download`）
    Download => "download",
    /// 图片缺失（lucide `image-off`）
    ImageOff => "image-off",
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
    /// 返回设置前的页面（lucide `arrow-left`）
    ArrowLeft => "arrow-left",
    /// 设置 API Key（lucide `key-round`）
    KeyRound => "key-round",
    /// 展开到对话（lucide `chevron-up`）
    ChevronUp => "chevron-up",
    /// 外部链接（lucide `external-link`）
    ExternalLink => "external-link",
    /// 自定义回答前缀（lucide `corner-down-right`）
    CornerDownRight => "corner-down-right",
    /// 工具审批（lucide `shield`）
    Shield => "shield",
    /// 允许（lucide `shield-check`）
    ShieldCheck => "shield-check",
    /// 拒绝（lucide `shield-x`）
    ShieldX => "shield-x",
    /// 检查更新（lucide `refresh-cw`）
    RefreshCw => "refresh-cw",
    /// 流式四角星（Buddy 自绘，v1 `.streaming-next-star`）
    StreamingStar => "streaming-star",
}

/// 方形图标；颜色随父元素或自身的 `text_color`
pub fn icon(name: IconName, size: Pixels) -> Icon {
    Icon(svg().path(name.path()).size(size).flex_none())
}

/// 图标元素：包一层 [`Svg`]，在绘制时取祖先的文字颜色。
///
/// **GPUI 的 `Svg` 不继承父元素的 `text_color`**（它只读自己的样式，`gpui/src/elements/svg.rs`），
/// 直接用 `svg()` 时，放在带颜色的按钮里的图标会整个不画出来，悬停变色也不会生效（目检 #16 / #17 发现：
/// 输入区的设置、模型、发送图标与「复制」图标全部缺失，`Window::render_to_image` 证实）。
/// 这里在绘制时读取 `window.text_style().color`（此时祖先的悬停 / 聚焦样式已入栈），自身显式设置的颜色优先。
pub struct Icon(Svg);

impl Icon {
    /// 变换（缩放 / 旋转），同 [`Svg::with_transformation`]
    pub fn with_transformation(self, transformation: gpui::Transformation) -> Self {
        Icon(self.0.with_transformation(transformation))
    }
}

impl Styled for Icon {
    fn style(&mut self) -> &mut gpui::StyleRefinement {
        self.0.style()
    }
}

impl IntoElement for Icon {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl gpui::Element for Icon {
    type RequestLayoutState = <Svg as gpui::Element>::RequestLayoutState;
    type PrepaintState = <Svg as gpui::Element>::PrepaintState;

    fn id(&self) -> Option<gpui::ElementId> {
        gpui::Element::id(&self.0)
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        gpui::Element::source_location(&self.0)
    }

    fn request_layout(
        &mut self,
        id: Option<&gpui::GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        self.0.request_layout(id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&gpui::GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> Self::PrepaintState {
        self.0.prepaint(id, inspector_id, bounds, state, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&gpui::GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        let style = self.0.style();
        if style.text.color.is_none() {
            style.text.color = Some(window.text_style().color);
        }
        self.0.paint(
            id,
            inspector_id,
            bounds,
            request_layout,
            prepaint,
            window,
            cx,
        )
    }
}

/// Buddy 的资源源（目前只有图标）
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(IconName::ALL
            .iter()
            .find(|i| i.path() == path)
            .map(|i| Cow::Borrowed(i.bytes())))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(IconName::ALL
            .iter()
            .map(|i| i.path())
            .filter(|p| p.starts_with(path))
            .map(Into::into)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_loads_as_svg() {
        for &i in IconName::ALL {
            let bytes = Assets.load(i.path()).unwrap().expect("资源应存在");
            assert!(
                std::str::from_utf8(&bytes).unwrap().starts_with("<svg"),
                "{:?}",
                i
            );
        }
        assert!(Assets.load("icons/missing.svg").unwrap().is_none());
        assert_eq!(Assets.list("icons/").unwrap().len(), IconName::ALL.len());
    }

    /// 每个图标在小尺寸下都要有可见的笔画（曾发生「设置」等图标缺笔画 / 几乎不可见而无人察觉）
    #[test]
    fn every_icon_has_visible_ink_at_small_sizes() {
        use gpui::{DevicePixels, Size, SvgRenderer, SvgSize};
        let renderer = SvgRenderer::new(std::sync::Arc::new(Assets));
        for &i in IconName::ALL {
            let bytes = Assets.load(i.path()).unwrap().unwrap();
            let parsed = renderer.parse_svg(&bytes).expect("SVG 应可解析");
            for px in [13, 14, 16] {
                // 2x 屏
                let dev = DevicePixels(px * 2);
                let image = renderer
                    .render_parsed(&parsed, SvgSize::ExactSize(Size::new(dev, dev)))
                    .expect("应可渲染");
                let ink = image
                    .as_bytes(0)
                    .unwrap()
                    .chunks_exact(4)
                    .filter(|p| p[3] > 0)
                    .count();
                let total = (px * 2 * px * 2) as usize;
                eprintln!(
                    "{:?} @{px}px: 覆盖 {:.1}%",
                    i,
                    ink as f64 * 100.0 / total as f64
                );
            }
        }
    }
}
