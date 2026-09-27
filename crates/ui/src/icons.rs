//! SVG 图标（硬约束 4：只用 SVG，不用 emoji）
//!
//! 图形取自 v1 所用的 Lucide（`lucide-react`，ISC，见 `THIRD_PARTY_NOTICES.md`），
//! 文件在 `crates/ui/assets/icons/`，编译期嵌入。GPUI 把 SVG 当作蒙版绘制，
//! 颜色取元素的 `text_color`（等价于 v1 的 `stroke="currentColor"`）。
//!
//! 使用前须 `Application::with_assets(buddy_ui::icons::Assets)`，否则图标静默不显示。

use gpui::{AssetSource, Pixels, Result, SharedString, Svg, prelude::*, svg};
use std::borrow::Cow;

/// 图标名（按需增加；每项须在 `assets/icons/` 有同名文件）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconName {
    /// 复制（lucide `copy`）
    Copy,
    /// 对勾（lucide `check`）
    Check,
}

impl IconName {
    const ALL: [IconName; 2] = [IconName::Copy, IconName::Check];

    /// 资源路径
    pub fn path(self) -> &'static str {
        match self {
            IconName::Copy => "icons/copy.svg",
            IconName::Check => "icons/check.svg",
        }
    }

    fn bytes(self) -> &'static [u8] {
        match self {
            IconName::Copy => include_bytes!("../assets/icons/copy.svg"),
            IconName::Check => include_bytes!("../assets/icons/check.svg"),
        }
    }
}

/// 方形图标；颜色随父元素或自身的 `text_color`
pub fn icon(name: IconName, size: Pixels) -> Svg {
    svg().path(name.path()).size(size).flex_none()
}

/// Buddy 的资源源（目前只有图标）
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(IconName::ALL.into_iter().find(|i| i.path() == path).map(|i| Cow::Borrowed(i.bytes())))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(IconName::ALL.into_iter().map(IconName::path).filter(|p| p.starts_with(path)).map(Into::into).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_loads_as_svg() {
        for i in IconName::ALL {
            let bytes = Assets.load(i.path()).unwrap().expect("资源应存在");
            assert!(std::str::from_utf8(&bytes).unwrap().starts_with("<svg"), "{:?}", i);
        }
        assert!(Assets.load("icons/missing.svg").unwrap().is_none());
        assert_eq!(Assets.list("icons/").unwrap().len(), IconName::ALL.len());
    }
}
