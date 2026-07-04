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

import { useEffect } from 'react';
import { motion } from 'framer-motion';
import { useConfigStore } from '@/stores/configStore';
import { useUIStore } from '@/stores/uiStore';
import { useChatStore } from '@/stores/chatStore';
import { useStreaming } from '@/hooks/useStreaming';
import { EmptyPage } from '@/pages/EmptyPage';
import { NoApiKeyPage } from '@/pages/NoApiKeyPage';
import { ChatPage } from '@/pages/ChatPage';
import { SettingsPage } from '@/pages/SettingsPage';
import { SlideInPanel } from '@/components/shared/SlideInPanel';

/**
 * 页面渲染器 — 根据 currentPage 状态返回对应页面组件
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
  const { loadConfig, config } = useConfigStore();
  const { loadMessages } = useChatStore();
  const { setPage, setThemeReady, currentPage } = useUIStore();

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
    if (config?.theme) {
      const isDark = config.theme === 'dark';
      document.documentElement.classList.toggle('dark', isDark);
    }
  }, [config?.theme]);

  // 根据配置决定入口页面（双向：缺失时提示，具备时回对话页）
  useEffect(() => {
    if (!config) return;
    const hasValidConfig = config.providers.length > 0 && !!config.selected_model_id;
    if (!hasValidConfig) {
      // 配置缺失 → 回到空态/提示页
      setPage('empty');
    } else {
      // 配置已具备 → 如果当前正显示「无 Key」页，自动跳回对话页
      const cur = useUIStore.getState().currentPage;
      if (cur === 'noapikey' || cur === 'empty') {
        setPage('empty');
      }
    }
  }, [config?.providers.length, config?.selected_model_id]);

  // ── Esc = 隐藏窗口（不停止流式生成）──
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        // 动态导入 Tauri API，浏览器环境下会 catch 忽略
        import('@tauri-apps/api/window').then(({ getCurrentWindow }) => {
          getCurrentWindow().hide();
        }).catch(() => {});
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  // ── 监听 Rust 后端发送的 selected-text 事件 ──
  // 用户在其他应用中选中文本后按快捷键，Rust 会发送该事件
  // 前端收到后将文本填入输入框草稿
  useEffect(() => {
    import('@tauri-apps/api/event').then(({ listen }) => {
      listen<string>('selected-text', (event) => {
        const text = event.payload.trim();
        if (text) {
          const uiPage = useUIStore.getState().currentPage;
          // 仅在可以输入的状态下设置草稿文本
          if (uiPage === 'empty' || uiPage === 'noapikey' || uiPage === 'conversation') {
            useChatStore.getState().setDraftInput(text);
          }
        }
      });
    });
  }, []);

  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.95 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{
        duration: 0.3,
        ease: [0.34, 1.56, 0.64, 1],
      }}
      style={{
        width: '100vw',
        height: '100vh',
        overflow: 'hidden',
        background: 'transparent',
        position: 'relative',
      }}
    >
      {/* 背景页面 */}
      <PageRenderer />

      {/* 设置页覆层：从右滑入（在 ChatPage 之上，不卸载背景） */}
      <SlideInPanel from="right" show={currentPage === 'settings'}>
        {currentPage === 'settings' && (
          <SettingsPage
            onBack={() => {
              const prev = useUIStore.getState().previousPage ?? 'empty';
              setPage(prev);
            }}
          />
        )}
      </SlideInPanel>
    </motion.div>
  );
}

export default App;
