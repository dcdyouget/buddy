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
  const handleTextStart = useChatStore((s) => s.handleTextStart);
  const handleTextEnd = useChatStore((s) => s.handleTextEnd);
  const feedTextDelta = useChatStore((s) => s.feedTextDelta);
  const handleThinkingStart = useChatStore((s) => s.handleThinkingStart);
  const handleThinkingDelta = useChatStore((s) => s.handleThinkingDelta);
  const handleThinkingEnd = useChatStore((s) => s.handleThinkingEnd);
  const handleToolCallStart = useChatStore((s) => s.handleToolCallStart);
  const handleToolCallDelta = useChatStore((s) => s.handleToolCallDelta);
  const handleToolCallEnd = useChatStore((s) => s.handleToolCallEnd);
  const handleToolExecuting = useChatStore((s) => s.handleToolExecuting);
  const handleToolResult = useChatStore((s) => s.handleToolResult);
  const handleToolApprovalRequired = useChatStore((s) => s.handleToolApprovalRequired);
  const setPendingQuestion = useChatStore((s) => s.setPendingQuestion);
  const handleStreamDone = useChatStore((s) => s.handleStreamDone);
  const handleStreamError = useChatStore((s) => s.handleStreamError);
  const saveMessage = useChatStore((s) => s.saveMessage);
  const setPage = useUIStore((s) => s.setPage);

  const epochRef = useRef(0);

  useEffect(() => {
    if (isBrowser) return;

    const epoch = ++epochRef.current;
    const unlisteners: Array<() => void> = [];

    import('@tauri-apps/api/event').then(async ({ listen }) => {
      if (epoch !== epochRef.current) return;

      unlisteners.push(
        await listen<StreamEvent>('stream-event', (event) => {
          const e = event.payload;
          switch (e.event) {
            case 'start':
              break;

            case 'text_start':
              handleTextStart(e.content_index);
              break;

            case 'text_delta':
              // P9: 入队缓冲 → rAF 循环逐字渲染，避免突发 SSE chunk 导致视觉卡顿
              feedTextDelta(e.delta);
              break;

            case 'text_end':
              handleTextEnd(e.content_index, e.content);
              break;

            case 'thinking_start':
              handleThinkingStart(e.content_index);
              break;

            case 'thinking_delta':
              handleThinkingDelta(e.content_index, e.delta);
              break;

            case 'thinking_end':
              handleThinkingEnd(e.content_index, e.content);
              break;

            // ── Tool 事件 ────────────────────────────────
            case 'tool_call_start':
              handleToolCallStart(e.id, e.name, e.content_index);
              break;

            case 'tool_call_delta':
              handleToolCallDelta(e.id, e.arguments_delta);
              break;

            case 'tool_call_end':
              handleToolCallEnd(e.id, e.name, e.arguments);
              break;

            case 'tool_executing':
              handleToolExecuting(e.id, e.name);
              break;

            case 'tool_result':
              handleToolResult(e.id, e.name, e.content, e.is_error);
              break;

            case 'tool_approval_required': {
              handleToolApprovalRequired(e.id, e.name, e.arguments, e.reason);
              // 阻塞等待用户审批
              import('@tauri-apps/api/core').then(async ({ invoke }) => {
                const doApprove = await new Promise<{ approved: boolean; approveAll: boolean }>((resolve) => {
                  approvalResolve = (approved: boolean, approveAll: boolean) =>
                    resolve({ approved, approveAll });
                  approvalId = e.id;
                });
                await invoke('approve_tool_call', {
                  id: e.id,
                  approved: doApprove.approved,
                  approveAll: doApprove.approveAll,
                }).catch(() => {});
                approvalId = null;
              });
              break;
            }

            case 'tool_question_required': {
              // 模型调用了 ask_user tool:把问题挂到 chatStore,
              // QuestionModal 会自动弹出,answer 由 answerPendingQuestion 发起
              setPendingQuestion({
                id: e.id,
                question: e.question,
                options: e.options,
                multiSelect: e.multi_select,
                header: e.header,
              });
              break;
            }

            case 'turn_end':
              // model 本轮结束，tool_calls_pending 非零时后端还在循环
              break;

            case 'done':
              handleStreamDone();
              setPage('conversation');
              break;

            case 'error': {
              if (e.reason === 'aborted') {
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
        }),
      );
    });

    return () => {
      epochRef.current++;
      unlisteners.forEach((fn) => fn());
    };
  }, []);
}
