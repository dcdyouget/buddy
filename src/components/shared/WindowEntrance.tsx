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

const ENTRANCE_SETTLE_DELAY = 400;
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
  const restartFrameRef = useRef(0);
  const showRequestIdRef = useRef(0);

  const resetEntrance = useCallback(() => {
    showRequestIdRef.current += 1;
    window.cancelAnimationFrame(restartFrameRef.current);
    restartFrameRef.current = 0;
    setPhase(prefersReducedMotion() ? 'settled' : 'hidden');
  }, []);

  const playEntrance = useCallback(() => {
    if (prefersReducedMotion()) {
      setPhase('settled');
      return;
    }

    window.cancelAnimationFrame(restartFrameRef.current);
    setGlowVariant((previous) => pickEntranceGlowVariant(previous));
    setPhase('hidden');
    restartFrameRef.current = window.requestAnimationFrame(() => {
      restartFrameRef.current = 0;
      setPhase('entering');
    });
  }, []);

  const handleShowRequest = useCallback(
    (openCompact: boolean) => {
      if (!openCompact || !onCompactRequested) {
        showRequestIdRef.current += 1;
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
    if (phase !== 'entering') return;
    const timer = window.setTimeout(
      () => setPhase('settled'),
      ENTRANCE_SETTLE_DELAY,
    );
    return () => window.clearTimeout(timer);
  }, [phase]);

  useEffect(() => {
    const handleDomShow = (event: Event) => {
      const payload = (event as CustomEvent<WindowWillShowPayload>).detail;
      handleShowRequest(payload?.open_compact === true);
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
          (event) => handleShowRequest(event.payload?.open_compact === true),
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
    '--window-entrance-scale-x': isExpanded ? 0.78 : 0.66,
    '--window-entrance-scale-y': isExpanded ? 0.58 : 0.8,
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
      >
        <span className="window-entrance-glow" />
      </div>
      <div className="window-entrance-content">{children}</div>
    </div>
  );
}
