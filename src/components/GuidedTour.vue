<template>
  <div class="tour-overlay" v-if="active" @click="skip">
    <div class="tour-spotlight" :style="spotlightStyle" v-if="targetRect"></div>
    <div class="tour-card" :class="cardPosition" @click.stop>
      <div class="tour-card-inner">
        <v-icon size="32" color="black" class="mb-3">{{ currentStep.icon }}</v-icon>
        <h3 class="text-h6 font-weight-bold text-zinc-primary mb-1">{{ currentTitle }}</h3>
        <p class="text-body-2 text-zinc-secondary mb-6">{{ currentDescription }}</p>
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
              v-for="(s, i) in steps"
              :key="i"
              class="tour-dot"
              :class="{ 'tour-dot--active': i === step }"
            ></span>
          </div>
          <v-btn
            v-if="step < steps.length - 1"
            color="black"
            variant="flat"
            size="small"
            class="font-weight-bold px-6"
            @click="next"
          >
            {{ $t('guided_tour.next') }}
          </v-btn>
          <v-btn
            v-else
            color="black"
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

<script>
export default {
  name: 'GuidedTour',
  props: {
    active: Boolean,
    target: String,
  },
  emits: ['finish', 'skip', 'update:target'],
  data: () => ({
    step: 0,
    targetRects: {},
    observer: null,
    steps: [
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
    ],
  }),
  computed: {
    currentStep() {
      return this.steps[this.step];
    },
    currentTitle() {
      return this.$t(this.currentStep.titleKey);
    },
    currentDescription() {
      return this.$t(this.currentStep.descKey);
    },
    targetRect() {
      const t = this.currentStep.target;
      return t ? this.targetRects[t] : null;
    },
    pad() {
      return 8;
    },
    spotlightStyle() {
      if (!this.targetRect) return { display: 'none' };
      const r = this.targetRect;
      const p = this.pad;
      return {
        left: r.left - p + 'px',
        top: r.top - p + 'px',
        width: r.width + p * 2 + 'px',
        height: r.height + p * 2 + 'px',
      };
    },
    cardPosition() {
      return this.currentStep.position === 'top' ? 'tour-card--top' : 'tour-card--bottom';
    },
  },
  watch: {
    step() {
      this.$nextTick(() => this.measureTarget());
    },
    active(val) {
      if (val) {
        this.step = 0;
        this.$nextTick(() => this.measureTarget());
        this.startObserver();
      } else {
        this.stopObserver();
      }
    },
  },
  methods: {
    measureTarget() {
      const t = this.currentStep.target;
      if (!t) return;
      const el = document.querySelector(t);
      if (el) {
        this.targetRects[t] = el.getBoundingClientRect();
      }
    },
    startObserver() {
      this.observer = new ResizeObserver(() => this.measureTarget());
      const t = this.currentStep.target;
      if (t) {
        const el = document.querySelector(t);
        if (el) this.observer.observe(el);
      }
    },
    stopObserver() {
      if (this.observer) {
        this.observer.disconnect();
        this.observer = null;
      }
    },
    next() {
      if (this.step < this.steps.length - 1) {
        this.step++;
      }
    },
    finish() {
      this.stopObserver();
      this.$emit('finish');
    },
    skip() {
      this.stopObserver();
      this.$emit('skip');
    },
  },
};
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
