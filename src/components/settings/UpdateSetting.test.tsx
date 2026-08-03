// @vitest-environment jsdom

import { cleanup, fireEvent, render, waitFor } from '@testing-library/react';
import type { DownloadEvent } from '@tauri-apps/plugin-updater';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { UpdateSetting } from './UpdateSetting';

const mocks = vi.hoisted(() => ({
  getVersion: vi.fn(),
  check: vi.fn(),
  relaunch: vi.fn(),
}));

vi.mock('@tauri-apps/api/app', () => ({
  getVersion: mocks.getVersion,
}));

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: mocks.check,
}));

vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: mocks.relaunch,
}));

beforeEach(() => {
  mocks.getVersion.mockResolvedValue('1.0.0');
  mocks.check.mockReset();
  mocks.relaunch.mockReset();
  mocks.relaunch.mockResolvedValue(undefined);
});

afterEach(cleanup);

describe('UpdateSetting', () => {
  it('只在用户点击后检查更新，并展示已是最新版本', async () => {
    mocks.check.mockResolvedValue(null);
    const { findByText, getByRole } = render(<UpdateSetting />);

    await findByText('当前版本 v1.0.0');
    expect(mocks.check).not.toHaveBeenCalled();

    fireEvent.click(getByRole('button', { name: '检查更新' }));

    await findByText('当前已是最新版本');
    expect(mocks.check).toHaveBeenCalledTimes(1);
  });

  it('展示更新说明，并在确认后下载安装和重启', async () => {
    const downloadAndInstall = vi.fn(async (onEvent?: (event: DownloadEvent) => void) => {
      onEvent?.({ event: 'Started', data: { contentLength: 100 } });
      onEvent?.({ event: 'Progress', data: { chunkLength: 60 } });
      onEvent?.({ event: 'Finished' });
    });
    mocks.check.mockResolvedValue({
      version: '1.1.0',
      body: '新增手动检查更新\n修复窗口唤起问题',
      close: vi.fn().mockResolvedValue(undefined),
      downloadAndInstall,
    });

    const { findByText, getByRole } = render(<UpdateSetting />);
    fireEvent.click(getByRole('button', { name: '检查更新' }));

    await findByText('发现新版本 v1.1.0');
    await findByText(/新增手动检查更新/);
    fireEvent.click(getByRole('button', { name: '立即更新' }));

    await waitFor(() => expect(downloadAndInstall).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(mocks.relaunch).toHaveBeenCalledTimes(1));
  });

  it('检查失败时显示错误并允许重新检查', async () => {
    mocks.check.mockRejectedValue(new Error('网络不可用'));
    const { findByText, getByRole } = render(<UpdateSetting />);

    fireEvent.click(getByRole('button', { name: '检查更新' }));

    await findByText('检查更新失败：网络不可用');
    const retryButton = getByRole('button', { name: '重新检查' }) as HTMLButtonElement;
    expect(retryButton.disabled).toBe(false);
  });
});
