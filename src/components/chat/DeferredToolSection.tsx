import { Suspense, lazy } from 'react';
import type { ToolCall } from '@/types';

const LazyToolSection = lazy(() =>
  import('./ToolSection').then(({ ToolSection }) => ({ default: ToolSection })),
);

interface DeferredToolSectionProps {
  toolCall: ToolCall;
  isStreaming: boolean;
}

/** 按需加载低频工具详情，避免其图标、动画和代码高亮进入首包。 */
export function DeferredToolSection(props: DeferredToolSectionProps) {
  return (
    <Suspense fallback={<div className="tool-section tool-section-loading" />}>
      <LazyToolSection {...props} />
    </Suspense>
  );
}
