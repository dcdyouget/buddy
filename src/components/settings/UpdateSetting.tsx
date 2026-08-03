import { useEffect, useRef, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { relaunch } from '@tauri-apps/plugin-process';
import {
  check,
  type DownloadEvent,
  type Update,
} from '@tauri-apps/plugin-updater';
import { CircleAlert, CircleCheck, Download, RefreshCw } from 'lucide-react';

type UpdateStatus =
  | 'idle'
  | 'checking'
  | 'latest'
  | 'available'
  | 'downloading'
  | 'installing'
  | 'restarting'
  | 'error';

function formatError(prefix: string, error: unknown): string {
  const detail = error instanceof Error ? error.message : String(error);
  return `${prefix}：${detail || '未知错误'}`;
}

/** 设置页中的手动更新检查与安装入口。 */
export function UpdateSetting() {
  const [currentVersion, setCurrentVersion] = useState<string | null>(null);
  const [availableUpdate, setAvailableUpdate] = useState<Update | null>(null);
  const [status, setStatus] = useState<UpdateStatus>('idle');
  const [error, setError] = useState<string | null>(null);
  const [downloadedBytes, setDownloadedBytes] = useState(0);
  const [contentLength, setContentLength] = useState<number | null>(null);
  const downloadedBytesRef = useRef(0);

  useEffect(() => {
    let active = true;
    getVersion()
      .then((version) => {
        if (active) setCurrentVersion(version);
      })
      .catch(() => {
        if (active) setCurrentVersion('未知');
      });
    return () => {
      active = false;
    };
  }, []);

  const isBusy = ['checking', 'downloading', 'installing', 'restarting'].includes(status);
  const progress = contentLength && contentLength > 0
    ? Math.min(100, Math.round((downloadedBytes / contentLength) * 100))
    : null;

  const handleCheck = async () => {
    if (isBusy) return;
    setStatus('checking');
    setError(null);
    setDownloadedBytes(0);
    setContentLength(null);

    if (availableUpdate) {
      await availableUpdate.close().catch(() => undefined);
      setAvailableUpdate(null);
    }

    try {
      const update = await check();
      if (!update) {
        setStatus('latest');
        return;
      }
      setAvailableUpdate(update);
      setStatus('available');
    } catch (checkError) {
      setError(formatError('检查更新失败', checkError));
      setStatus('error');
    }
  };

  const handleDownloadEvent = (event: DownloadEvent) => {
    if (event.event === 'Started') {
      downloadedBytesRef.current = 0;
      setDownloadedBytes(0);
      setContentLength(event.data.contentLength ?? null);
      setStatus('downloading');
      return;
    }
    if (event.event === 'Progress') {
      downloadedBytesRef.current += event.data.chunkLength;
      setDownloadedBytes(downloadedBytesRef.current);
      return;
    }
    setStatus('installing');
  };

  const handleInstall = async () => {
    if (!availableUpdate || isBusy) return;
    setError(null);
    setStatus('downloading');
    try {
      await availableUpdate.downloadAndInstall(handleDownloadEvent);
      setStatus('restarting');
      await relaunch();
    } catch (installError) {
      setError(formatError('更新失败', installError));
      setStatus('error');
    }
  };

  const checkLabel = status === 'checking'
    ? '正在检查…'
    : status === 'downloading' || status === 'installing' || status === 'restarting'
      ? '正在更新…'
      : status === 'idle'
        ? '检查更新'
        : '重新检查';

  return (
    <section className="settings-section update-setting">
      <div className="settings-row update-setting-row">
        <div className="settings-copy">
          <h3>软件更新</h3>
          <p>当前版本 {currentVersion ? `v${currentVersion}` : '读取中…'}</p>
        </div>
        <button
          type="button"
          className="update-setting-action"
          onClick={handleCheck}
          disabled={isBusy}
        >
          <RefreshCw className={status === 'checking' ? 'buddy-spin' : ''} size={14} />
          {checkLabel}
        </button>
      </div>

      <div className="update-setting-feedback" aria-live="polite">
        {status === 'latest' && (
          <div className="update-setting-status is-success">
            <CircleCheck size={14} />
            当前已是最新版本
          </div>
        )}

        {status === 'available' && availableUpdate && (
          <div className="update-release-card">
            <div className="update-release-title">发现新版本 v{availableUpdate.version}</div>
            <div className="update-release-notes">
              {availableUpdate.body?.trim() || '此版本未提供更新说明。'}
            </div>
            <button type="button" className="update-setting-action is-primary" onClick={handleInstall}>
              <Download size={14} />
              立即更新
            </button>
          </div>
        )}

        {(status === 'downloading' || status === 'installing' || status === 'restarting') && (
          <div className="update-progress-card">
            <div className="update-progress-copy">
              <span>
                {status === 'downloading' && '正在下载更新…'}
                {status === 'installing' && '正在安装更新…'}
                {status === 'restarting' && '安装完成，正在重启…'}
              </span>
              {status === 'downloading' && progress !== null && <span>{progress}%</span>}
            </div>
            <div
              className="update-progress-track"
              role="progressbar"
              aria-label="更新下载进度"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={progress ?? undefined}
            >
              <span
                className={progress === null ? 'is-indeterminate' : undefined}
                style={{ width: progress === null ? '18%' : `${progress}%` }}
              />
            </div>
          </div>
        )}

        {status === 'error' && error && (
          <div className="update-setting-status is-error">
            <CircleAlert size={14} />
            {error}
          </div>
        )}
      </div>
    </section>
  );
}
