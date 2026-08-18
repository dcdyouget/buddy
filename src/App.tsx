/**
 * App.tsx — 应用根组件
 *
 * 职责：
 * 1. 页面路由：根据 uiStore.currentPage 渲染对应页面组件
 * 2. 主题切换：监听 config.theme 切换 dark/light 类名
 * 3. Esc 快捷键：按下 Esc 隐藏窗口（不停止流式生成）
 * 4. 选中文本接收：监听 Rust 后端发送的 selected-text 事件，
 *    将选中文本填入聊天输入框
 * 5. 应用入口动画：frameless 窗口的淡入缩放效果
 */

import { useCallback, useEffect } from 'react';
import { useConfigStore } from '@/stores/configStore';
import { useUIStore } from '@/stores/uiStore';
import { useChatStore } from '@/stores/chatStore';
import { useStreaming } from '@/hooks/useStreaming';
import {
  WINDOW_WILL_HIDE_EVENT,
  WINDOW_WILL_SHOW_EVENT,
} from '@/utils/windowEvents';
import { EmptyPage } from '@/pages/EmptyPage';
import { NoApiKeyPage } from '@/pages/NoApiKeyPage';
import { ChatPage } from '@/pages/ChatPage';
import { SettingsPage } from '@/pages/SettingsPage';
import { SlideInPanel } from '@/components/shared/SlideInPanel';
import { WindowEntrance } from '@/components/shared/WindowEntrance';
import { resizeWindowToPage } from '@/utils/windowResize';

/**
 * 这是一个轻量级的页面路由器，避免引入 react-router 增加包体积
 *
 * 关键：当 currentPage === 'settings' 时，不要卸载背景层，而是
 * 继续渲染上一个真正页面（previousPage），让 SettingsPage 在它
 * 上面叠盖滑入。这样对话/流式状态不会被清空，滑入动画才有意义。
 */
function PageRenderer() {
  const currentPage = useUIStore((s) => s.currentPage);
  const previousPage = useUIStore((s) => s.previousPage);

  // settings 模式下用 previousPage 作为底层页面；previousPage 为 null 时回落 empty
  const effectivePage =
    currentPage === 'settings' ? previousPage ?? 'empty' : currentPage;

  switch (effectivePage) {
    case 'empty':
      return <EmptyPage />;
    case 'noapikey':
      return <NoApiKeyPage />;
    case 'conversation':
    case 'streaming':
      return <ChatPage />;
    default:
      return (
        <div className="flex h-full w-full flex-col items-center justify-center gap-3 text-center">
          <p className="text-sm font-medium" style={{ color: 'var(--color-error)' }}>
            出现错误
          </p>
          <p className="text-xs" style={{ color: 'var(--color-muted)' }}>
            应用遇到了意外状态，请重启应用
          </p>
        </div>
      );
  }
}

/**
 * App 根组件
 * 负责初始化配置、绑定全局快捷键、注册流式事件监听、渲染页面
 */
function App() {
  const loadConfig = useConfigStore((state) => state.loadConfig);
  const configTheme = useConfigStore((state) => state.config?.theme);
  const providerCount = useConfigStore(
    (state) => state.config?.providers.length,
  );
  const selectedModelId = useConfigStore(
    (state) => state.config?.selected_model_id,
  );
  const loadMessages = useChatStore((state) => state.loadMessages);
  const setPage = useUIStore((state) => state.setPage);
  const setThemeReady = useUIStore((state) => state.setThemeReady);
  const currentPage = useUIStore((state) => state.currentPage);
  const entranceMode =
    currentPage === 'empty' || currentPage === 'noapikey'
      ? 'compact'
      : 'expanded';

  const openCompactAfterIdle = useCallback(async () => {
    if (useUIStore.getState().currentPage !== 'empty') {
      await useUIStore.getState().setPage('empty');
    }
  }, []);

  // 注册流式事件监听
  useStreaming();

  // 应用启动时加载配置、历史消息、检测平台
  useEffect(() => {
    loadConfig().then(() => {
      setThemeReady(true);
    });
    loadMessages();

    // 平台检测：用于 CSS 区分 Windows/macOS 样式（如透明效果）
    const userAgent = navigator.userAgent || '';
    const platform = userAgent.includes('Win') ? 'windows' : userAgent.includes('Mac') ? 'macos' : 'linux';
    document.documentElement.setAttribute('data-platform', platform);
  }, []);

  // 监听主题变更
  useEffect(() => {
    if (configTheme) {
      const isDark = configTheme === 'dark';
      document.documentElement.classList.toggle('dark', isDark);
    }
  }, [configTheme]);

  // 配置变化只处理当前可见的聊天页面；设置页中的分步保存不能打断配置流程。
  useEffect(() => {
    if (providerCount === undefined) return;
    const hasValidConfig = providerCount > 0 && !!selectedModelId;
    const cur = useUIStore.getState().currentPage;

    if (!hasValidConfig) {
      // 添加 Provider 会依次保存 provider、模型和默认模型；中间状态尚未完整，
      // 此时必须留在设置页，否则会提前退回紧凑气泡态。
      if (cur !== 'settings' && cur !== 'empty' && cur !== 'noapikey') {
        void setPage('empty');
      }
      return;
    }

    // 配置在「无 Key」页被外部补齐时，直接进入展开的对话页。
    // 设置页内添加完成后的返回由 AddProviderPanel 的 onAdded 统一触发。
    if (cur === 'noapikey') {
      void setPage('conversation');
    }
  }, [providerCount, selectedModelId, setPage]);

  // ── Esc = 隐藏窗口（不停止流式生成）──
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        // 隐藏后 WebView 的 requestAnimationFrame 会暂停；提前通知平滑渲染器
        // 切到后台直写模式，保证模型输出和收尾不会等到窗口再次显示。
        window.dispatchEvent(new Event(WINDOW_WILL_HIDE_EVENT));
        // 动态导入 Tauri API，浏览器环境下会 catch 忽略
        import('@tauri-apps/api/window')
          .then(async ({ getCurrentWindow }) => {
            try {
              await getCurrentWindow().hide();
            } catch {
              // 隐藏失败时恢复内容，避免窗口停留在透明的预备帧。
              window.dispatchEvent(new Event(WINDOW_WILL_SHOW_EVENT));
            }
          })
          .catch(() => {});
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  // ── 菜单栏操作：打开设置 / 同步开机自启状态 ──
  useEffect(() => {
    if (typeof window === 'undefined' || !(window as any).__TAURI_INTERNALS__) {
      return;
    }

    let disposed = false;
    const unlisteners: Array<() => void> = [];

    import('@tauri-apps/api/event').then(async ({ listen }) => {
      const settingsUnlisten = await listen('open-settings', () => {
        const ui = useUIStore.getState();
        if (ui.currentPage !== 'settings') {
          ui.setPage('settings');
        }
      });
      const autoStartUnlisten = await listen<boolean>('auto-start-changed', () => {
        useConfigStore.getState().loadConfig();
      });

      if (disposed) {
        settingsUnlisten();
        autoStartUnlisten();
        return;
      }
      unlisteners.push(settingsUnlisten, autoStartUnlisten);
    }).catch((error) => {
      console.error('[Buddy] 菜单栏事件监听失败:', error);
    });

    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  // ── 监听 Rust 后端发送的 selected-text 事件 ──
  // 用户在其他应用中选中文本后按快捷键，Rust 会发送该事件
  // 前端收到后将文本填入输入框草稿
  useEffect(() => {
    // 仅在 Tauri 环境下执行（浏览器环境无 __TAURI_INTERNALS__，调用 listen 会抛错）
    if (typeof window === 'undefined' || !(window as any).__TAURI_INTERNALS__) {
      return;
    }
    // disposed 守卫：处理 StrictMode 下 effect 运行两次 + listen() 是异步的竞态，
    // 避免在等待期间被卸载后注册一个永远不注销的监听器（监听器泄漏 / 事件重复处理）。
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    import('@tauri-apps/api/event')
      .then(async ({ listen }) => {
        const unlisten = await listen<string>('selected-text', (event) => {
          const text = event.payload.trim();
          if (text) {
            const uiPage = useUIStore.getState().currentPage;
            // 仅在可以输入的状态下设置草稿文本
            if (uiPage === 'empty' || uiPage === 'noapikey' || uiPage === 'conversation') {
              useChatStore.getState().setDraftInput(text);
            }
          }
        });
        if (disposed) {
          unlisten();
          return;
        }
        unlisteners.push(unlisten);
      })
      .catch((error) => {
        console.error('[Buddy] selected-text 事件监听失败:', error);
      });

    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  return (
    <WindowEntrance
      mode={entranceMode}
      onCompactRequested={openCompactAfterIdle}
    >
      {/* 背景页面 */}
      <PageRenderer />

      {/* 设置页覆层：从右滑入（在 ChatPage 之上，不卸载背景） */}
      <SlideInPanel from="right" show={currentPage === 'settings'}>
        {currentPage === 'settings' && (
          <SettingsPage
            onBack={async () => {
              const prev = useUIStore.getState().previousPage ?? 'empty';
              if (prev === 'empty' || prev === 'noapikey') {
                await resizeWindowToPage('conversation');
                await setPage('conversation');
                return;
              }
              await setPage(prev);
            }}
          />
        )}
      </SlideInPanel>
    </WindowEntrance>
  );
}

export default App;
