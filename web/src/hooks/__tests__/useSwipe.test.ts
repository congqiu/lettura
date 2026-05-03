// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { extractUrl } from '../../pages/ShareTargetPage';

describe('extractUrl', () => {
  it('returns url param when valid', () => {
    expect(extractUrl('https://example.com/article', null)).toBe('https://example.com/article');
  });

  it('extracts URL from text param when url is null', () => {
    expect(extractUrl(null, 'Check this out https://example.com/article great read')).toBe('https://example.com/article');
  });

  it('extracts URL from text param when url is empty', () => {
    expect(extractUrl('', 'https://example.com/article')).toBe('https://example.com/article');
  });

  it('returns null when no valid URL in either param', () => {
    expect(extractUrl(null, 'just some text')).toBeNull();
  });

  it('returns null when both params are null', () => {
    expect(extractUrl(null, null)).toBeNull();
  });

  it('rejects non-http protocols', () => {
    expect(extractUrl('ftp://example.com/file', null)).toBeNull();
  });

  it('accepts http protocol', () => {
    expect(extractUrl('http://example.com/article', null)).toBe('http://example.com/article');
  });

  it('extracts first URL from text with multiple URLs', () => {
    const text = 'https://first.com/a and https://second.com/b';
    expect(extractUrl(null, text)).toBe('https://first.com/a');
  });

  it('handles URL with query params', () => {
    expect(extractUrl('https://example.com/article?id=123&ref=share', null))
      .toBe('https://example.com/article?id=123&ref=share');
  });
});

// Test useSwipe core logic (pure functions matching hook behavior)
const DIRECTION_LOCK_THRESHOLD = 10;

interface SwipeState {
  touchStart: { x: number; y: number };
  lockedDirection: 'h' | 'v' | null;
  currentOffset: { x: number; y: number };
  startedFromEdge: 'left' | 'right' | null;
}

function createSwipeState(): SwipeState {
  return { touchStart: { x: 0, y: 0 }, lockedDirection: null, currentOffset: { x: 0, y: 0 }, startedFromEdge: null };
}

function processTouchStart(state: SwipeState, clientX: number, clientY: number, edgeStart?: number) {
  if (edgeStart !== undefined) {
    const screenWidth = 375;
    if (clientX <= edgeStart) {
      state.startedFromEdge = 'left';
    } else if (clientX >= screenWidth - edgeStart) {
      state.startedFromEdge = 'right';
    } else {
      state.startedFromEdge = null;
    }
  } else {
    state.startedFromEdge = null;
  }
  state.touchStart = { x: clientX, y: clientY };
  state.lockedDirection = null;
}

function processTouchMove(state: SwipeState, clientX: number, clientY: number, direction: 'horizontal' | 'vertical' | 'all' = 'horizontal') {
  const dx = clientX - state.touchStart.x;
  const dy = clientY - state.touchStart.y;

  if (!state.lockedDirection) {
    if (Math.abs(dx) > DIRECTION_LOCK_THRESHOLD || Math.abs(dy) > DIRECTION_LOCK_THRESHOLD) {
      if (Math.abs(dx) > Math.abs(dy)) {
        if (direction === 'vertical') return null;
        state.lockedDirection = 'h';
      } else {
        if (direction === 'horizontal') return null;
        state.lockedDirection = 'v';
      }
    } else {
      return null;
    }
  }

  if (state.lockedDirection === 'h') {
    const offset = { x: dx, y: 0 };
    state.currentOffset = offset;
    return { offset, direction: dx > 0 ? 'right' as const : 'left' as const };
  } else {
    const offset = { x: 0, y: dy };
    state.currentOffset = offset;
    return { offset, direction: dy > 0 ? 'down' as const : 'up' as const };
  }
}

function processTouchEnd(state: SwipeState, threshold: number): { triggered: boolean; callback: 'onSwipeLeft' | 'onSwipeRight' | 'onEdgeSwipeLeft' | 'onEdgeSwipeRight' | 'onSwipeUp' | 'onSwipeDown' | null } {
  const { x, y } = state.currentOffset;
  const edge = state.startedFromEdge;

  if (Math.abs(x) > threshold) {
    if (x < 0) {
      return { triggered: true, callback: edge === 'right' ? 'onEdgeSwipeLeft' : 'onSwipeLeft' };
    } else {
      return { triggered: true, callback: edge === 'left' ? 'onEdgeSwipeRight' : 'onSwipeRight' };
    }
  }
  if (Math.abs(y) > threshold) {
    return { triggered: true, callback: y < 0 ? 'onSwipeUp' : 'onSwipeDown' };
  }
  return { triggered: false, callback: null };
}

describe('useSwipe logic', () => {
  it('triggers onSwipeRight for right swipe from center', () => {
    const state = createSwipeState();
    processTouchStart(state, 100, 200);
    processTouchMove(state, 150, 200);
    processTouchMove(state, 200, 200);
    const result = processTouchEnd(state, 80);
    expect(result.triggered).toBe(true);
    expect(result.callback).toBe('onSwipeRight');
  });

  it('triggers onSwipeLeft for left swipe from center', () => {
    const state = createSwipeState();
    processTouchStart(state, 200, 200);
    processTouchMove(state, 150, 200);
    processTouchMove(state, 100, 200);
    const result = processTouchEnd(state, 80);
    expect(result.triggered).toBe(true);
    expect(result.callback).toBe('onSwipeLeft');
  });

  it('does not trigger when distance is below threshold', () => {
    const state = createSwipeState();
    processTouchStart(state, 100, 200);
    processTouchMove(state, 140, 200);
    const result = processTouchEnd(state, 80);
    expect(result.triggered).toBe(false);
  });

  it('triggers onEdgeSwipeRight when swiping right from left edge', () => {
    const state = createSwipeState();
    processTouchStart(state, 20, 200, 30);
    processTouchMove(state, 70, 200);
    processTouchMove(state, 120, 200);
    const result = processTouchEnd(state, 80);
    expect(result.triggered).toBe(true);
    expect(result.callback).toBe('onEdgeSwipeRight');
  });

  it('triggers onSwipeRight (not edge) when swiping right from center with edgeStart set', () => {
    const state = createSwipeState();
    processTouchStart(state, 100, 200, 30);
    processTouchMove(state, 150, 200);
    processTouchMove(state, 200, 200);
    const result = processTouchEnd(state, 80);
    expect(result.triggered).toBe(true);
    expect(result.callback).toBe('onSwipeRight');
  });

  it('triggers onEdgeSwipeLeft when swiping left from right edge', () => {
    const state = createSwipeState();
    processTouchStart(state, 360, 200, 30);
    processTouchMove(state, 310, 200);
    processTouchMove(state, 260, 200);
    const result = processTouchEnd(state, 80);
    expect(result.triggered).toBe(true);
    expect(result.callback).toBe('onEdgeSwipeLeft');
  });

  it('triggers onSwipeDown for vertical swipe', () => {
    const state = createSwipeState();
    processTouchStart(state, 200, 100);
    processTouchMove(state, 200, 150, 'vertical');
    processTouchMove(state, 200, 200, 'vertical');
    const result = processTouchEnd(state, 60);
    expect(result.triggered).toBe(true);
    expect(result.callback).toBe('onSwipeDown');
  });

  it('ignores horizontal swipes when direction is vertical', () => {
    const state = createSwipeState();
    processTouchStart(state, 100, 200);
    const result = processTouchMove(state, 200, 200, 'vertical');
    expect(result).toBeNull();
  });

  it('locks direction after moving past threshold', () => {
    const state = createSwipeState();
    processTouchStart(state, 100, 200);
    const result1 = processTouchMove(state, 120, 200);
    expect(result1?.direction).toBe('right');
    expect(state.lockedDirection).toBe('h');
  });

  it('ignores short movements below direction lock threshold', () => {
    const state = createSwipeState();
    processTouchStart(state, 100, 200);
    const result = processTouchMove(state, 105, 200);
    expect(result).toBeNull();
    expect(state.lockedDirection).toBeNull();
  });
});