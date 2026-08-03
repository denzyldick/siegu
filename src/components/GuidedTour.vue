<template>
  <div class="tour-overlay" v-if="props.active" @click="skip">
    <div class="tour-spotlight" :style="spotlightStyle" v-if="targetRect"></div>
    <div class="tour-card" :class="cardPosition" @click.stop>
      <div class="tour-card-inner">
        <v-icon size="32" color="primary" class="mb-3">{{ currentStep.icon }}</v-icon>
        <h3 class="text-h6 font-weight-bold text-zinc-primary mb-1">{{ $t(currentStep.titleKey) }}</h3>
        <p class="text-body-2 text-zinc-secondary mb-6">{{ $t(currentStep.descKey) }}</p>
        <div class="d-flex align-center justify-space-between">
          <v-btn
            variant="text"
            size="small"
            class="text-zinc-muted font-weight-bold"
            @click="skip"
            >{{ $t('guided_tour.skip') }}</v-btn
          >
          <div class="d-flex align-center ga-1">
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
            @click="next"
          >
            {{ $t('guided_tour.next') }}
          </v-btn>
          <v-btn
            v-else
            color="primary"
            variant="flat"
            size="small"
            class="font-weight-bold px-6"
            @click="finish"
          >
            {{ $t('guided_tour.done') }}
          </v-btn>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onUnmounted } from 'vue'

interface Step {
  icon: string
  titleKey: string
  descKey: string
  target: string | null
  position: string
}

const props = withDefaults(defineProps<{
  active: boolean
  target?: string
}>(), {
  target: '',
})

const emit = defineEmits<{
  (e: 'finish'): void
  (e: 'skip'): void
  (e: 'update:target', value: string): void
}>()

const step = ref(0)
const targetRects = ref<Record<string, DOMRect>>({})
const observer = ref<ResizeObserver | null>(null)

const steps: Step[] = [
  {
    icon: 'mdi-walk',
    titleKey: 'guided_tour.welcome_title',
    descKey: 'guided_tour.welcome_desc',
    target: null,
    position: 'bottom',
  },
  {
    icon: 'mdi-magnify',
    titleKey: 'guided_tour.search_title',
    descKey: 'guided_tour.search_desc',
    target: "[data-tour='search']",
    position: 'top',
  },
  {
    icon: 'mdi-image-multiple-outline',
    titleKey: 'guided_tour.library_title',
    descKey: 'guided_tour.library_desc',
    target: "[data-tour='photos']",
    position: 'bottom',
  },
  {
    icon: 'mdi-magnify-scan',
    titleKey: 'guided_tour.scan_button_title',
    descKey: 'guided_tour.scan_button_desc',
    target: "[data-tour='scan-button']",
    position: 'bottom',
  },
  {
    icon: 'mdi-progress-check',
    titleKey: 'guided_tour.scan_progress_title',
    descKey: 'guided_tour.scan_progress_desc',
    target: "[data-tour='scan-progress']",
    position: 'bottom',
  },
  {
    icon: 'mdi-account-group-outline',
    titleKey: 'guided_tour.people_title',
    descKey: 'guided_tour.people_desc',
    target: "[data-tour='dock-people']",
    position: 'top',
  },
  {
    icon: 'mdi-map-outline',
    titleKey: 'guided_tour.map_title',
    descKey: 'guided_tour.map_desc',
    target: "[data-tour='dock-map']",
    position: 'top',
  },
  {
    icon: 'mdi-laptop',
    titleKey: 'guided_tour.devices_title',
    descKey: 'guided_tour.devices_desc',
    target: "[data-tour='dock-devices']",
    position: 'top',
  },
  {
    icon: 'mdi-cog-outline',
    titleKey: 'guided_tour.settings_title',
    descKey: 'guided_tour.settings_desc',
    target: "[data-tour='dock-settings']",
    position: 'top',
  },
  {
    icon: 'mdi-check-decagram',
    titleKey: 'guided_tour.done_title',
    descKey: 'guided_tour.done_desc',
    target: null,
    position: 'bottom',
  },
]

const currentStep = computed(() => steps[step.value])

const targetRect = computed(() => {
  const t = currentStep.value.target
  return t ? targetRects.value[t] ?? null : null
})

const pad = 8

const spotlightStyle = computed(() => {
  if (!targetRect.value) return { display: 'none' }
  const r = targetRect.value
  return {
    left: r.left - pad + 'px',
    top: r.top - pad + 'px',
    width: r.width + pad * 2 + 'px',
    height: r.height + pad * 2 + 'px',
  }
})

const cardPosition = computed(() => {
  return currentStep.value.position === 'top' ? 'tour-card--top' : 'tour-card--bottom'
})

function measureTarget() {
  const t = currentStep.value.target
  if (!t) return
  const el = document.querySelector(t)
  if (el) {
    targetRects.value[t] = el.getBoundingClientRect()
  }
}

function stopObserver() {
  if (observer.value) {
    observer.value.disconnect()
    observer.value = null
  }
}

function startObserver() {
  stopObserver()
  if (typeof ResizeObserver === 'undefined') return
  observer.value = new ResizeObserver(() => measureTarget())
  const t = currentStep.value.target
  if (t) {
    const el = document.querySelector(t)
    if (el) observer.value.observe(el)
  }
}

function next() {
  if (step.value < steps.length - 1) {
    step.value++
  }
}

function finish() {
  stopObserver()
  emit('finish')
}

function skip() {
  stopObserver()
  emit('skip')
}

watch(step, () => {
  nextTick(() => {
    measureTarget()
    startObserver()
  })
})

watch(
  () => props.active,
  (val) => {
    if (val) {
      step.value = 0
      nextTick(() => {
        measureTarget()
        startObserver()
      })
    } else {
      stopObserver()
    }
  },
)

onUnmounted(() => {
  stopObserver()
})
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
  border-radius: 12px;
  box-shadow: 0 0 0 2px white;
  pointer-events: none;
  z-index: 10000;
  transition: all 0.35s cubic-bezier(0.16, 1, 0.3, 1);
}
.tour-card {
  position: relative;
  z-index: 10001;
  width: 100%;
  max-width: 420px;
  margin: 16px;
  animation: tourSlideUp 0.35s cubic-bezier(0.16, 1, 0.3, 1);
}
.tour-card--top {
  margin-top: 60px;
  align-self: flex-start;
}
.tour-card--bottom {
  margin-bottom: 80px;
  align-self: flex-end;
}
.tour-card-inner {
  background: white;
  border-radius: 20px;
  padding: 24px;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.15);
}
@media (max-width: 480px) {
  .tour-card {
    margin: 8px;
    max-width: none;
  }
  .tour-card--top {
    margin-top: 40px;
  }
  .tour-card--bottom {
    margin-bottom: 70px;
  }
  .tour-card-inner {
    padding: 20px;
  }
}
.tour-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #d4d4d8;
  transition: all 0.2s ease;
}
.tour-dot--active {
  background: #18181b;
  transform: scale(1.3);
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
