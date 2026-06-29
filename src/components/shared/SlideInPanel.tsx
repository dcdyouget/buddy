import { motion, AnimatePresence } from 'framer-motion';

/** SlideInPanel 侧滑面板组件的 Props */
interface SlideInPanelProps {
  /** 面板内容 */
  children: React.ReactNode;
  /** 滑入方向，默认从右侧滑入 */
  from?: 'right' | 'left';
  /** 是否显示面板 */
  show: boolean;
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
        <motion.div
          // 进入动画：从侧边滑入 + 淡入
          initial={{ x: from === 'right' ? '100%' : '-100%', opacity: 0 }}
          // 动画终点：正常位置 + 完全不透明
          animate={{ x: 0, opacity: 1 }}
          // 退出动画：滑回侧边 + 淡出
          exit={{ x: from === 'right' ? '100%' : '-100%', opacity: 0 }}
          transition={{
            duration: 0.2,
            // 自定义缓动曲线，使滑动更自然
            ease: [0.2, 0.0, 0, 1],
          }}
          style={{
            position: 'absolute',
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            zIndex: 100,
          }}
        >
          {children}
        </motion.div>
      )}
    </AnimatePresence>
  );
}
