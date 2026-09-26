/**
 * useStreaming.ts — 流式事件监听 Hook
 *
 * 在应用根组件中调用一次，注册对 Rust 后端流式事件的监听。
 *
 * 事件协议（v2.0 统一格式）：
 * - stream-event：统一事件，携带 JSON 化的 StreamEvent
 *   - start → 流式开始
 *   - text_start/delta/end → 文本块生命周期
 *   - thinking_start/delta/end → 思考块生命周期
 *   - tool_call_start/delta/end → 工具调用生命周期
 *   - tool_executing/tool_result → 工具执行状态
 *   - tool_approval_required → 需要用户审批
 *   - turn_end → 本轮结束
 *   - done → 流式正常完成 → chatStore.handleStreamDone()
 *   - error → 流式错误/取消 → chatStore.handleStreamError()
 *
 * 防竞态机制：
 * 使用 epochRef 计数器确保组件重新挂载时，旧的监听器不会被注册。
 */

import { useEffect, useRef } from 'react';
import { useChatStore } from '@/stores/chatStore';
import { useUIStore } from '@/stores/uiStore';
import type { Message, StreamEvent } from '@/types';
import { isBrowser } from '@/utils/mock';

/** 审批 modal 状态栈：前端用 zustand style 管理 */
let approvalResolve: ((approved: boolean, approveAll: boolean) => void) | null = null;
let approvalId: string | null = null;

/** 从外部触发审批（供 ApprovalModal 组件调用） */
export function resolveApproval(approved: boolean, approveAll: boolean) {
  if (approvalResolve && approvalId) {
    approvalResolve(approved, approveAll);
    approvalResolve = null;
    approvalId = null;
  }
}

export function useStreaming() {
  const queueStreamEvent = useChatStore((s) => s.queueStreamEvent);
  const toolApproval = useChatStore((s) => s.toolApproval);
  const handleStreamDone = useChatStore((s) => s.handleStreamDone);
  const handleStreamError = useChatStore((s) => s.handleStreamError);
  const saveMessage = useChatStore((s) => s.saveMessage);
  const setPage = useUIStore((s) => s.setPage);

  const epochRef = useRef(0);

  // 审批事件也先进入 FIFO；只有前序正文和 tool_call 已落到对应气泡后才显示弹窗。
  useEffect(() => {
    if (isBrowser || !toolApproval || approvalId === toolApproval.id) return;

    approvalId = toolApproval.id;
    import('@tauri-apps/api/core').then(async ({ invoke }) => {
      const decision = await new Promise<{
        approved: boolean;
        approveAll: boolean;
      }>((resolve) => {
        approvalResolve = (approved, approveAll) => resolve({ approved, approveAll });
      });
      await invoke('approve_tool_call', {
        id: toolApproval.id,
        approved: decision.approved,
        approveAll: decision.approveAll,
      }).catch(() => {});
      approvalId = null;
    });
  }, [toolApproval]);

  useEffect(() => {
    if (isBrowser) return;

    const epoch = ++epochRef.current;
    const unlisteners: Array<() => void> = [];

    import('@tauri-apps/api/event').then(async ({ listen }) => {
      const unlisten = await listen<StreamEvent>('stream-event', (event) => {
          const e = event.payload;
          switch (e.event) {
            case 'start':
              break;

            case 'text_start':
              queueStreamEvent({ event: 'text_start', contentIndex: e.content_index });
              break;

            case 'text_delta':
              queueStreamEvent({
                event: 'text_delta',
                contentIndex: e.content_index,
                delta: e.delta,
              });
              break;

            case 'text_end':
              queueStreamEvent({
                event: 'text_end',
                contentIndex: e.content_index,
                content: e.content,
              });
              break;

            case 'thinking_start':
              queueStreamEvent({ event: 'thinking_start', contentIndex: e.content_index });
              break;

            case 'thinking_delta':
              queueStreamEvent({
                event: 'thinking_delta',
                contentIndex: e.content_index,
                delta: e.delta,
              });
              break;

            case 'thinking_end':
              queueStreamEvent({
                event: 'thinking_end',
                contentIndex: e.content_index,
                content: e.content,
              });
              break;

            // ── Tool 事件 ────────────────────────────────
            case 'tool_call_start':
              queueStreamEvent({
                event: 'tool_call_start',
                id: e.id,
                name: e.name,
                contentIndex: e.content_index,
              });
              break;

            case 'tool_call_delta':
              queueStreamEvent({
                event: 'tool_call_delta',
                id: e.id,
                argumentsDelta: e.arguments_delta,
              });
              break;

            case 'tool_call_end':
              queueStreamEvent({
                event: 'tool_call_end',
                id: e.id,
                name: e.name,
                arguments: e.arguments,
              });
              break;

            case 'tool_executing':
              queueStreamEvent({ event: 'tool_executing', id: e.id, name: e.name });
              break;

            case 'tool_result':
              queueStreamEvent({
                event: 'tool_result',
                id: e.id,
                name: e.name,
                content: e.content,
                images: e.images ?? [],
                isError: e.is_error,
              });
              break;

            case 'tool_approval_required':
              queueStreamEvent({
                event: 'tool_approval_required',
                id: e.id,
                name: e.name,
                arguments: e.arguments,
                reason: e.reason,
              });
              break;

            case 'tool_question_required':
              queueStreamEvent({
                event: 'tool_question_required',
                id: e.id,
                question: e.question,
                options: e.options,
                multiSelect: e.multi_select,
                header: e.header,
              });
              break;

            case 'turn_end':
              queueStreamEvent({
                event: 'turn_end',
                toolCallsPending: e.tool_calls_pending,
              });
              break;

            case 'done':
              handleStreamDone();
              setPage('conversation');
              break;

            case 'error': {
              if (e.reason === 'aborted') {
                // 若正停在审批等待中，先释放前端 Promise；后端取消后会忽略这次拒绝。
                resolveApproval(false, false);
                handleStreamDone();
                setPage('conversation');
                break;
              }

              handleStreamError(e.reason, e.message);
              useUIStore.getState().setError(e.message);

              if (e.message.includes('401') || e.message.includes('unauthorized')) {
                setPage('noapikey');
              } else if (e.message.includes('429') || e.message.includes('quota')) {
                const chatState = useChatStore.getState();
                const warningMsg: Message = {
                  id: 'warn-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 9),
                  role: 'assistant',
                  content: 'API 配额已用尽，请稍后再试或检查您的账户限额。',
                  model_id: null,
                  created_at: Math.floor(Date.now() / 1000),
                };
                chatState.setMessages([...chatState.messages, warningMsg]);
                saveMessage(warningMsg);
                setPage('conversation');
              } else if (e.message.includes('HTTP 5') || e.message.includes('server_error')) {
                const chatState = useChatStore.getState();
                const retryMsg: Message = {
                  id: 'err-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 9),
                  role: 'assistant',
                  content: e.message,
                  model_id: null,
                  created_at: Math.floor(Date.now() / 1000),
                };
                chatState.setMessages([...chatState.messages, retryMsg]);
                saveMessage(retryMsg);
                setPage('conversation');
              } else if (e.message.includes('网络错误') || e.message.includes('network') || e.message.includes('timeout')) {
                const chatState = useChatStore.getState();
                const retryMsg: Message = {
                  id: 'err-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 9),
                  role: 'assistant',
                  content: '网络错误，请重试',
                  model_id: null,
                  created_at: Math.floor(Date.now() / 1000),
                };
                chatState.setMessages([...chatState.messages, retryMsg]);
                saveMessage(retryMsg);
                setPage('conversation');
              } else {
                setPage('conversation');
              }
              break;
            }
          }
        });

      // 竞态防御：组件可能在 `await listen()` 期间被卸载。
      // epoch 在 await 之前已校验一次；若卸载发生在 await 期间，cleanup 已经跑完
      // （unlisteners 不再会被注销），这里再次校验 epoch，不一致则立即注销刚注册的监听器。
      if (epoch !== epochRef.current) {
        unlisten();
        return;
      }
      unlisteners.push(unlisten);
    });

    return () => {
      epochRef.current++;
      unlisteners.forEach((fn) => fn());
    };
  }, []);
}
