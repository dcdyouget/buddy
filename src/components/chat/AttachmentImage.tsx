import {
  convertFileSrc,
} from '@tauri-apps/api/core';
import { ImageOff } from 'lucide-react';
import {
  useEffect,
  useMemo,
  useState,
  type CSSProperties,
} from 'react';

import type { ImageAttachment } from '@/types';

interface AttachmentImageProps {
  image: ImageAttachment;
  alt: string;
  className?: string;
  style?: CSSProperties;
  loading?: 'eager' | 'lazy';
}

function attachmentSource(image: ImageAttachment): string {
  if (image.path) {
    try {
      return convertFileSrc(image.path);
    } catch {
      return image.path;
    }
  }
  return image.data_url || '';
}

export function AttachmentImage({
  image,
  alt,
  className,
  style,
  loading,
}: AttachmentImageProps) {
  const source = useMemo(
    () => attachmentSource(image),
    [image.path, image.data_url],
  );
  // missing = 完全没有可显示来源（文件与 data_url 都不存在）——永久性
  const [missing, setMissing] = useState(!source);
  // loadFailed = 有来源但 <img> 加载失败（可能是暂时性错误）——允许重试
  const [loadFailed, setLoadFailed] = useState(false);
  const [retryKey, setRetryKey] = useState(0);

  useEffect(() => {
    setMissing(!source);
    setLoadFailed(false);
  }, [source]);

  if (missing) {
    return (
      <div
        className={`attachment-image-missing ${className || ''}`}
        style={style}
        role="img"
        aria-label={`${alt}，图片已删除`}
      >
        <ImageOff size={18} aria-hidden="true" />
        <span>图片已删除</span>
        <code title={image.path || image.name}>
          {image.path || image.name}
        </code>
      </div>
    );
  }

  if (loadFailed) {
    // 暂时性加载失败：提供重试，避免瞬时错误被当成"图片已删除"永久卡死
    return (
      <div
        className={`attachment-image-missing ${className || ''}`}
        style={style}
        role="img"
        aria-label={`${alt}，图片加载失败`}
      >
        <ImageOff size={18} aria-hidden="true" />
        <span>图片加载失败</span>
        <button
          type="button"
          onClick={() => {
            setLoadFailed(false);
            setRetryKey((key) => key + 1);
          }}
          style={{
            border: 'none',
            background: 'transparent',
            color: 'var(--buddy-primary)',
            cursor: 'pointer',
            fontSize: '12px',
            textDecoration: 'underline',
          }}
        >
          重试
        </button>
        <code title={image.path || image.name}>
          {image.path || image.name}
        </code>
      </div>
    );
  }

  return (
    <img
      key={retryKey}
      className={className}
      src={source}
      alt={alt}
      title={image.name}
      style={style}
      loading={loading}
      onError={() => setLoadFailed(true)}
    />
  );
}
