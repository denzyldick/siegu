<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import { useUiStore } from '@/stores/ui';
import { APP_LANDING_URL } from '@/services/appConfig';

const { t } = useI18n();
const uiStore = useUiStore();

function backToLibrary(): void {
  uiStore.setPage('home');
}
</script>

<template>
  <div class="guest-upsell">
    <div class="guest-upsell__inner">
      <img src="/og-logo.png" alt="siegu" class="guest-upsell__logo" />
      <h1 class="guest-upsell__title">{{ t('guest.upsell_title') }}</h1>
      <p class="guest-upsell__desc">{{ t('guest.upsell_desc') }}</p>

      <ul class="guest-upsell__features">
        <li>
          <v-icon start size="18" color="primary">mdi-image-multiple-outline</v-icon>
          <span>{{ t('guest.upsell_feature_1') }}</span>
        </li>
        <li>
          <v-icon start size="18" color="primary">mdi-lock-outline</v-icon>
          <span>{{ t('guest.upsell_feature_2') }}</span>
        </li>
        <li>
          <v-icon start size="18" color="primary">mdi-share-variant-outline</v-icon>
          <span>{{ t('guest.upsell_feature_3') }}</span>
        </li>
      </ul>

      <div class="guest-upsell__external">
        <div class="text-body-2 font-weight-bold">{{ t('guest.upsell_external') }}</div>
        <div class="text-body-2 text-medium-emphasis">{{ t('guest.upsell_external_desc') }}</div>
      </div>

      <a class="guest-upsell__cta" :href="APP_LANDING_URL" target="_blank" rel="noopener">
        {{ t('guest.upsell_cta') }}
        <v-icon end size="16">mdi-open-in-new</v-icon>
      </a>

      <button type="button" class="guest-upsell__close" @click="backToLibrary">
        {{ t('guest.upsell_close') }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.guest-upsell {
  min-height: calc(100vh - 120px);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}
.guest-upsell__inner {
  max-width: 460px;
  width: 100%;
  text-align: center;
}
.guest-upsell__logo {
  width: 56px;
  height: 56px;
  object-fit: contain;
  margin: 0 auto 12px;
  display: block;
}
.guest-upsell__title {
  font-size: 26px;
  font-weight: 700;
  margin: 0 0 10px;
  color: rgb(var(--v-theme-on-surface));
}
.guest-upsell__desc {
  font-size: 15px;
  line-height: 1.5;
  color: rgb(var(--v-theme-on-surface), 0.7);
  margin: 0 0 28px;
}
.guest-upsell__features {
  list-style: none;
  margin: 0 0 28px;
  padding: 0;
  text-align: left;
}
.guest-upsell__features li {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 0;
  border-bottom: 1px solid rgba(var(--v-theme-on-surface), 0.08);
  font-size: 15px;
  color: rgb(var(--v-theme-on-surface), 0.85);
}
.guest-upsell__external {
  background: rgba(var(--v-theme-primary), 0.08);
  border: 1px solid rgba(var(--v-theme-primary), 0.2);
  border-radius: 14px;
  padding: 14px 16px;
  margin-bottom: 20px;
}
.guest-upsell__cta {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  justify-content: center;
  padding: 13px;
  border-radius: 12px;
  background: rgb(var(--v-theme-primary));
  color: rgb(var(--v-theme-on-primary));
  font-size: 15px;
  font-weight: 700;
  text-decoration: none;
  transition: filter 0.2s ease;
}
.guest-upsell__cta:hover {
  filter: brightness(0.92);
}
.guest-upsell__close {
  margin-top: 16px;
  background: none;
  border: none;
  color: rgb(var(--v-theme-on-surface), 0.6);
  font-size: 14px;
  cursor: pointer;
  text-decoration: underline;
}
</style>
