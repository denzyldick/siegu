<script setup lang="ts">
import { useUiStore } from '@/stores/ui'
import logo from '@/assets/logo.png'

const uiStore = useUiStore()

const navItems = [
  { page: 'home' as const, icon: null, tour: 'dock-home', useLogo: true },
  { page: 'people' as const, icon: 'mdi-account-group-outline', tour: 'dock-people', useLogo: false },
  { page: 'location' as const, icon: 'mdi-map-outline', tour: 'dock-map', useLogo: false },
  { page: 'devices' as const, icon: 'mdi-laptop', tour: 'dock-devices', useLogo: false },
  { page: 'settings' as const, icon: 'mdi-cog-outline', tour: 'dock-settings', useLogo: false },
]

function navigate(page: 'home' | 'people' | 'location' | 'devices' | 'settings'): void {
  uiStore.setPage(page)
}
</script>

<template>
  <div class="dock-container">
    <v-sheet
      class="dock d-flex justify-space-around align-center pa-2 rounded-pill mb-8"
      elevation="0"
      width="100%"
      max-width="380"
      color="surface"
    >
      <v-btn
        v-for="item in navItems"
        :key="item.page"
        icon
        variant="text"
        size="small"
        class="siegu-dock-btn"
        :class="{ 'siegu-dock-btn--active': uiStore.currentPage === item.page }"
        :data-tour="item.tour"
        @click="navigate(item.page)"
      >
        <v-img
          v-if="item.useLogo"
          :src="logo"
          width="24"
          height="24"
          :class="uiStore.currentPage === item.page ? 'opacity-100' : 'opacity-40'"
        />
        <v-icon v-else size="24">{{ item.icon }}</v-icon>
      </v-btn>
    </v-sheet>
  </div>
</template>

<style scoped>
.dock-container {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  display: flex;
  justify-content: center;
  pointer-events: none;
  z-index: 2000;
}

.dock {
  pointer-events: auto;
  backdrop-filter: blur(16px);
  border: 1px solid var(--color-border-default);
}

.siegu-dock-btn {
  color: var(--color-text-muted) !important;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1) !important;
  border-radius: 50% !important;
}

.siegu-dock-btn:hover {
  background: var(--color-bg-hover) !important;
  color: var(--color-text-primary) !important;
  transform: translateY(-2px);
}

.siegu-dock-btn--active {
  color: var(--color-text-primary) !important;
  background: var(--color-bg-hover) !important;
}
</style>
