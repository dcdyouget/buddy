import { motion, AnimatePresence, useIsPresent } from 'framer-motion';

/** SlideInPanel 侧滑面板组件的 Props */
interface SlideInPanelProps {
  /** 面板内容 */
  children: React.ReactNode;
  /** 滑入方向，默认从右侧滑入 */
  from?: 'right' | 'left';
  /** 是否显示面板 */
  show: boolean;
}

interface SlidingLayerProps {
  children: React.ReactNode;
  from: 'right' | 'left';
}

/** 退出动画期间立即释放鼠标事件，避免透明覆层吞掉下一次点击。 */
function SlidingLayer({ children, from }: SlidingLayerProps) {
  const isPresent = useIsPresent();

  return (
    <motion.div
      initial={{ x: from === 'right' ? '100%' : '-100%', opacity: 0 }}
      animate={{ x: 0, opacity: 1 }}
      exit={{ x: from === 'right' ? '100%' : '-100%', opacity: 0 }}
      transition={{
        duration: 0.2,
        ease: [0.2, 0.0, 0, 1],
      }}
      style={{
        position: 'absolute',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        zIndex: 100,
        pointerEvents: isPresent ? 'auto' : 'none',
      }}
    >
      {children}
    </motion.div>
  );
}

/**
 * SlideInPanel — 侧滑面板组件
 *
 * 从屏幕左侧或右侧滑入的覆盖面板，带淡入淡出动画。
 * 使用 Framer Motion 的 AnimatePresence 实现进入和退出动画。
 * 面板覆盖整个父容器（position: absolute，撑满四边）。
 *
 * @param props.children - 面板内渲染的内容
 * @param props.from - 滑入方向，'right'（默认）或 'left'
 * @param props.show - 控制面板显示/隐藏
 */
export function SlideInPanel({ children, from = 'right', show }: SlideInPanelProps) {
  return (
    <AnimatePresence>
      {show && (
        <SlidingLayer from={from}>
          {children}
        </SlidingLayer>
      )}
    </AnimatePresence>
  );
}
