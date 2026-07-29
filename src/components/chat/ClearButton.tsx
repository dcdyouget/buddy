import { AnimatePresence, motion, useReducedMotion } from 'framer-motion';
import { X } from 'lucide-react';

/**
 * ClearButton 组件的 Props
 * @param visible - 是否显示清除按钮（仅当输入框有内容时为 true）
 * @param onClear - 点击清除按钮时的回调，通常用于清空输入框
 */
interface ClearButtonProps {
  visible: boolean;
  onClear: () => void;
}

/**
 * 清除按钮组件
 * 用于清空输入框内容。仅在 visible 为 true 时渲染，
 * 渲染为一个圆形的小按钮，内含 X 图标。
 */
export function ClearButton({ visible, onClear }: ClearButtonProps) {
  const shouldReduceMotion = useReducedMotion();

  return (
    <AnimatePresence initial={false}>
      {visible && (
        <motion.button
          className="clear-input-button"
          initial={shouldReduceMotion ? false : { opacity: 0, scale: 0.82 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={shouldReduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.82 }}
          transition={{
            duration: shouldReduceMotion ? 0 : 0.12,
            ease: [0.2, 0, 0, 1],
          }}
          whileTap={shouldReduceMotion ? undefined : { scale: 0.9 }}
          onClick={onClear}
          title="清除输入"
          type="button"
        >
          <X size={12} />
        </motion.button>
      )}
    </AnimatePresence>
  );
}
