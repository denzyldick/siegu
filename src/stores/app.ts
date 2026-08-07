import { defineStore } from 'pinia';
import { ref } from 'vue';
import {
  isInitialized,
  getOs,
  listDirectories,
  getLastScanTime,
  scanFiles,
} from '@/services/tauri';
import { listenEvent } from '@/services/events';

export const useAppStore = defineStore('app', () => {
  const initialized = ref(false);
  const isNewInstall = ref(false);
  const onboardingStep = ref(0);
  const showTour = ref(false);
  const os = ref('');
  const directories = ref<string[]>([]);
  const lastScanTime = ref<string>('Never');
  const lastScanTimestamp = ref(0);

  async function checkInitialized(): Promise<void> {
    try {
      const result = await isInitialized();
      initialized.value = true;
      isNewInstall.value = !result;
    } catch (error) {
      console.error('[AppStore] Failed to check initialization:', error);
      initialized.value = true;
      isNewInstall.value = true;
    }
  }

  async function detectOs(): Promise<void> {
    try {
      os.value = await getOs();
    } catch (error) {
      console.error('[AppStore] Failed to detect OS:', error);
    }
  }

  async function loadDirectories(): Promise<void> {
    try {
      directories.value = await listDirectories();
    } catch (error) {
      console.error('[AppStore] Failed to load directories:', error);
      directories.value = [];
    }
  }

  async function loadLastScanTime(): Promise<void> {
    try {
      const time = await getLastScanTime();
      lastScanTime.value = time;
      if (time !== 'Never') {
        lastScanTimestamp.value = parseInt(time, 10) || 0;
      }
    } catch (error) {
      console.error('[AppStore] Failed to load scan time:', error);
    }
  }

  async function startScan(): Promise<void> {
    try {
      await scanFiles();
    } catch (error) {
      console.error('[AppStore] Failed to start scan:', error);
    }
  }

  function completeOnboarding(): void {
    isNewInstall.value = false;
    onboardingStep.value = 0;
  }

  function startTour(): void {
    showTour.value = true;
  }

  function dismissTour(): void {
    showTour.value = false;
  }

  void listenEvent('scan-progress', () => {
    loadLastScanTime();
  });

  return {
    initialized,
    isNewInstall,
    onboardingStep,
    showTour,
    os,
    directories,
    lastScanTime,
    lastScanTimestamp,
    checkInitialized,
    detectOs,
    loadDirectories,
    loadLastScanTime,
    startScan,
    completeOnboarding,
    startTour,
    dismissTour,
  };
});
