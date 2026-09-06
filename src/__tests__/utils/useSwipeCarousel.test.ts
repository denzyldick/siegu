import { describe, it, expect } from 'vitest';
import { useSwipeCarousel } from '@/composables/useSwipeCarousel';

function makeCarousel(currentIndex = 1, total = 5) {
  return useSwipeCarousel({
    totalItems: () => total,
    currentIndex: () => currentIndex,
    onNavigate: () => {},
  });
}

describe('useSwipeCarousel track layout', () => {
  it('opens translated onto the CENTRE slide before any touch', () => {
    const c = makeCarousel();
    expect(c.trackTransform.value).toBe(`translateX(-${window.innerWidth}px)`);
  });

  it('keeps the centre slide aligned at rest (offset 0)', () => {
    const c = makeCarousel();
    expect(c.offset.value).toBe(0);
    expect(c.trackTransform.value).toBe(`translateX(-${window.innerWidth}px)`);
  });

  it('wraps prev/next indices around the list', () => {
    const c = makeCarousel(0, 5);
    expect(c.getPrevIndex()).toBe(4);
    expect(c.getNextIndex()).toBe(1);
  });
});