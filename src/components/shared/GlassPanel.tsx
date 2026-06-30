import { type HTMLAttributes, forwardRef } from 'react';

interface GlassPanelProps extends HTMLAttributes<HTMLDivElement> {
  children: React.ReactNode;
  className?: string;
}

/**
 * GlassPanel - 毛玻璃容器面板
 *
 * 提供统一的毛玻璃效果容器，用于承载页面内容。
 * 继承 HTMLDivElement 的所有原生属性，并通过 forwardRef 暴露底层 DOM 引用。
 * 默认设置了 `data-tauri-drag-region` 属性，使该区域可作为 Tauri 窗口拖拽手柄。
 *
 * @param children - 容器内的子元素
 * @param className - 额外的 CSS 类名，会与 `surface-glass` 合并
 */
export const GlassPanel = forwardRef<HTMLDivElement, GlassPanelProps>(
  ({ children, className = '', style, ...props }, ref) => {
    return (
      <div
        ref={ref}
        className={`surface-glass ${className}`}
        style={{
          ...style, // 透传外部样式，允许调用方覆盖或扩展
        }}
        {...props}
      >
        {children}
      </div>
    );
  },
);

GlassPanel.displayName = 'GlassPanel';
