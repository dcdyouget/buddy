// @vitest-environment jsdom

import { beforeEach, describe, expect, it } from 'vitest';

import type { Message } from '@/types';
import { useChatStore } from './chatStore';

function assistantMessage(): Message {
  return {
    id: 'assistant-test',
    role: 'assistant',
    content: '',
    blocks: [],
    model_id: 'test-model',
    created_at: 0,
  };
}

beforeEach(() => {
  useChatStore.setState({
    messages: [assistantMessage()],
    isStreaming: true,
    streamingTokens: 0,
    streamingBlocks: [],
    pendingTextBuffer: '',
    pendingTextEnd: null,
    streamDonePending: false,
    activeToolCalls: {},
    pendingQuestion: null,
    toolApproval: null,
    error: null,
  });
});

describe('chatStore think 流式解析', () => {
  it('将内容出现前的工具调用记录在第一个 block 之前', () => {
    useChatStore
      .getState()
      .handleToolCallStart('call-before-content', 'websearch', 0);

    expect(
      useChatStore.getState().activeToolCalls['call-before-content']
        .insertAfterBlockIndex,
    ).toBe(-1);

    useChatStore.setState({
      streamingBlocks: [
        { type: 'thinking', content: '后续思考', is_open: true },
      ],
    });
    useChatStore
      .getState()
      .handleToolCallStart('call-after-thinking', 'websearch', 0);

    expect(
      useChatStore.getState().activeToolCalls['call-after-thinking']
        .insertAfterBlockIndex,
    ).toBe(0);
  });

  it('按 Rust 输出的结构化事件增量展示思考内容', () => {
    const store = useChatStore.getState();

    store.handleTextStart(0);
    store.handleThinkingStart(0);
    expect(useChatStore.getState().streamingBlocks).toEqual([
      { type: 'thinking', content: '', is_open: true },
    ]);

    store.handleThinkingDelta(0, '正在');
    store.handleThinkingDelta(0, '分析');
    expect(useChatStore.getState().streamingBlocks).toEqual([
      { type: 'thinking', content: '正在分析', is_open: true },
    ]);
  });

  it('思考结束后切回正文，并且 text_end 不重复思考块', () => {
    const store = useChatStore.getState();

    store.handleTextStart(0);
    store.handleThinkingStart(0);
    store.handleThinkingDelta(0, '分析过程');
    store.handleThinkingEnd(0, '分析过程');
    store.feedTextDelta('最终答案');
    store.smoothTextDelta(4);

    expect(useChatStore.getState().streamingBlocks).toEqual([
      { type: 'thinking', content: '分析过程', is_open: false },
      { type: 'text', content: '最终答案' },
    ]);

    useChatStore
      .getState()
      .handleTextEnd(0, '<think>分析过程</think>最终答案');
    expect(useChatStore.getState().streamingBlocks).toEqual([
      { type: 'thinking', content: '分析过程', is_open: false },
      { type: 'text', content: '最终答案' },
      { type: 'text', content: '' },
    ]);
  });

  it('结构化思考块结束后，普通 text_delta 仍作为正文处理', () => {
    useChatStore.setState({
      streamingBlocks: [
        { type: 'thinking', content: '结构化思考', is_open: true },
      ],
    });

    const store = useChatStore.getState();
    store.feedTextDelta('正文');
    store.smoothTextDelta(2);

    expect(useChatStore.getState().streamingBlocks).toEqual([
      { type: 'thinking', content: '结构化思考', is_open: false },
      { type: 'text', content: '正文' },
    ]);
  });
});

describe('chatStore 流式中断收尾', () => {
  it('等待逐字队列消费完毕后再提交 text_end 和 done', () => {
    const store = useChatStore.getState();

    store.feedTextDelta('答案');
    store.handleTextEnd(0, '答案');
    store.handleStreamDone();

    expect(useChatStore.getState()).toEqual(
      expect.objectContaining({
        isStreaming: true,
        pendingTextBuffer: '答案',
        pendingTextEnd: { contentIndex: 0, content: '答案' },
        streamDonePending: true,
      }),
    );

    useChatStore.getState().smoothTextDelta(1);
    expect(useChatStore.getState().messages[0].content).toBe('答');
    expect(useChatStore.getState().isStreaming).toBe(true);

    useChatStore.getState().smoothTextDelta(1);
    const state = useChatStore.getState();
    expect(state.messages[0].content).toBe('答案');
    expect(state.messages[0].blocks).toEqual([
      { type: 'text', content: '答案' },
    ]);
    expect(state.isStreaming).toBe(false);
    expect(state.pendingTextEnd).toBeNull();
    expect(state.streamDonePending).toBe(false);
  });

  it('逐字消费时不会拆开 Unicode 代理对字符', () => {
    const store = useChatStore.getState();

    store.feedTextDelta('A😀');
    store.smoothTextDelta(1);
    expect(useChatStore.getState().messages[0].content).toBe('A');

    useChatStore.getState().smoothTextDelta(1);
    expect(useChatStore.getState().messages[0].content).toBe('A😀');
    expect(useChatStore.getState().pendingTextBuffer).toBe('');
  });

  it('丢弃尚未完成且参数只有半截 JSON 的工具调用', () => {
    useChatStore.setState({
      activeToolCalls: {
        partial: {
          id: 'partial',
          name: 'ask_user',
          arguments: '{"question":"尚未生成完',
          status: 'calling',
        },
      },
      pendingQuestion: {
        id: 'partial',
        question: '等待回答',
        options: [],
        multiSelect: false,
        header: '询问用户',
      },
      toolApproval: {
        id: 'partial',
        name: 'ask_user',
        arguments: '{}',
        reason: '测试',
      },
    });

    useChatStore.getState().handleStreamDone();

    const state = useChatStore.getState();
    expect(state.messages[0].tool_calls).toBeUndefined();
    expect(state.activeToolCalls).toEqual({});
    expect(state.pendingQuestion).toBeNull();
    expect(state.toolApproval).toBeNull();
  });

  it('保留已经完成且参数为 JSON 对象的工具调用', () => {
    useChatStore.setState({
      activeToolCalls: {
        completed: {
          id: 'completed',
          name: 'read_file',
          arguments: '{"path":"/tmp/a.txt"}',
          status: 'done',
          result: 'ok',
        },
      },
    });

    useChatStore.getState().handleStreamDone();

    expect(useChatStore.getState().messages[0].tool_calls).toEqual([
      expect.objectContaining({
        id: 'completed',
        arguments: '{"path":"/tmp/a.txt"}',
        status: 'done',
      }),
    ]);
  });

  it('流式报错时不会把未完成调用伪装成可复用的错误调用', () => {
    useChatStore.setState({
      activeToolCalls: {
        partial: {
          id: 'partial',
          name: 'ask_user',
          arguments: '{"question":',
          status: 'executing',
        },
      },
    });

    useChatStore.getState().handleStreamError('network', '网络错误');

    const state = useChatStore.getState();
    // 空 assistant 占位气泡在流式立即报错时被移除（避免"幽灵气泡"残留、
    // 下一次发送时被重新发给 API），未完成调用也不会写入历史。
    expect(state.messages).toEqual([]);
    expect(state.activeToolCalls).toEqual({});
    expect(state.error).toBe('网络错误');
  });
});

describe('chatStore 历史工具状态恢复', () => {
  it('根据 tool result 将历史调用与展示图片恢复为已完成', () => {
    useChatStore.getState().setMessages([
      {
        ...assistantMessage(),
        tool_calls: [
          {
            id: 'call-success',
            name: 'ask_user',
            arguments: '{"question":"选哪个？"}',
          },
        ],
      },
      {
        id: 'tool-success',
        role: 'tool',
        content: '用户选择：继续',
        images: [
          {
            id: 'generated-1',
            name: 'generated.png',
            media_type: 'image/png',
            data_url: 'data:image/png;base64,aGVsbG8=',
          },
        ],
        model_id: null,
        created_at: 1,
        tool_call_id: 'call-success',
        tool_name: 'ask_user',
        is_error: false,
      },
    ]);

    expect(useChatStore.getState().messages[0].tool_calls?.[0]).toEqual(
      expect.objectContaining({
        status: 'done',
        result: '用户选择：继续',
        is_error_result: false,
        images: [
          expect.objectContaining({
            id: 'generated-1',
            data_url: 'data:image/png;base64,aGVsbG8=',
          }),
        ],
      }),
    );
  });

  it('根据错误结果恢复为失败，没有结果的调用恢复为已中断', () => {
    useChatStore.getState().setMessages([
      {
        ...assistantMessage(),
        tool_calls: [
          {
            id: 'call-error',
            name: 'read_file',
            arguments: '{"path":"/missing"}',
          },
          {
            id: 'call-interrupted',
            name: 'ask_user',
            arguments: '{"question":"未完成"}',
          },
        ],
      },
      {
        id: 'tool-error',
        role: 'tool',
        content: '文件不存在',
        model_id: null,
        created_at: 1,
        tool_call_id: 'call-error',
        tool_name: 'read_file',
        is_error: true,
      },
    ]);

    const toolCalls = useChatStore.getState().messages[0].tool_calls;
    expect(toolCalls?.[0]).toEqual(
      expect.objectContaining({
        status: 'error',
        result: '文件不存在',
        is_error_result: true,
      }),
    );
    expect(toolCalls?.[1].status).toBe('interrupted');
  });
});
