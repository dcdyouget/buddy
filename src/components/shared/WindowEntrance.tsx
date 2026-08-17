import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type AnimationEvent,
  type CSSProperties,
  type ReactNode,
} from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  WINDOW_WILL_HIDE_EVENT,
  WINDOW_WILL_SHOW_EVENT,
  type WindowWillShowPayload,
} from '@/utils/windowEvents';

type EntrancePhase = 'hidden' | 'entering' | 'settled';
type EntranceGlowVariant =
  | 'left-to-right'
  | 'right-to-left'
  | 'top-right-to-bottom-left'
  | 'top-left-to-bottom-right'
  | 'center-out';

interface WindowEntranceProps {
  children: ReactNode;
  onCompactRequested?: () => void | Promise<void>;
}

type EntranceDiagnosticStage =
  | 'event-received'
  | 'raf-restart'
  | 'animation-start'
  | 'animation-end'
  | 'glow-start'
  | 'glow-end'
  | 'compact-ready';

interface EntranceTrace {
  traceId: number;
  emittedAtMs: number;
  receivedAt: number;
  loggedStages: Set<EntranceDiagnosticStage>;
}

const ENTRANCE_SETTLE_DELAY = 340;
const ENTRANCE_GLOW_VARIANTS: readonly EntranceGlowVariant[] = [
  'left-to-right',
  'right-to-left',
  'top-right-to-bottom-left',
  'top-left-to-bottom-right',
  'center-out',
];

function pickEntranceGlowVariant(
  previous?: EntranceGlowVariant,
  randomValue = Math.random(),
): EntranceGlowVariant {
  const candidates = previous
    ? ENTRANCE_GLOW_VARIANTS.filter((variant) => variant !== previous)
    : ENTRANCE_GLOW_VARIANTS;
  const index = Math.min(
    candidates.length - 1,
    Math.floor(randomValue * candidates.length),
  );
  return candidates[index];
}

function prefersReducedMotion(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  );
}

/**
 * macOS Spotlight 风格窗口出现层。
 * 玻璃外壳先从胶囊形态展开，真实内容稍后显现，避免缩放文字和控件。
 */
export function WindowEntrance({
  children,
  onCompactRequested,
}: WindowEntranceProps) {
  const [phase, setPhase] = useState<EntrancePhase>(() =>
    prefersReducedMotion() ? 'settled' : 'entering',
  );
  const [glowVariant, setGlowVariant] = useState<EntranceGlowVariant>(() =>
    pickEntranceGlowVariant(),
  );
  const phaseRef = useRef<EntrancePhase>(phase);
  const restartFrameRef = useRef(0);
  const showRequestIdRef = useRef(0);
  const activeTraceRef = useRef<EntranceTrace | null>(null);

  const logEntranceStage = useCallback(
    (trace: EntranceTrace | null, stage: EntranceDiagnosticStage) => {
      if (!trace || trace.loggedStages.has(stage)) return;
      trace.loggedStages.add(stage);
      void invoke('log_window_frontend_diagnostic', {
        traceId: trace.traceId,
        stage,
        emittedAtMs: trace.emittedAtMs,
        phaseElapsedMs: performance.now() - trace.receivedAt,
      }).catch(() => {
        // 诊断日志不能影响窗口呼出主链路。
      });
    },
    [],
  );

  const setEntrancePhase = useCallback((nextPhase: EntrancePhase) => {
    phaseRef.current = nextPhase;
    setPhase(nextPhase);
  }, []);

  const resetEntrance = useCallback(() => {
    activeTraceRef.current = null;
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
    setGlowVariant((previous) => pickEntranceGlowVariant(previous));

    // hide 事件已把 WebView 预置在起始帧时，直接进入动画，
    // 避免发布版在原生窗口显示后再空等一帧。
    if (phaseRef.current === 'hidden') {
      setEntrancePhase('entering');
      restartFrameRef.current = window.requestAnimationFrame(() => {
        restartFrameRef.current = 0;
        logEntranceStage(activeTraceRef.current, 'raf-restart');
      });
      return;
    }

    // 窗口只是失焦、没有隐藏时，仍需先落到起始态才能重播动画。
    setEntrancePhase('hidden');
    restartFrameRef.current = window.requestAnimationFrame(() => {
      restartFrameRef.current = 0;
      logEntranceStage(activeTraceRef.current, 'raf-restart');
      setEntrancePhase('entering');
    });
  }, [logEntranceStage, setEntrancePhase]);

  const handleShowRequest = useCallback(
    (payload?: WindowWillShowPayload) => {
      const trace =
        typeof payload?.trace_id === 'number' &&
        typeof payload.emitted_at_ms === 'number'
          ? {
              traceId: payload.trace_id,
              emittedAtMs: payload.emitted_at_ms,
              receivedAt: performance.now(),
              loggedStages: new Set<EntranceDiagnosticStage>(),
            }
          : null;
      logEntranceStage(trace, 'event-received');

      const openCompact = payload?.open_compact === true;
      if (!openCompact || !onCompactRequested) {
        activeTraceRef.current = trace;
        showRequestIdRef.current += 1;
        playEntrance();
        return;
      }

      resetEntrance();
      activeTraceRef.current = trace;
      const requestId = ++showRequestIdRef.current;
      void Promise.resolve(onCompactRequested())
        .catch((error) => {
          console.error('[Buddy] 恢复气泡模式失败:', error);
        })
        .finally(() => {
          if (showRequestIdRef.current === requestId) {
            logEntranceStage(trace, 'compact-ready');
            playEntrance();
          }
        });
    },
    [logEntranceStage, onCompactRequested, playEntrance, resetEntrance],
  );

  const handleEntranceAnimationStart = useCallback(
    (event: AnimationEvent<HTMLDivElement>) => {
      if (
        event.target === event.currentTarget &&
        event.animationName === 'window-content-reveal'
      ) {
        logEntranceStage(activeTraceRef.current, 'animation-start');
      }
    },
    [logEntranceStage],
  );

  const handleEntranceAnimationEnd = useCallback(
    (event: AnimationEvent<HTMLDivElement>) => {
      if (
        event.target === event.currentTarget &&
        event.animationName === 'window-content-reveal'
      ) {
        logEntranceStage(activeTraceRef.current, 'animation-end');
      }
    },
    [logEntranceStage],
  );

  const handleGlowAnimationStart = useCallback(
    (event: AnimationEvent<HTMLSpanElement>) => {
      if (
        event.target === event.currentTarget &&
        event.animationName === 'window-border-glow-flow'
      ) {
        logEntranceStage(activeTraceRef.current, 'glow-start');
      }
    },
    [logEntranceStage],
  );

  const handleGlowAnimationEnd = useCallback(
    (event: AnimationEvent<HTMLSpanElement>) => {
      if (
        event.target === event.currentTarget &&
        event.animationName === 'window-border-glow-flow'
      ) {
        logEntranceStage(activeTraceRef.current, 'glow-end');
      }
    },
    [logEntranceStage],
  );

  useEffect(() => {
    if (phase !== 'entering') return;
    const timer = window.setTimeout(
      () => setEntrancePhase('settled'),
      ENTRANCE_SETTLE_DELAY,
    );
    return () => window.clearTimeout(timer);
  }, [phase, setEntrancePhase]);

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
        console.error('[Buddy] 窗口动效事件监听失败:', error);
      });

    return () => {
      disposed = true;
      window.cancelAnimationFrame(restartFrameRef.current);
      unlisteners.forEach((unlisten) => unlisten());
      window.removeEventListener(WINDOW_WILL_SHOW_EVENT, handleDomShow);
      window.removeEventListener(WINDOW_WILL_HIDE_EVENT, resetEntrance);
    };
  }, [handleShowRequest, resetEntrance]);

  const isExpanded =
    typeof window !== 'undefined' &&
    window.innerHeight > window.innerWidth * 0.25;
  const entranceStyle = {
    '--window-entrance-scale-x': isExpanded ? 0.97 : 0.94,
    '--window-entrance-scale-y': isExpanded ? 0.96 : 0.88,
    '--window-entrance-radius': isExpanded
      ? 'var(--radius-xl)'
      : 'var(--radius-full)',
  } as CSSProperties;

  return (
    <div
      className={`window-entrance is-${phase} glow-${glowVariant}`}
      data-entrance-phase={phase}
      data-glow-variant={glowVariant}
      style={entranceStyle}
    >
      <div
        className="window-entrance-surface surface-glass buddy-shell"
        aria-hidden="true"
      />
      <div
        className="window-entrance-content"
        onAnimationStart={handleEntranceAnimationStart}
        onAnimationEnd={handleEntranceAnimationEnd}
      >
        {children}
      </div>
      <span
        className="window-entrance-glow"
        aria-hidden="true"
        onAnimationStart={handleGlowAnimationStart}
        onAnimationEnd={handleGlowAnimationEnd}
      />
    </div>
  );
}
