<template>
  <div class="tour-overlay" v-if="props.active" @click="skip">
    <div class="tour-spotlight" :style="spotlightStyle" v-if="targetRect"></div>
    <div class="tour-card" :style="cardStyle" @click.stop>
      <div class="tour-card-inner">
        <v-icon size="32" color="primary" class="mb-3">{{ currentStep.icon }}</v-icon>
        <h3 class="text-h6 font-weight-bold text-high-emphasis mb-1">
          {{ $t(currentStep.titleKey) }}
        </h3>
        <p class="text-body-2 text-medium-emphasis mb-6">{{ $t(currentStep.descKey) }}</p>
        <div class="d-flex align-center justify-space-between tour-footer">
          <v-btn
            variant="text"
            size="small"
            class="text-disabled font-weight-bold"
            @click.stop="skip"
            >{{ $t('guided_tour.skip') }}</v-btn
          >
          <div class="d-flex align-center ga-1 tour-dots">
            <span
              v-for="(_, i) in steps"
              :key="i"
              class="tour-dot"
              :class="{ 'tour-dot--active': i === step }"
            ></span>
          </div>
          <v-btn
            v-if="step < steps.length - 1"
            color="primary"
            variant="flat"
            size="small"
            class="font-weight-bold px-6"
            @click.stop="next"
          >
            {{ $t('guided_tour.next') }}
          </v-btn>
          <v-btn
            v-else
            color="primary"
            variant="flat"
            size="small"
            class="font-weight-bold px-6"
            @click.stop="finish"
          >
            {{ $t('guided_tour.done') }}
          </v-btn>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onUnmounted } from 'vue';
import { defaultTourSteps, type Step } from './GuidedTourSteps';

const props = withDefaults(
  defineProps<{
    active: boolean;
    target?: string;
    steps?: Step[];
  }>(),
  {
    target: '',
    steps: undefined,
  },
);

const emit = defineEmits<{
  (e: 'finish'): void;
  (e: 'skip'): void;
  (e: 'update:target', value: string): void;
}>();

const step = ref(0);
const targetRects = ref<Record<string, DOMRect>>({});
const observer = ref<ResizeObserver | null>(null);
let scrollSettledTimer: ReturnType<typeof setTimeout> | null = null;

const steps = computed<Step[]>(() => props.steps ?? defaultTourSteps);

const currentStep = computed(() => steps.value[step.value]);

const targetRect = computed(() => {
  const t = currentStep.value.target;
  return t ? (targetRects.value[t] ?? null) : null;
});

const pad = 8;

const spotlightStyle = computed(() => {
  if (!targetRect.value) return { display: 'none' };
  const r = targetRect.value;
  return {
    left: r.left - pad + 'px',
    top: r.top - pad + 'px',
    width: r.width + pad * 2 + 'px',
    height: r.height + pad * 2 + 'px',
  };
});

const cardStyle = computed(() => {
  const horizontal = { left: '12px', right: '12px', maxWidth: '420px', margin: '0 auto' };
  const r = targetRect.value;
  const vh = typeof window !== 'undefined' ? window.innerHeight : 0;
  if (!r) {
    // Intro / done steps: show a clean centered card at the bottom.
    return {
      ...horizontal,
      bottom: '24px',
      top: 'auto',
    };
  }
  const cardH = 260;
  const gap = 16;
  const below = vh - r.bottom - gap;
  const above = r.top - gap;
  if (above >= cardH) {
    // Enough room above the target: place the card above the spotlight.
    return { ...horizontal, bottom: `${vh - r.top + gap}px`, top: 'auto' };
  }
  if (below >= cardH) {
    // Enough room below the target: place the card below the spotlight.
    return { ...horizontal, top: `${r.bottom + gap}px`, bottom: 'auto' };
  }
  // Not enough room around a tall highlighted section: bottom sheet.
  return { ...horizontal, bottom: '24px', top: 'auto' };
});

function measureTarget() {
  const t = currentStep.value.target;
  if (!t) return;
  const el = document.querySelector(t);
  if (el) {
    targetRects.value[t] = el.getBoundingClientRect();
  }
}

function scrollTargetIntoView() {
  const t = currentStep.value.target;
  if (!t) return;
  const el = document.querySelector(t);
  if (el) {
    el.scrollIntoView({ behavior: 'smooth', block: 'center' });
  }
}

function positionStep() {
  scrollTargetIntoView();
  measureTarget();
  startObserver();
  // Re-measure once the smooth scroll has settled so the spotlight and
  // tooltip card end up at the correct on-screen position (smooth scroll is
  // asynchronous, so the immediate measure reflects the pre-scroll layout).
  if (scrollSettledTimer) clearTimeout(scrollSettledTimer);
  scrollSettledTimer = setTimeout(() => {
    measureTarget();
    startObserver();
  }, 500);
}

function stopObserver() {
  if (observer.value) {
    observer.value.disconnect();
    observer.value = null;
  }
}

function startObserver() {
  stopObserver();
  if (typeof ResizeObserver === 'undefined') return;
  observer.value = new ResizeObserver(() => measureTarget());
  const t = currentStep.value.target;
  if (t) {
    const el = document.querySelector(t);
    if (el) observer.value.observe(el);
  }
}

function next() {
  if (step.value < steps.value.length - 1) {
    step.value++;
  }
}

function finish() {
  stopObserver();
  emit('finish');
}

function skip() {
  stopObserver();
  emit('skip');
}

watch(step, () => {
  nextTick(() => {
    positionStep();
  });
});

watch(
  () => props.active,
  (val) => {
    if (val) {
      step.value = 0;
      nextTick(() => {
        positionStep();
      });
    } else {
      stopObserver();
    }
  },
);

onUnmounted(() => {
  stopObserver();
  if (scrollSettledTimer) clearTimeout(scrollSettledTimer);
});
</script>

<style scoped>
.tour-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  background: rgba(0, 0, 0, 0.4);
}
.tour-spotlight {
  position: fixed;
  border-radius: 8px;
  box-shadow: 0 0 0 2px white;
  pointer-events: none;
  z-index: 10000;
  transition: all 0.35s cubic-bezier(0.16, 1, 0.3, 1);
}
.tour-card {
  position: fixed;
  z-index: 10001;
  animation: tourSlideUp 0.35s cubic-bezier(0.16, 1, 0.3, 1);
}
.tour-card-inner {
  background: rgb(var(--v-theme-surface));
  border-radius: 24px;
  padding: 24px;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.15);
}
.tour-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.tour-dots {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
  overflow: hidden;
  flex-wrap: nowrap;
}

.tour-dot {
  flex: 0 0 auto;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: rgba(var(--v-theme-on-surface), 0.6);
  transition: all 0.2s ease;
}
.tour-dot--active {
  background: rgb(var(--v-theme-on-surface));
  transform: scale(1.3);
}

@media (max-width: 480px) {
  .tour-card-inner {
    padding: 20px;
  }
  .tour-footer {
    flex-wrap: wrap;
    row-gap: 8px;
  }
  .tour-dots {
    order: 2;
    width: 100%;
    justify-content: center;
    flex-wrap: nowrap;
  }
  .tour-dot {
    width: 7px;
    height: 7px;
    margin: 0 2px;
  }
  .tour-footer > .v-btn {
    order: 3;
  }
  .tour-footer > .v-btn:first-child {
    order: 1;
  }
}
@keyframes tourSlideUp {
  from {
    opacity: 0;
    transform: translateY(20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
