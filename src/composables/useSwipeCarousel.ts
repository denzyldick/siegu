import { ref, computed } from 'vue';

const AXIS_LOCK_THRESHOLD = 8;
const SNAP_THRESHOLD_RATIO = 0.25;
const VELOCITY_THRESHOLD = 0.5;
const ANIMATION_DURATION = 300;

type Phase = 'idle' | 'touching' | 'dragging' | 'animating';
type Axis = 'horizontal' | 'vertical' | null;

export interface SwipeCarouselOptions {
  totalItems: () => number;
  currentIndex: () => number;
  onNavigate: (index: number) => void;
  onVerticalSwipe?: (direction: 'up' | 'down') => void;
  onDragProgress?: (offset: number, progress: number) => void;
}

export function useSwipeCarousel(options: SwipeCarouselOptions) {
  const offset = ref(0);
  const phase = ref<Phase>('idle');
  const axis = ref<Axis>(null);
  const isAnimating = ref(false);
  const direction = ref<'next' | 'prev' | null>(null);

  let startX = 0;
  let startY = 0;
  let startTime = 0;
  let viewportWidth = 0;
  let animationFrame: number | null = null;

  const trackTransform = computed(() => {
    return `translateX(${-getViewportWidth() + offset.value}px)`;
  });

  function getViewportWidth(): number {
    return viewportWidth || window.innerWidth;
  }

  function wrapIndex(index: number): number {
    const total = options.totalItems();
    if (total === 0) return 0;
    return ((index % total) + total) % total;
  }

  function getPrevIndex(): number {
    return wrapIndex(options.currentIndex() - 1);
  }

  function getNextIndex(): number {
    return wrapIndex(options.currentIndex() + 1);
  }

  function animateTo(targetOffset: number, duration: number): Promise<void> {
    return new Promise((resolve) => {
      const startOffset = offset.value;
      const delta = targetOffset - startOffset;
      if (Math.abs(delta) < 0.5) {
        offset.value = targetOffset;
        resolve();
        return;
      }

      isAnimating.value = true;
      const startTime = performance.now();

      function tick(now: number) {
        const elapsed = now - startTime;
        const progress = Math.min(elapsed / duration, 1);
        // Approximate cubic-bezier(0.16, 1, 0.3, 1) with ease-out
        const eased = 1 - Math.pow(1 - progress, 3);
        offset.value = startOffset + delta * eased;

        if (progress < 1) {
          animationFrame = requestAnimationFrame(tick);
        } else {
          offset.value = targetOffset;
          isAnimating.value = false;
          animationFrame = null;
          resolve();
        }
      }

      if (animationFrame !== null) cancelAnimationFrame(animationFrame);
      animationFrame = requestAnimationFrame(tick);
    });
  }

  async function snapToNext(): Promise<void> {
    phase.value = 'animating';
    direction.value = 'next';
    await animateTo(-getViewportWidth(), ANIMATION_DURATION);
    options.onNavigate(getNextIndex());
    offset.value = 0;
    phase.value = 'idle';
    direction.value = null;
  }

  async function snapToPrev(): Promise<void> {
    phase.value = 'animating';
    direction.value = 'prev';
    await animateTo(getViewportWidth(), ANIMATION_DURATION);
    options.onNavigate(getPrevIndex());
    offset.value = 0;
    phase.value = 'idle';
    direction.value = null;
  }

  async function snapBack(): Promise<void> {
    phase.value = 'animating';
    await animateTo(0, ANIMATION_DURATION);
    phase.value = 'idle';
  }

  function onTouchStart(e: TouchEvent): void {
    if (phase.value === 'animating') {
      if (animationFrame !== null) cancelAnimationFrame(animationFrame);
      isAnimating.value = false;
      phase.value = 'idle';
      offset.value = 0;
    }

    const t = e.touches[0];
    startX = t.clientX;
    startY = t.clientY;
    startTime = performance.now();
    axis.value = null;
    phase.value = 'touching';
    viewportWidth = getViewportWidth();
  }

  function onTouchMove(e: TouchEvent): { axis: Axis; defaultPrevented: boolean } {
    if (phase.value !== 'touching' && phase.value !== 'dragging') {
      return { axis: null, defaultPrevented: false };
    }

    const t = e.touches[0];
    const dx = t.clientX - startX;
    const dy = t.clientY - startY;
    const absDx = Math.abs(dx);
    const absDy = Math.abs(dy);

    // Lock axis on first significant movement
    if (phase.value === 'touching') {
      if (absDx < AXIS_LOCK_THRESHOLD && absDy < AXIS_LOCK_THRESHOLD) {
        return { axis: null, defaultPrevented: false };
      }
      axis.value = absDx > absDy ? 'horizontal' : 'vertical';
      phase.value = 'dragging';

      if (axis.value === 'vertical') {
        // Let vertical gestures pass through to time period handler
        return { axis: 'vertical', defaultPrevented: false };
      }
    }

    if (axis.value === 'vertical') {
      return { axis: 'vertical', defaultPrevented: false };
    }

    // Horizontal drag — update offset
    if (axis.value === 'horizontal') {
      // Add resistance at edges
      const index = options.currentIndex();
      const total = options.totalItems();
      const atStart = index === 0 && dx > 0;
      const atEnd = index === total - 1 && dx < 0;
      const resistance = atStart || atEnd ? 0.3 : 1;

      offset.value = dx * resistance;

      if (options.onDragProgress) {
        const progress = offset.value / viewportWidth;
        options.onDragProgress(offset.value, progress);
      }

      return { axis: 'horizontal', defaultPrevented: true };
    }

    return { axis: null, defaultPrevented: false };
  }

  async function onTouchEnd(e: TouchEvent): Promise<void> {
    if (phase.value === 'touching') {
      // Was a tap, not a drag
      phase.value = 'idle';
      return;
    }

    if (phase.value !== 'dragging' || axis.value !== 'horizontal') {
      phase.value = 'idle';
      return;
    }

    const t = e.changedTouches[0];
    const dx = t.clientX - startX;
    const elapsed = performance.now() - startTime;
    const velocity = Math.abs(dx) / elapsed;
    const threshold = viewportWidth * SNAP_THRESHOLD_RATIO;

    if (Math.abs(dx) > threshold || velocity > VELOCITY_THRESHOLD) {
      if (dx < 0) {
        await snapToNext();
      } else {
        await snapToPrev();
      }
    } else {
      await snapBack();
    }
  }

  function animateNext(): void {
    if (phase.value === 'animating') return;
    void snapToNext();
  }

  function animatePrev(): void {
    if (phase.value === 'animating') return;
    void snapToPrev();
  }

  function reset(): void {
    if (animationFrame !== null) cancelAnimationFrame(animationFrame);
    offset.value = 0;
    phase.value = 'idle';
    axis.value = null;
    isAnimating.value = false;
    direction.value = null;
  }

  return {
    offset,
    phase,
    axis,
    isAnimating,
    direction,
    trackTransform,
    getPrevIndex,
    getNextIndex,
    onTouchStart,
    onTouchMove,
    onTouchEnd,
    animateNext,
    animatePrev,
    reset,
    wrapIndex,
  };
}
