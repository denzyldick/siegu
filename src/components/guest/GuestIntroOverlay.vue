<script setup lang="ts">
import { ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { APP_LANDING_URL, APP_GITHUB_URL, APP_DOCS_URL } from '@/services/appConfig';

const props = defineProps<{ forceClose?: boolean }>();

const { t } = useI18n();
const revealed = ref(false);

function goWatch(): void {
  revealed.value = true;
}

// Force the overlay closed when the guest session ends (expiry / host close).
watch(
  () => props.forceClose,
  (v) => {
    if (v) revealed.value = true;
  },
);
</script>

<template>
  <div :class="['guest-intro', { 'guest-intro--revealed': revealed }]">
    <div class="guest-intro__blur" aria-hidden="true"></div>
    <div class="guest-intro__card" :class="{ 'guest-intro__card--gone': revealed }">
      <svg class="guest-intro__logo" viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <rect x="2" y="2" width="20" height="20" rx="5" fill="rgb(var(--v-theme-primary))" />
        <circle cx="9" cy="9.5" r="2.2" fill="rgb(var(--v-theme-on-primary))" />
        <path
          d="M4.5 18.5 10 12l3.2 3.4 2.3-2.3 4 4.9"
          stroke="rgb(var(--v-theme-on-primary))"
          stroke-width="1.6"
          stroke-linecap="round"
          stroke-linejoin="round"
          fill="none"
        />
      </svg>
      <h2 class="guest-intro__title">{{ t('guest.intro_title') }}</h2>
      <p class="guest-intro__desc">{{ t('guest.intro_desc') }}</p>

      <div class="guest-intro__platform">
        <span class="guest-intro__platform-label">{{ t('guest.intro_learn_more') }}</span>
        <div class="guest-intro__links">
          <a :href="APP_LANDING_URL" target="_blank" rel="noopener">siegu.io</a>
          <a :href="APP_GITHUB_URL" target="_blank" rel="noopener">GitHub</a>
          <a :href="APP_DOCS_URL" target="_blank" rel="noopener">Docs</a>
        </div>
      </div>

      <button type="button" class="guest-intro__go" @click="goWatch">
        {{ t('guest.intro_go_watch') }}
      </button>
      <p class="guest-intro__note">{{ t('guest.intro_note') }}</p>
    </div>
  </div>
</template>

<style scoped>
.guest-intro {
  position: fixed;
  inset: 0;
  z-index: 3000;
  pointer-events: none;
}
.guest-intro__blur {
  position: absolute;
  inset: 0;
  backdrop-filter: blur(14px);
  background: rgb(var(--v-theme-background), 0.55);
  transition: opacity 0.35s ease;
}
.guest-intro--revealed .guest-intro__blur {
  opacity: 0;
}
.guest-intro__card {
  position: relative;
  z-index: 1;
  max-width: 460px;
  margin: 0 auto;
  top: 50%;
  transform: translateY(-50%);
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgba(var(--v-theme-on-surface), 0.1);
  border-radius: 24px;
  padding: 40px 32px;
  text-align: center;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  pointer-events: auto;
  transition:
    opacity 0.3s ease,
    transform 0.3s ease;
}
.guest-intro__card--gone {
  opacity: 0;
  transform: translateY(-50%) scale(0.96);
  pointer-events: none;
}
.guest-intro__logo {
  width: 64px;
  height: 64px;
  object-fit: contain;
  margin: 0 auto 12px;
  display: block;
}
.guest-intro__title {
  font-size: 22px;
  font-weight: 700;
  line-height: 1.25;
  margin: 0 0 12px;
  color: rgb(var(--v-theme-on-surface));
}
.guest-intro__desc {
  font-size: 14px;
  line-height: 1.5;
  color: rgb(var(--v-theme-on-surface), 0.7);
  margin: 0 0 20px;
}
.guest-intro__platform {
  margin-bottom: 24px;
}
.guest-intro__platform-label {
  display: block;
  font-size: 12px;
  color: rgb(var(--v-theme-on-surface), 0.5);
  margin-bottom: 8px;
}
.guest-intro__links {
  display: flex;
  gap: 16px;
  justify-content: center;
}
.guest-intro__links a {
  font-size: 13px;
  font-weight: 600;
  color: rgb(var(--v-theme-primary));
  text-decoration: none;
}
.guest-intro__go {
  width: 100%;
  padding: 12px;
  border: none;
  border-radius: 12px;
  background: rgb(var(--v-theme-primary));
  color: rgb(var(--v-theme-on-primary));
  font-size: 15px;
  font-weight: 700;
  cursor: pointer;
  transition: filter 0.2s ease;
}
.guest-intro__go:hover {
  filter: brightness(0.92);
}
.guest-intro__note {
  font-size: 11px;
  color: rgb(var(--v-theme-on-surface), 0.45);
  margin: 14px 0 0;
}
</style>
