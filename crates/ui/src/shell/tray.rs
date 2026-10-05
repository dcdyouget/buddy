//! 系统托盘图标、菜单与 GPUI 事件桥（S07-09）。
//!
//! `tray-icon` 的平台对象必须由 GPUI 主线程持有；本模块不直接读取或修改
//! `App`，只把原生回调转换成 [`MenuAction`]。调用方负责在 GPUI 任务中消费
//! 接收端，并调用 runtime / Router。

use image::ImageFormat;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

const TRAY_ID: &str = "buddy-tray";
const SETTINGS_ID: &str = "settings";
const AUTOSTART_ID: &str = "autostart";
const QUIT_ID: &str = "quit";
const DEFAULT_TOOLTIP: &str = "Buddy";
const AUTOSTART_ERROR_TOOLTIP: &str = "Buddy：开机自启不可用";
const AUTOSTART_BUSY_TOOLTIP: &str = "Buddy：正在更新开机自启";

// v1 已验收的 2x 菜单栏图标；模板模式由 macOS 根据浅深色自动着色。
const TRAY_ICON_PNG: &[u8] = include_bytes!("../../assets/tray@2x.png");

/// 托盘回调交给 GPUI 侧处理的动作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    /// 左键点击托盘图标，显示并聚焦主窗口。
    Show,
    /// 打开设置页。
    Settings,
    /// 切换系统开机自启。
    ToggleAutostart,
    /// 退出进程。
    Quit,
}

/// 主线程拥有的托盘服务。
///
/// `TrayIcon`、菜单项及其平台对象都不能跨线程移动；调用方应把此类型存
/// 在 GPUI 的主线程 Entity 中，直到应用退出。菜单事件接收端由构造函数
/// 返回，消费动作的生命周期由调用方管理。
pub struct TrayService {
    _icon: TrayIcon,
    autostart: CheckMenuItem,
    autostart_available: bool,
    autostart_busy: bool,
}

impl TrayService {
    /// 创建 v1 菜单栏图标与菜单，并返回原生动作接收端。
    ///
    /// `autostart` 必须是系统实际查询结果。查询失败会禁用菜单项并显示
    /// 中文错误提示，不会把配置文件中的旧值伪装成系统已启用。
    pub fn new(
        autostart: Result<bool, String>,
    ) -> Result<(Self, UnboundedReceiver<MenuAction>), String> {
        let (sender, receiver) = unbounded_channel();
        let (autostart_enabled, autostart_available) = match autostart {
            Ok(enabled) => (enabled, true),
            Err(error) => {
                log::warn!("[tray] 查询开机自启状态失败：{error}");
                (false, false)
            }
        };

        let settings = MenuItem::with_id(SETTINGS_ID, "设置…", true, None);
        let autostart_item = CheckMenuItem::with_id(
            AUTOSTART_ID,
            "开机自启",
            autostart_available,
            autostart_enabled,
            None,
        );
        let separator = PredefinedMenuItem::separator();
        let quit = MenuItem::with_id(QUIT_ID, "退出", true, None);
        let menu = Menu::new();
        menu.append(&settings)
            .map_err(|error| format!("添加托盘设置菜单失败：{error}"))?;
        menu.append(&autostart_item)
            .map_err(|error| format!("添加托盘自启菜单失败：{error}"))?;
        menu.append(&separator)
            .map_err(|error| format!("添加托盘分隔线失败：{error}"))?;
        menu.append(&quit)
            .map_err(|error| format!("添加托盘退出菜单失败：{error}"))?;

        let rgba = image::load_from_memory_with_format(TRAY_ICON_PNG, ImageFormat::Png)
            .map_err(|error| format!("读取托盘图标失败：{error}"))?
            .to_rgba8();
        let (width, height) = rgba.dimensions();
        let icon = Icon::from_rgba(rgba.into_raw(), width, height)
            .map_err(|error| format!("创建托盘图标失败：{error}"))?;

        let tray = TrayIconBuilder::new()
            .with_id(TRAY_ID)
            .with_menu(Box::new(menu))
            .with_icon(icon)
            .with_icon_as_template(true)
            .with_title("Buddy")
            .with_tooltip(DEFAULT_TOOLTIP)
            // v1 左键直接显示窗口；菜单保留给右键。
            .with_menu_on_left_click(false)
            .with_menu_on_right_click(true)
            .build()
            .map_err(|error| format!("创建系统托盘失败：{error}"))?;

        install_event_bridge(tray.id().clone(), sender)?;

        let service = Self {
            _icon: tray,
            autostart: autostart_item,
            autostart_available,
            autostart_busy: false,
        };
        service.refresh_tooltip();
        Ok((service, receiver))
    }

    /// 更新系统实际返回的开机自启状态。
    ///
    /// 失败时菜单项保持禁用，勾选清除，避免把配置值当作 OS 状态。
    pub fn set_autostart(&mut self, state: Result<bool, String>) {
        match state {
            Ok(enabled) => {
                self.autostart_available = true;
                self.autostart.set_checked(enabled);
            }
            Err(error) => {
                log::warn!("[tray] 更新开机自启状态失败：{error}");
                self.autostart_available = false;
                self.autostart.set_checked(false);
            }
        }
        self.refresh_menu_state();
    }

    /// 暂时禁用或恢复开机自启菜单项，覆盖系统写入期间的竞态点击。
    pub fn set_autostart_busy(&mut self, busy: bool) {
        self.autostart_busy = busy;
        self.refresh_menu_state();
    }

    fn refresh_menu_state(&self) {
        self.autostart
            .set_enabled(self.autostart_available && !self.autostart_busy);
        self.refresh_tooltip();
    }

    fn refresh_tooltip(&self) {
        let tooltip = if self.autostart_busy {
            AUTOSTART_BUSY_TOOLTIP
        } else if !self.autostart_available {
            AUTOSTART_ERROR_TOOLTIP
        } else {
            DEFAULT_TOOLTIP
        };
        if let Err(error) = self._icon.set_tooltip(Some(tooltip)) {
            log::warn!("[tray] 更新托盘提示失败：{error}");
        }
    }
}

/// 安装一次性原生事件桥；回调只向 GPUI 侧发送动作，不接触 `App`。
fn install_event_bridge(
    tray_id: tray_icon::TrayIconId,
    sender: UnboundedSender<MenuAction>,
) -> Result<(), String> {
    let menu_sender = sender.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if let Some(action) = menu_action(event.id().0.as_str()) {
            let _ = menu_sender.send(action);
        }
    }));

    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let Some(action) = tray_action(&event, &tray_id) {
            let _ = sender.send(action);
        }
    }));
    Ok(())
}

fn menu_action(id: &str) -> Option<MenuAction> {
    match id {
        SETTINGS_ID => Some(MenuAction::Settings),
        AUTOSTART_ID => Some(MenuAction::ToggleAutostart),
        QUIT_ID => Some(MenuAction::Quit),
        _ => None,
    }
}

fn tray_action(event: &TrayIconEvent, tray_id: &tray_icon::TrayIconId) -> Option<MenuAction> {
    match event {
        TrayIconEvent::Click {
            id,
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } if id == tray_id => Some(MenuAction::Show),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click(id: &str, button: MouseButton, button_state: MouseButtonState) -> TrayIconEvent {
        TrayIconEvent::Click {
            id: tray_icon::TrayIconId::new(id),
            position: Default::default(),
            rect: Default::default(),
            button,
            button_state,
        }
    }

    #[test]
    fn maps_menu_actions_to_v1_items() {
        assert_eq!(menu_action(SETTINGS_ID), Some(MenuAction::Settings));
        assert_eq!(menu_action(AUTOSTART_ID), Some(MenuAction::ToggleAutostart));
        assert_eq!(menu_action(QUIT_ID), Some(MenuAction::Quit));
        assert_eq!(menu_action("unknown"), None);
    }

    #[test]
    fn only_left_button_release_on_our_tray_shows_window() {
        let tray_id = tray_icon::TrayIconId::new(TRAY_ID);
        assert_eq!(
            tray_action(
                &click(TRAY_ID, MouseButton::Left, MouseButtonState::Up),
                &tray_id,
            ),
            Some(MenuAction::Show)
        );
        assert_eq!(
            tray_action(
                &click(TRAY_ID, MouseButton::Left, MouseButtonState::Down),
                &tray_id,
            ),
            None
        );
        assert_eq!(
            tray_action(
                &click(TRAY_ID, MouseButton::Right, MouseButtonState::Up),
                &tray_id,
            ),
            None
        );
        assert_eq!(
            tray_action(
                &click("other-tray", MouseButton::Left, MouseButtonState::Up),
                &tray_id,
            ),
            None
        );
    }
}
