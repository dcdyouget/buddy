/**
 * useStreaming.ts — 流式事件监听 Hook
 *
 * 在应用根组件中调用一次，注册对 Rust 后端 SSE 流式事件的监听。
 *
 * 监听的四个事件：
 * - stream-token：单个 token 到达，调用 chatStore.appendToken() 追加到消息内容
 * - stream-done：流式正常完成，调用 chatStore.finalizeMessage() 收尾
 * - stream-error：流式出错，根据错误类型可能跳转到 noapikey 页面
 * - stream-cancelled：用户主动停止，正常收尾并切回对话页面
 *
 * 防竞态机制：
 * 使用 epochRef 计数器确保组件重新挂载时，旧的监听器不会被注册。
 * cleanup 函数递增 epoch，所有尚未完成的 import().then() 回调
 * 在拿到结果后会检查 epoch 是否已过期，过期则跳过注册。
 */

import { useEffect, useRef } from 'react';
import { useChatStore } from '@/stores/chatStore';
import { useUIStore } from '@/stores/uiStore';
import type { Message } from '@/types';
import { isBrowser } from '@/utils/mock';

export function useStreaming() {
  // 从 store 中获取需要的操作方法（使用 selector 避免不必要的重渲染）
  const appendToken = useChatStore((s) => s.appendToken);
  const finalizeMessage = useChatStore((s) => s.finalizeMessage);
  const setError = useChatStore((s) => s.setError);
  const setPage = useUIStore((s) => s.setPage);

  // epoch 计数器：每次 effect 重新执行时递增，用于标记"当前有效的世代"
  const epochRef = useRef(0);

  useEffect(() => {
    // 浏览器模式下不使用 Tauri 事件系统
    if (isBrowser) return;

    // 递增 epoch，标记当前 effect 为最新一代
    const epoch = ++epochRef.current;

    // 收集所有 listen 返回的取消监听函数，用于 cleanup 时统一注销
    const unlisteners: Array<() => void> = [];

    // 动态导入 Tauri event API（浏览器环境下不会被执行，因为上面已 return）
    import('@tauri-apps/api/event').then(async ({ listen }) => {
      // 如果在此期间有新的 effect 执行了，跳过当前注册（防竞态）
      if (epoch !== epochRef.current) return;

      // 监听流式 token 事件：每收到一个 token 就追加到消息末尾
      unlisteners.push(await listen<string>('stream-token', (event) => {
        appendToken(event.payload);
      }));

      // 监听流式完成事件：正常结束时收尾并跳转到对话页
      unlisteners.push(await listen<void>('stream-done', () => {
        finalizeMessage();
        setPage('conversation');
      }));

      // 监听流式错误事件：根据错误类型进行不同的处理
      unlisteners.push(await listen<string>('stream-error', (event) => {
        const errorMsg = event.payload;

        // 将错误同步到 chatStore（停止流式状态）和 uiStore（错误分类展示）
        setError(errorMsg);
        useUIStore.getState().setError(errorMsg);

        // 401 / unauthorized：认证失败，跳转到 noapikey 页面提示用户检查 API Key
        if (errorMsg.includes('401') || errorMsg.includes('unauthorized')) {
          setPage('noapikey');
        }
        // 429 / quota：配额超限，在对话中添加一条内联警告消息
        else if (errorMsg.includes('429') || errorMsg.includes('quota')) {
          const chatState = useChatStore.getState();
          const warningMsg: Message = {
            id: 'warn-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 9),
            role: 'assistant',
            content: 'API 配额已用尽，请稍后再试或检查您的账户限额。',
            model_id: null,
            created_at: Math.floor(Date.now() / 1000),
          };
          chatState.setMessages([...chatState.messages, warningMsg]);
          setPage('conversation');
        }
        // 5xx / server_error：服务器错误，追加重试提示消息
        else if (errorMsg.includes('5xx') || errorMsg.includes('server_error') || errorMsg.includes('500')) {
          const chatState = useChatStore.getState();
          const retryMsg: Message = {
            id: 'err-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 9),
            role: 'assistant',
            content: '请求失败，请重试',
            model_id: null,
            created_at: Math.floor(Date.now() / 1000),
          };
          chatState.setMessages([...chatState.messages, retryMsg]);
          setPage('conversation');
        }
        // network / timeout：网络错误，追加重试提示消息
        else if (errorMsg.includes('network') || errorMsg.includes('timeout')) {
          const chatState = useChatStore.getState();
          const retryMsg: Message = {
            id: 'err-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 9),
            role: 'assistant',
            content: '网络错误，请重试',
            model_id: null,
            created_at: Math.floor(Date.now() / 1000),
          };
          chatState.setMessages([...chatState.messages, retryMsg]);
          setPage('conversation');
        }
      }));

      // 监听用户取消事件：正常收尾并跳转回对话页面
      unlisteners.push(await listen<void>('stream-cancelled', () => {
        finalizeMessage();
        setPage('conversation');
      }));
    });

    // cleanup：递增 epoch 使旧 effect 失效，并注销所有已注册的 Tauri 事件监听器
    return () => {
      epochRef.current++; // 使旧 effect 中仍在等待的 .then() 回调跳过注册
      unlisteners.forEach((fn) => fn()); // 注销所有已注册的监听器
    };
  }, []);
}
