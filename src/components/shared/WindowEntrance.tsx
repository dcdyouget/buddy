import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from 'react';
import {
  WINDOW_WILL_HIDE_EVENT,
  WINDOW_WILL_SHOW_EVENT,
  type WindowWillShowPayload,
} from '@/utils/windowEvents';

type EntrancePhase = 'hidden' | 'entering' | 'settled';

interface WindowEntranceProps {
  children: ReactNode;
  mode: 'compact' | 'expanded';
  onCompactRequested?: () => void | Promise<void>;
}

const COMPACT_ENTRANCE_SETTLE_DELAY = 260;

function prefersReducedMotion(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  );
}

/**
 * 稳定的窗口容器。
 * 对话页始终静态呈现；紧凑输入框只保留短暂的外壳形变，不包含炫光图层。
 */
export function WindowEntrance({
  children,
  mode,
  onCompactRequested,
}: WindowEntranceProps) {
  const [phase, setPhase] = useState<EntrancePhase>('settled');
  const phaseRef = useRef<EntrancePhase>(phase);
  const restartFrameRef = useRef(0);
  const showRequestIdRef = useRef(0);

  const setEntrancePhase = useCallback((nextPhase: EntrancePhase) => {
    phaseRef.current = nextPhase;
    setPhase(nextPhase);
  }, []);

  const resetEntrance = useCallback(() => {
    showRequestIdRef.current += 1;
    window.cancelAnimationFrame(restartFrameRef.current);
    restartFrameRef.current = 0;
    setEntrancePhase(prefersReducedMotion() ? 'settled' : 'hidden');
  }, [setEntrancePhase]);

  const playEntrance = useCallback(() => {
    if (prefersReducedMotion()) {
      setEntrancePhase('settled');
      return;
    }

    window.cancelAnimationFrame(restartFrameRef.current);
    if (phaseRef.current === 'hidden') {
      setEntrancePhase('entering');
      return;
    }

    setEntrancePhase('hidden');
    restartFrameRef.current = window.requestAnimationFrame(() => {
      restartFrameRef.current = 0;
      setEntrancePhase('entering');
    });
  }, [setEntrancePhase]);

  const handleShowRequest = useCallback(
    (payload?: WindowWillShowPayload) => {
      if (payload?.open_compact !== true || !onCompactRequested) {
        playEntrance();
        return;
      }

      resetEntrance();
      const requestId = ++showRequestIdRef.current;
      void Promise.resolve(onCompactRequested())
        .catch((error) => {
          console.error('[Buddy] 恢复气泡模式失败:', error);
        })
        .finally(() => {
          if (showRequestIdRef.current === requestId) {
            playEntrance();
          }
        });
    },
    [onCompactRequested, playEntrance, resetEntrance],
  );

  useEffect(() => {
    if (phase !== 'entering' || mode !== 'compact') return;
    const timer = window.setTimeout(
      () => setEntrancePhase('settled'),
      COMPACT_ENTRANCE_SETTLE_DELAY,
    );
    return () => window.clearTimeout(timer);
  }, [mode, phase, setEntrancePhase]);

  useEffect(() => {
    const handleDomShow = (event: Event) => {
      const payload = (event as CustomEvent<WindowWillShowPayload>).detail;
      handleShowRequest(payload);
    };
    window.addEventListener(WINDOW_WILL_SHOW_EVENT, handleDomShow);
    window.addEventListener(WINDOW_WILL_HIDE_EVENT, resetEntrance);

    if (!(window as any).__TAURI_INTERNALS__) {
      return () => {
        window.cancelAnimationFrame(restartFrameRef.current);
        window.removeEventListener(WINDOW_WILL_SHOW_EVENT, handleDomShow);
        window.removeEventListener(WINDOW_WILL_HIDE_EVENT, resetEntrance);
      };
    }

    let disposed = false;
    const unlisteners: Array<() => void> = [];
    import('@tauri-apps/api/event')
      .then(async ({ listen }) => {
        const showUnlisten = await listen<WindowWillShowPayload>(
          WINDOW_WILL_SHOW_EVENT,
          (event) => handleShowRequest(event.payload),
        );
        const hideUnlisten = await listen(WINDOW_WILL_HIDE_EVENT, resetEntrance);
        if (disposed) {
          showUnlisten();
          hideUnlisten();
          return;
        }
        unlisteners.push(showUnlisten, hideUnlisten);
      })
      .catch((error) => {
        console.error('[Buddy] 窗口状态事件监听失败:', error);
      });

    return () => {
      disposed = true;
      window.cancelAnimationFrame(restartFrameRef.current);
      unlisteners.forEach((unlisten) => unlisten());
      window.removeEventListener(WINDOW_WILL_SHOW_EVENT, handleDomShow);
      window.removeEventListener(WINDOW_WILL_HIDE_EVENT, resetEntrance);
    };
  }, [handleShowRequest, resetEntrance]);

  const isExpanded = mode === 'expanded';
  const entranceStyle = {
    '--window-entrance-scale-x': isExpanded ? 1 : 0.94,
    '--window-entrance-scale-y': isExpanded ? 1 : 0.88,
    '--window-entrance-radius': isExpanded
      ? 'var(--radius-xl)'
      : 'var(--radius-full)',
  } as CSSProperties;

  return (
    <div
      className={`window-entrance is-${phase} ${
        isExpanded ? 'is-expanded' : 'is-compact'
      }`}
      data-entrance-phase={phase}
      style={entranceStyle}
    >
      <div
        className="window-entrance-surface surface-glass buddy-shell"
        aria-hidden="true"
      />
      <div className="window-entrance-content">{children}</div>
    </div>
  );
}
