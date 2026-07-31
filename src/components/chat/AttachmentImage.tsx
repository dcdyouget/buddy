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
  const [missing, setMissing] = useState(!source);

  useEffect(() => {
    setMissing(!source);
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

  return (
    <img
      className={className}
      src={source}
      alt={alt}
      title={image.name}
      style={style}
      loading={loading}
      onError={() => setMissing(true)}
    />
  );
}
