import { invoke, isTauri } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { resizeWindowForPage, resizeWindowToPage } from './windowResize';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  isTauri: vi.fn(),
}));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockResolvedValue(undefined);
});

describe('windowResize', () => {
  it('把页面尺寸计算收敛为一次 Rust IPC', async () => {
    await resizeWindowToPage('conversation');

    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith('resize_window_to_page', {
      page: 'conversation',
    });
  });

  it('只有离开紧凑页面时才自动放大窗口', async () => {
    await resizeWindowForPage('empty', 'conversation');
    await resizeWindowForPage('conversation', 'streaming');

    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it('浏览器预览不调用原生窗口命令', async () => {
    vi.mocked(isTauri).mockReturnValue(false);

    await resizeWindowToPage('conversation');

    expect(invoke).not.toHaveBeenCalled();
  });
});
