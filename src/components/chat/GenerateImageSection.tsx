import { memo, useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Check,
  ChevronDown,
  ChevronRight,
  Copy,
  Download,
  Image as ImageIcon,
  LoaderCircle,
} from 'lucide-react';

import type { ImageAttachment, ToolCall } from '@/types';

interface GenerateImageSectionProps {
  toolCall: ToolCall;
}

interface GenerateImageResult {
  model?: string;
  prompt?: string;
  image_count?: number;
  revised_prompts?: string[];
  note?: string;
}

interface DownloadState {
  status: 'saving' | 'saved' | 'error';
  message?: string;
}

function parsePrompt(argumentsJson: string): string {
  try {
    const prompt = JSON.parse(argumentsJson)?.prompt;
    return typeof prompt === 'string' ? prompt.trim() : '';
  } catch {
    return '';
  }
}

function parseResult(result: string | undefined): GenerateImageResult {
  if (!result) return {};
  try {
    const payload = JSON.parse(result);
    return payload && typeof payload === 'object'
      ? (payload as GenerateImageResult)
      : {};
  } catch {
    return {};
  }
}

export const GenerateImageSection = memo(function GenerateImageSection({
  toolCall,
}: GenerateImageSectionProps) {
  const [expanded, setExpanded] = useState(false);
  const [promptCopied, setPromptCopied] = useState(false);
  const copyResetTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [downloadStates, setDownloadStates] = useState<
    Record<string, DownloadState>
  >({});
  useEffect(() => {
    return () => {
      if (copyResetTimer.current) clearTimeout(copyResetTimer.current);
    };
  }, []);
  const toggle = useCallback(() => {
    setExpanded((previous) => !previous);
  }, []);
  const downloadImage = useCallback(async (image: ImageAttachment) => {
    setDownloadStates((previous) => ({
      ...previous,
      [image.id]: { status: 'saving' },
    }));
    try {
      const savedPath = await invoke<string>('download_generated_image', {
        dataUrl: image.data_url,
        mediaType: image.media_type,
      });
      setDownloadStates((previous) => ({
        ...previous,
        [image.id]: { status: 'saved', message: savedPath },
      }));
    } catch (error) {
      setDownloadStates((previous) => ({
        ...previous,
        [image.id]: {
          status: 'error',
          message: error instanceof Error ? error.message : String(error),
        },
      }));
    }
  }, []);

  const status = toolCall.status ?? 'calling';
  const isActive = status === 'calling' || status === 'executing';
  const isError = status === 'error';
  const images = toolCall.images ?? [];
  const result = parseResult(toolCall.result);
  const prompt = result.prompt?.trim() || parsePrompt(toolCall.arguments);
  const copyPrompt = useCallback(async () => {
    if (!prompt) return;
    try {
      await navigator.clipboard.writeText(prompt);
      setPromptCopied(true);
      if (copyResetTimer.current) clearTimeout(copyResetTimer.current);
      copyResetTimer.current = setTimeout(() => setPromptCopied(false), 1600);
    } catch (error) {
      console.error('复制生成提示词失败', error);
    }
  }, [prompt]);
  const label = isActive
    ? '正在生成图片'
    : isError
      ? '图片生成失败'
      : status === 'interrupted'
        ? '图片生成已中断'
        : '图片生成完成';

  return (
    <div
      className={`generate-image-section ${
        isActive ? 'is-generating' : ''
      } ${isError ? 'is-error' : ''} ${expanded ? 'is-expanded' : ''}`}
      role="status"
      aria-live="polite"
    >
      <button
        className="generate-image-section-header"
        type="button"
        onClick={toggle}
        aria-expanded={expanded}
        aria-label={`${label}：${prompt || '未获得提示词'}`}
      >
        <ImageIcon size={14} aria-hidden="true" />
        <span className="generate-image-section-label">{label}</span>
        {isActive && (
          <span className="think-section-loader" aria-label="生成中">
            <span />
            <span />
            <span />
          </span>
        )}
        {prompt && (
          <span className="generate-image-section-prompt" title={prompt}>
            {prompt}
          </span>
        )}
        <span className="generate-image-section-chevron" aria-hidden="true">
          {expanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
        </span>
      </button>

      {images.length > 0 && (
        <div className="generate-image-grid">
          {images.map((image, index) => {
            const downloadState = downloadStates[image.id];
            return (
              <figure className="generate-image-card" key={image.id}>
                <img
                  src={image.data_url}
                  alt={prompt || image.name || '模型生成的图片'}
                  loading="lazy"
                />
                <button
                  className={`generate-image-download ${
                    downloadState?.status === 'saved' ? 'is-saved' : ''
                  } ${downloadState?.status === 'error' ? 'is-error' : ''}`}
                  type="button"
                  disabled={downloadState?.status === 'saving'}
                  aria-label={`下载图片 ${index + 1}`}
                  title={
                    downloadState?.message ||
                    '将图片保存到系统“下载”目录'
                  }
                  onClick={() => void downloadImage(image)}
                >
                  {downloadState?.status === 'saving' ? (
                    <LoaderCircle
                      className="buddy-spin"
                      size={14}
                      aria-hidden="true"
                    />
                  ) : downloadState?.status === 'saved' ? (
                    <Check size={14} aria-hidden="true" />
                  ) : (
                    <Download size={14} aria-hidden="true" />
                  )}
                  <span>
                    {downloadState?.status === 'saving'
                      ? '保存中'
                      : downloadState?.status === 'saved'
                        ? '已下载'
                        : downloadState?.status === 'error'
                          ? '重试'
                          : '下载'}
                  </span>
                </button>
                {downloadState?.status === 'error' && (
                  <figcaption className="generate-image-download-error">
                    {downloadState.message || '图片下载失败'}
                  </figcaption>
                )}
              </figure>
            );
          })}
        </div>
      )}

      {expanded && (
        <div className="generate-image-section-body">
          <div className="generate-image-meta-row">
            <span>生成提示词</span>
            <strong>{prompt || '未获得提示词'}</strong>
          </div>
          {(result.model || prompt) && (
            <div className="generate-image-meta-row">
              <span>生成模型</span>
              <div className="generate-image-meta-value">
                <strong>{result.model || '未知模型'}</strong>
                {prompt && (
                <button
                  className={`generate-image-prompt-copy ${
                    promptCopied ? 'is-copied' : ''
                  }`}
                  type="button"
                  onClick={() => void copyPrompt()}
                  title={promptCopied ? '已复制' : '复制生成提示词'}
                  aria-label={promptCopied ? '生成提示词已复制' : '复制生成提示词'}
                >
                  {promptCopied ? (
                    <Check size={12} aria-hidden="true" />
                  ) : (
                    <Copy size={12} aria-hidden="true" />
                  )}
                  <span>{promptCopied ? '已复制' : '复制'}</span>
                </button>
                )}
              </div>
            </div>
          )}
          {Array.isArray(result.revised_prompts) &&
            result.revised_prompts.length > 0 && (
              <div className="generate-image-meta-row">
                <span>优化后提示词</span>
                <strong>{result.revised_prompts.join('\n')}</strong>
              </div>
            )}
          {isActive && (
            <div className="generate-image-empty">正在等待图片生成结果…</div>
          )}
          {isError && (
            <div className="generate-image-error">
              {toolCall.result || '图片生成接口调用失败'}
            </div>
          )}
        </div>
      )}
    </div>
  );
});
