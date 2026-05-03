import { useRef, useCallback, useEffect, useState } from 'react';

interface UseSwipeOptions {
  threshold?: number;
  direction?: 'horizontal' | 'vertical' | 'all';
  /** Only trigger edge callbacks when touch starts within this many pixels from the screen edge */
  edgeStart?: number;
}

interface UseSwipeReturn {
  swipeOffset: { x: number; y: number };
  swipingDirection: 'left' | 'right' | 'up' | 'down' | null;
  isSwiping: boolean;
  ref: React.RefObject<HTMLDivElement | null>;
}

type SwipeCallbacks = {
  onSwipeLeft?: () => void;
  onSwipeRight?: () => void;
  onSwipeUp?: () => void;
  onSwipeDown?: () => void;
  /** Called instead of onSwipeRight when swipe starts from left edge */
  onEdgeSwipeRight?: () => void;
  /** Called instead of onSwipeLeft when swipe starts from right edge */
  onEdgeSwipeLeft?: () => void;
};

const DIRECTION_LOCK_THRESHOLD = 10;

export function useSwipe(
  callbacks: SwipeCallbacks,
  options: UseSwipeOptions = {},
): UseSwipeReturn {
  const {
    threshold = 80,
    direction = 'horizontal',
    edgeStart,
  } = options;

  const ref = useRef<HTMLDivElement>(null);
  const [swipeOffset, setSwipeOffset] = useState({ x: 0, y: 0 });
  const [swipingDirection, setSwipingDirection] = useState<'left' | 'right' | 'up' | 'down' | null>(null);
  const [isSwiping, setIsSwiping] = useState(false);

  const touchStartRef = useRef({ x: 0, y: 0 });
  const currentOffsetRef = useRef({ x: 0, y: 0 });
  const lockedDirectionRef = useRef<'h' | 'v' | null>(null);
  const isSwipingRef = useRef(false);
  const startedFromEdgeRef = useRef<'left' | 'right' | null>(null);
  const callbacksRef = useRef(callbacks);
  callbacksRef.current = callbacks;

  const handleTouchStart = useCallback((e: TouchEvent) => {
    const touch = e.touches[0];
    touchStartRef.current = { x: touch.clientX, y: touch.clientY };
    lockedDirectionRef.current = null;

    // Track if touch started from an edge
    if (edgeStart !== undefined) {
      const screenWidth = window.innerWidth;
      if (touch.clientX <= edgeStart) {
        startedFromEdgeRef.current = 'left';
      } else if (touch.clientX >= screenWidth - edgeStart) {
        startedFromEdgeRef.current = 'right';
      } else {
        startedFromEdgeRef.current = null;
      }
    } else {
      startedFromEdgeRef.current = null;
    }
  }, [edgeStart]);

  const handleTouchMove = useCallback((e: TouchEvent) => {
    if (!lockedDirectionRef.current && !isSwipingRef.current) return;

    const touch = e.touches[0];
    const dx = touch.clientX - touchStartRef.current.x;
    const dy = touch.clientY - touchStartRef.current.y;

    if (!lockedDirectionRef.current) {
      if (Math.abs(dx) > DIRECTION_LOCK_THRESHOLD || Math.abs(dy) > DIRECTION_LOCK_THRESHOLD) {
        if (Math.abs(dx) > Math.abs(dy)) {
          if (direction === 'vertical') return;
          lockedDirectionRef.current = 'h';
        } else {
          if (direction === 'horizontal') return;
          lockedDirectionRef.current = 'v';
        }
        isSwipingRef.current = true;
        setIsSwiping(true);
      } else {
        return;
      }
    }

    if (lockedDirectionRef.current === 'h') {
      e.preventDefault();
    }

    if (lockedDirectionRef.current === 'h') {
      const offset = { x: dx, y: 0 };
      currentOffsetRef.current = offset;
      setSwipeOffset(offset);
      setSwipingDirection(dx > 0 ? 'right' : 'left');
    } else {
      const offset = { x: 0, y: dy };
      currentOffsetRef.current = offset;
      setSwipeOffset(offset);
      setSwipingDirection(dy > 0 ? 'down' : 'up');
    }
  }, [direction]);

  const handleTouchEnd = useCallback(() => {
    if (!isSwipingRef.current && !lockedDirectionRef.current) {
      setSwipeOffset({ x: 0, y: 0 });
      setSwipingDirection(null);
      return;
    }

    const { x, y } = currentOffsetRef.current;
    const cbs = callbacksRef.current;
    const edge = startedFromEdgeRef.current;

    if (Math.abs(x) > threshold) {
      if (x < 0) {
        // Swipe left
        if (edge === 'right' && cbs.onEdgeSwipeLeft) cbs.onEdgeSwipeLeft();
        else cbs.onSwipeLeft?.();
      } else {
        // Swipe right
        if (edge === 'left' && cbs.onEdgeSwipeRight) cbs.onEdgeSwipeRight();
        else cbs.onSwipeRight?.();
      }
    }
    if (Math.abs(y) > threshold) {
      if (y < 0) cbs.onSwipeUp?.();
      else cbs.onSwipeDown?.();
    }

    setSwipeOffset({ x: 0, y: 0 });
    setSwipingDirection(null);
    setIsSwiping(false);
    isSwipingRef.current = false;
    lockedDirectionRef.current = null;
    startedFromEdgeRef.current = null;
  }, [threshold]);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    el.addEventListener('touchstart', handleTouchStart, { passive: true });
    el.addEventListener('touchmove', handleTouchMove, { passive: false });
    el.addEventListener('touchend', handleTouchEnd, { passive: true });

    return () => {
      el.removeEventListener('touchstart', handleTouchStart);
      el.removeEventListener('touchmove', handleTouchMove);
      el.removeEventListener('touchend', handleTouchEnd);
    };
  }, [handleTouchStart, handleTouchMove, handleTouchEnd]);

  return { swipeOffset, swipingDirection, isSwiping, ref };
}
