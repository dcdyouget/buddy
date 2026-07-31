import {
  useRef,
  useEffect,
  useState,
  type ChangeEvent,
  type KeyboardEvent,
  type PointerEvent,
} from 'react';
import { Bot, ImagePlus, Send, Settings, Square, X } from 'lucide-react';
import { IconButton } from '@/components/shared/IconButton';
import { ClearButton } from './ClearButton';
import { AttachmentImage } from './AttachmentImage';
import type { ImageAttachment, ModelInfo } from '@/types';

const MAX_IMAGE_COUNT = 4;
const MAX_IMAGE_BYTES = 5 * 1024 * 1024;
const SUPPORTED_IMAGE_TYPES = new Set([
  'image/jpeg',
  'image/png',
  'image/gif',
  'image/webp',
]);

/**
 * InputDock 组件的 Props
 * @param isStreaming - 是否正在流式生成中，控制输入栏的状态切换
 * @param streamingModelName - 流式生成时显示的模型名称
 * @param selectedModel - 当前选中的模型信息
 * @param draftInput - 输入框中的当前草稿文本
 * @param onDraftChange - 输入文本变化时的回调
 * @param onSend - 发送消息的回调
 * @param onStop - 停止生成的回调
 * @param onModelPickerClick - 模型选择器点击回调
 */
interface InputDockProps {
  isStreaming: boolean;
  streamingModelName?: string;
  selectedModel: ModelInfo | null;
  draftInput: string;
  draftImages: ImageAttachment[];
  onDraftChange: (text: string) => void;
  onAddImages: (images: ImageAttachment[]) => void | Promise<void>;
  onRemoveImage: (id: string) => void;
  onAttachmentError?: (message: string) => void;
  onSend: () => void;
  onStop: () => void;
  onModelPickerClick?: () => void;
  onSettingsClick?: () => void;
  /** 禁用 textarea 自动撑高，改为固定高度 + 滚动条（用于紧凑窗口） */
  disableAutoResize?: boolean;
  /** 隐藏顶部分隔线（空态气泡中不需要分隔消息列表） */
  hideBorder?: boolean;
}

/**
 * 输入栏组件
 * 聊天窗口底部的输入区域，包含两种状态：
 * 1. 正常状态：多行输入框（自动撑高，最高 120px）+ 清除按钮 + 发送按钮
 * 2. 流式状态：显示生成进度文本 + 停止按钮
 *
 * 键盘交互：
 * - Enter 直接发送消息
 * - Cmd/Ctrl + Enter 换行
 */
export function InputDock({
  isStreaming,
  streamingModelName,
  selectedModel,
  draftInput,
  draftImages = [],
  onDraftChange,
  onAddImages,
  onRemoveImage,
  onAttachmentError,
  onSend,
  onStop,
  onModelPickerClick,
  onSettingsClick,
  disableAutoResize = false,
  hideBorder = false,
}: InputDockProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const imageInputRef = useRef<HTMLInputElement>(null);
  const [isSavingImages, setIsSavingImages] = useState(false);
  // 保持最新的 isStreaming 值供事件回调使用（避免闭包过期）
  const isStreamingRef = useRef(isStreaming);
  isStreamingRef.current = isStreaming;

  // 输入内容变化时，自动调整 textarea 高度（最高 120px）
  // 紧凑窗口（如 EmptyPage）禁用自动撑高，使用固定高度 + 滚动条
  useEffect(() => {
    if (disableAutoResize) return;
    const ta = textareaRef.current;
    if (ta) {
      ta.style.height = 'auto';
      ta.style.height = Math.min(ta.scrollHeight, 120) + 'px';
    }
  }, [draftInput, disableAutoResize]);

  // 自动聚焦：组件挂载、流式结束、窗口呼出时聚焦输入框
  useEffect(() => {
    const ta = textareaRef.current;
    if (!ta || isStreaming) return;

    // 立即聚焦（组件挂载或 isStreaming 变为 false 时）
    requestAnimationFrame(() => ta.focus());

    // 监听窗口获得焦点事件（快捷键呼出 / 点击托盘图标时触发）
    let unlisten: (() => void) | undefined;
    import('@tauri-apps/api/window').then(({ getCurrentWindow }) => {
      getCurrentWindow().onFocusChanged((focused) => {
        if (focused && !isStreamingRef.current) {
          requestAnimationFrame(() => textareaRef.current?.focus());
        }
      }).then((fn) => { unlisten = fn; });
    }).catch(() => {});

    return () => {
      unlisten?.();
    };
  }, [isStreaming]);

  const hasText = draftInput.trim().length > 0;
  const supportsVision = selectedModel?.supports_vision === true;
  const hasUnsupportedImages = draftImages.length > 0 && !supportsVision;
  const canSend =
    !isSavingImages &&
    !hasUnsupportedImages &&
    (hasText || (supportsVision && draftImages.length > 0));

  const handleImageSelection = async (
    event: ChangeEvent<HTMLInputElement>,
  ) => {
    const files = Array.from(event.target.files || []);
    event.target.value = '';
    if (files.length === 0) return;

    const availableSlots = Math.max(0, MAX_IMAGE_COUNT - draftImages.length);
    if (availableSlots === 0) {
      onAttachmentError?.(`每条消息最多添加 ${MAX_IMAGE_COUNT} 张图片`);
      return;
    }

    const accepted: File[] = [];
    let rejectedType = false;
    let rejectedSize = false;
    for (const file of files.slice(0, availableSlots)) {
      if (!SUPPORTED_IMAGE_TYPES.has(file.type)) {
        rejectedType = true;
        continue;
      }
      if (file.size > MAX_IMAGE_BYTES) {
        rejectedSize = true;
        continue;
      }
      accepted.push(file);
    }

    if (files.length > availableSlots) {
      onAttachmentError?.(`每条消息最多添加 ${MAX_IMAGE_COUNT} 张图片`);
    } else if (rejectedType) {
      onAttachmentError?.('仅支持 JPEG、PNG、GIF 和 WebP 图片');
    } else if (rejectedSize) {
      onAttachmentError?.('单张图片不能超过 5 MB');
    }

    const images = await Promise.all(
      accepted.map(
        (file) =>
          new Promise<ImageAttachment>((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => {
              if (typeof reader.result !== 'string') {
                reject(new Error('图片读取失败'));
                return;
              }
              resolve({
                id:
                  typeof crypto.randomUUID === 'function'
                    ? crypto.randomUUID()
                    : `${Date.now()}-${Math.random()}`,
                name: file.name,
                media_type: file.type,
                data_url: reader.result,
              });
            };
            reader.onerror = () => reject(new Error(`无法读取图片：${file.name}`));
            reader.readAsDataURL(file);
          }),
      ),
    ).catch((error) => {
      onAttachmentError?.(String(error));
      return [];
    });
    if (images.length > 0) {
      setIsSavingImages(true);
      try {
        await onAddImages(images);
      } finally {
        setIsSavingImages(false);
      }
    }
  };

  /**
   * 键盘事件处理
   * - 输入法组合中（中文输入法确认英文等）：不拦截 Enter
   * - Cmd/Ctrl + Enter：允许默认换行行为
   * - 普通 Enter：阻止默认换行行为，触发发送
   * - 流式生成中或无有效输入时不发送
   */
  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter') {
      // IME 输入法处理中：keyCode 229 表示按键正被输入法拦截处理
      // （中文输入法确认英文时 compositionend 在 keydown 之前触发，
      //   此时 isComposing 已为 false，但 keyCode 仍为 229）
      if (e.nativeEvent.isComposing || e.nativeEvent.keyCode === 229) {
        return;
      }
      // Cmd/Ctrl+Enter → 换行，不拦截
      if (e.metaKey || e.ctrlKey) {
        return; // Let default behavior insert newline
      }
      // Plain Enter → 发送消息
      e.preventDefault();
      if (!isStreaming && canSend) {
        onSend();
      }
    }
  };

  const handlePointerMove = (e: PointerEvent<HTMLDivElement>) => {
    const bounds = e.currentTarget.getBoundingClientRect();
    e.currentTarget.style.setProperty(
      '--composer-pointer-x',
      `${e.clientX - bounds.left}px`,
    );
    e.currentTarget.style.setProperty(
      '--composer-pointer-y',
      `${e.clientY - bounds.top}px`,
    );
  };

  const handlePointerLeave = (e: PointerEvent<HTMLDivElement>) => {
    e.currentTarget.style.setProperty('--composer-pointer-x', '50%');
    e.currentTarget.style.setProperty('--composer-pointer-y', '0px');
  };

  return (
    <div
      className={`input-dock ${hideBorder ? 'is-standalone' : ''}`}
      onPointerMove={handlePointerMove}
      onPointerLeave={handlePointerLeave}
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 'var(--space-2)',
        padding: 'var(--space-3)',
        borderTop: 'none',
        width: '100%',
        flexWrap: 'wrap',
      }}
    >
      {!isStreaming && draftImages.length > 0 && (
        <div
          style={{
            width: '100%',
            display: 'flex',
            alignItems: 'center',
            gap: 'var(--space-2)',
            overflowX: 'auto',
            paddingBottom: 'var(--space-1)',
          }}
        >
          {draftImages.map((image) => (
            <div
              key={image.id}
              title={image.name}
              style={{
                position: 'relative',
                width: 44,
                height: 44,
                borderRadius: 'var(--radius-sm)',
                border: '1px solid var(--border-default)',
                overflow: 'hidden',
                flexShrink: 0,
                background: 'var(--bg-sunken)',
              }}
            >
              <AttachmentImage
                image={image}
                alt={image.name}
                style={{ width: '100%', height: '100%', objectFit: 'cover' }}
              />
              <button
                type="button"
                aria-label={`移除图片 ${image.name}`}
                onClick={() => onRemoveImage(image.id)}
                style={{
                  position: 'absolute',
                  top: 2,
                  right: 2,
                  width: 18,
                  height: 18,
                  padding: 0,
                  border: '1px solid var(--border-default)',
                  borderRadius: 'var(--radius-full)',
                  background: 'var(--bg-elevated)',
                  color: 'var(--text-primary)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  cursor: 'pointer',
                }}
              >
                <X size={11} />
              </button>
            </div>
          ))}
          {hasUnsupportedImages && (
            <span style={{ color: 'var(--state-warning)', fontSize: '11px' }}>
              当前模型不支持图片，请移除图片或切换模型
            </span>
          )}
        </div>
      )}
      {!isStreaming && isSavingImages && (
        <span
          style={{
            width: '100%',
            color: 'var(--text-secondary)',
            fontSize: '11px',
          }}
        >
          正在保存图片…
        </span>
      )}
      {isStreaming ? (
        /* 流式生成状态：显示模型名称和生成进度 + 停止按钮 */
        <>
          <div
            style={{
              flex: 1,
              padding: 'var(--space-2) var(--space-3)',
              fontSize: '13px',
              color: 'var(--text-muted)',
            }}
          >
            {streamingModelName || 'AI'} · 生成中...
          </div>
          <IconButton
            icon={Square}
            onClick={onStop}
            variant="danger"
            size={28}
            iconSize={14}
            title="停止生成"
          />
        </>
      ) : (
        /* 正常输入状态：textarea（内含清除按钮）+ 设置 + 模型选择 + 发送 */
        <>
          <div style={{ position: 'relative', flex: 1, display: 'flex' }}>
            <textarea
              className="composer-textarea"
              ref={textareaRef}
              value={draftInput}
              onChange={(e) => onDraftChange(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="问点什么…"
              rows={1}
              style={{
                flex: 1,
                padding: 'var(--space-2) var(--space-3)',
                paddingRight: hasText ? '28px' : 'var(--space-3)',
                borderRadius: 'var(--radius-md)',
                border: '1px solid var(--border-subtle)',
                background: 'var(--bg-sunken)',
                color: 'var(--text-primary)',
                fontFamily: 'var(--font-sans)',
                fontSize: '14px',
                lineHeight: 1.5,
                resize: 'none',
                outline: 'none',
                maxHeight: '120px',
                overflowY: 'auto',
                overflowWrap: 'break-word',
                wordBreak: 'break-word',
              }}
            />

            {/* 清除按钮：定位在输入框内部右侧 */}
            <div
              style={{
                position: 'absolute',
                right: '6px',
                top: '50%',
                transform: 'translateY(-50%)',
                display: 'flex',
                pointerEvents: hasText ? 'auto' : 'none',
              }}
            >
              <ClearButton visible={hasText} onClear={() => onDraftChange('')} />
            </div>
          </div>

          {supportsVision && (
            <>
              <input
                ref={imageInputRef}
                type="file"
                accept="image/jpeg,image/png,image/gif,image/webp"
                multiple
                hidden
                onChange={handleImageSelection}
              />
              <IconButton
                icon={ImagePlus}
                onClick={() => imageInputRef.current?.click()}
                size={24}
                iconSize={14}
                title="添加图片"
                disabled={isSavingImages || draftImages.length >= MAX_IMAGE_COUNT}
              />
            </>
          )}

          {onSettingsClick && (
            <IconButton
              icon={Settings}
              onClick={onSettingsClick}
              className="settings-motion-button"
              size={24}
              iconSize={13}
              title="设置"
            />
          )}

          {onModelPickerClick && (
            <button
              className="model-picker-trigger"
              onClick={onModelPickerClick}
              title={`切换模型：${selectedModel?.display_name || '未选择'}`}
              aria-label={`切换模型，当前为${selectedModel?.display_name || '未选择'}`}
              aria-haspopup="menu"
              type="button"
            >
              <span
                key={selectedModel?.id || 'no-model'}
                className="model-picker-icon"
                aria-hidden="true"
              >
                <Bot size={14} strokeWidth={1.8} />
              </span>
            </button>
          )}

          <IconButton
            icon={Send}
            onClick={onSend}
            className={`send-motion-button ${canSend ? 'is-ready' : ''}`}
            // 有内容时显示主色，无内容时显示默认色
            variant={canSend ? 'primary' : 'default'}
            disabled={!canSend}
            size={28}
            iconSize={14}
            title="发送"
          />
        </>
      )}
    </div>
  );
}
