/**
 * uiStore.ts — UI 状态管理
 *
 * 管理用户界面相关的全局状态，包括：
 * - 当前页面路由（currentPage / previousPage）
 * - 错误信息和错误类型分类
 * - 模型下拉菜单开关状态
 * - 主题就绪状态
 *
 * 页面切换时会自动调用 resizeWindowForPage() 调整窗口尺寸。
 * 错误信息会自动分类为 401/429/5xx/network 四种类型，便于 UI 展示对应的友好提示。
 */

import { create } from 'zustand';
import type { PageState } from '@/types';
import { resizeWindowForPage } from '@/utils/windowResize';

/** UIStore 状态和操作定义 */
interface UIState {
  currentPage: PageState;            // 当前页面
  previousPage: PageState | null;    // 上一个页面（用于返回导航）
  error: string | null;              // 错误消息文本
  errorType: '401' | '429' | '5xx' | 'network' | null; // 错误类型分类
  isModelDropdownOpen: boolean;      // 模型选择下拉菜单是否展开
  themeReady: boolean;               // 主题是否已加载就绪（避免闪烁）

  // ── 操作 ──
  setPage: (page: PageState) => void;         // 切换页面
  goBack: () => void;                          // 返回上一页
  setError: (error: string) => void;           // 设置错误（自动分类）
  clearError: () => void;                      // 清除错误
  setModelDropdownOpen: (open: boolean) => void; // 设置下拉菜单开关
  toggleModelDropdown: () => void;             // 切换下拉菜单开关
  setThemeReady: (ready: boolean) => void;     // 设置主题就绪
}

export const useUIStore = create<UIState>((set, get) => ({
  currentPage: 'empty',
  previousPage: null,
  error: null,
  errorType: null,
  isModelDropdownOpen: false,
  themeReady: false,

  /**
   * 切换到指定页面
   * - 记录当前页面为 previousPage（支持返回导航）
   * - 清除错误状态
   * - 自动调整窗口尺寸
   */
  setPage: (page: PageState) => {
    const { currentPage } = get();
    console.log('[Buddy] setPage called:', currentPage, '→', page);
    // 在 set 之前调用 resizeWindowForPage，使其能够读取到旧页面状态
    resizeWindowForPage(currentPage, page);
    set({ currentPage: page, previousPage: currentPage, error: null, errorType: null });
  },

  /** 返回上一个页面（如果有的话） */
  goBack: () => {
    const { previousPage } = get();
    if (previousPage) {
      set({
        currentPage: previousPage,
        previousPage: null,
        error: null,
        errorType: null,
      });
    }
  },

  /**
   * 设置错误信息并自动分类
   * 根据错误文本内容匹配对应的错误类型：
   * - 401：认证失败（API Key 无效）
   * - 429：配额超限
   * - 5xx：服务器错误
   * - network：网络连接问题
   */
  setError: (error: string) => {
    let errorType: UIState['errorType'] = null;
    if (error.includes('401') || error.includes('unauthorized')) {
      errorType = '401';
    } else if (error.includes('429') || error.includes('quota')) {
      errorType = '429';
    } else if (error.includes('5') && (error.includes('server') || error.includes('500'))) {
      errorType = '5xx';
    } else if (error.includes('network') || error.includes('timeout')) {
      errorType = 'network';
    }
    set({ error, errorType });
  },

  /** 清除错误状态 */
  clearError: () => {
    set({ error: null, errorType: null });
  },

  /** 设置模型下拉菜单的开关状态 */
  setModelDropdownOpen: (open: boolean) => {
    set({ isModelDropdownOpen: open });
  },

  /** 切换模型下拉菜单的展开/收起 */
  toggleModelDropdown: () => {
    set((s) => ({ isModelDropdownOpen: !s.isModelDropdownOpen }));
  },

  /** 标记主题已加载就绪，用于控制 UI 的初始渲染时机，避免主题闪烁 */
  setThemeReady: (ready: boolean) => {
    set({ themeReady: ready });
  },
}));
