/** 原生窗口显示前触发，用于重新播放出现动画。 */
export const WINDOW_WILL_SHOW_EVENT = 'buddy:window-will-show';

/** 原生窗口隐藏前触发，用于预置下一次出现动画并刷新后台流式文本。 */
export const WINDOW_WILL_HIDE_EVENT = 'buddy:window-will-hide';

export interface WindowWillShowPayload {
  open_compact: boolean;
  /** 快捷键呼出链路 ID；托盘呼出时为 null。 */
  trace_id?: number | null;
  /** Rust 发出事件的 Unix 毫秒时间，用于计算 WebView 事件排队耗时。 */
  emitted_at_ms?: number;
}
