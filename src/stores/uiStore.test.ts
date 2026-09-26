import { beforeEach, describe, expect, it, vi } from 'vitest';
import { resizeWindowForPage } from '@/utils/windowResize';
import { useUIStore } from './uiStore';

vi.mock('@/utils/windowResize', () => ({
  resizeWindowForPage: vi.fn(),
}));

beforeEach(() => {
  vi.clearAllMocks();
  useUIStore.setState({
    currentPage: 'empty',
    previousPage: null,
    error: null,
    errorType: null,
  });
});

describe('uiStore.setPage', () => {
  it('离开气泡页时立即挂载目标页，不等待原生窗口缩放', async () => {
    let finishResize!: () => void;
    vi.mocked(resizeWindowForPage).mockReturnValue(
      new Promise<void>((resolve) => {
        finishResize = resolve;
      }),
    );

    const transition = useUIStore.getState().setPage('conversation');

    expect(useUIStore.getState().currentPage).toBe('conversation');
    expect(useUIStore.getState().previousPage).toBe('empty');
    expect(resizeWindowForPage).toHaveBeenCalledWith('empty', 'conversation');

    finishResize();
    await transition;
  });
});
