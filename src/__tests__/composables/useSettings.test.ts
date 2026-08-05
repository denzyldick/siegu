import { describe, it, expect, beforeEach, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-os', () => ({
  platform: vi.fn(() => 'linux'),
}));

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: vi.fn(),
}));

vi.mock('vue-i18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

import { useSettings } from '@/composables/useSettings';

function configWith(overrides: Record<string, string>): string {
  const base: Record<string, string> = {
    scan_threads: '4',
    ml_threads: '2',
    batch_delay_ms: '0',
    ml_memory_budget_mb: '0',
    indexing_mode: 'immediate',
  };
  return JSON.stringify({ ...base, ...overrides });
}

describe('useSettings performance + memory', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(undefined);
  });

  it('loads performance config into the reactive object', async () => {
    invoke.mockResolvedValueOnce(
      configWith({
        scan_threads: '2',
        ml_threads: '1',
        batch_delay_ms: '200',
        ml_memory_budget_mb: '2048',
        indexing_mode: 'idle',
      }),
    );
    const settings = useSettings();
    await settings.loadPerformanceConfig();
    expect(settings.performance.scanThreads).toBe(2);
    expect(settings.performance.mlThreads).toBe(1);
    expect(settings.performance.batchDelayMs).toBe(200);
    expect(settings.performance.memoryBudgetMb).toBe(2048);
    expect(settings.performance.indexingMode).toBe('idle');
  });

  it('defaults to the balanced preset', async () => {
    invoke.mockResolvedValueOnce(configWith({}));
    const settings = useSettings();
    await settings.loadPerformanceConfig();
    expect(settings.currentPreset.value).toBe('balanced');
  });

  it('applyPreset(low) writes every performance key', async () => {
    invoke.mockResolvedValueOnce(configWith({}));
    const settings = useSettings();
    await settings.loadPerformanceConfig();
    await settings.applyPreset('low');
    expect(settings.performance.scanThreads).toBe(2);
    expect(settings.performance.mlThreads).toBe(1);
    expect(settings.performance.batchDelayMs).toBe(200);
    expect(settings.currentPreset.value).toBe('low');
    const saved = invoke.mock.calls.filter(([name]) => name === 'save_config');
    expect(saved).toHaveLength(4);
    expect(saved.map(([, args]) => [args.key, args.value])).toEqual([
      ['scan_threads', '2'],
      ['ml_threads', '1'],
      ['batch_delay_ms', '200'],
      ['ml_memory_budget_mb', '0'],
    ]);
  });

  it('manual tweaks flip the preset to custom', async () => {
    invoke.mockResolvedValueOnce(configWith({}));
    const settings = useSettings();
    await settings.loadPerformanceConfig();
    settings.performance.mlThreads = 3;
    expect(settings.currentPreset.value).toBe('custom');
  });

  it('freeMemory invokes unload_models and clears loaded state', async () => {
    invoke.mockResolvedValueOnce(true).mockResolvedValueOnce(undefined);
    const settings = useSettings();
    await settings.loadModelsLoaded();
    expect(settings.modelsLoaded.value).toBe(true);
    await settings.freeMemory();
    expect(invoke).toHaveBeenCalledWith('unload_models');
    expect(settings.modelsLoaded.value).toBe(false);
  });

  it('reports no models loaded when command is unavailable', async () => {
    invoke.mockRejectedValueOnce(new Error('command not found'));
    const settings = useSettings();
    await settings.loadModelsLoaded();
    expect(settings.modelsLoaded.value).toBe(false);
  });

  it('ram estimate counts only enabled downloaded models', async () => {
    const settings = useSettings();
    settings.modelEnabled.value = { clip: false, aesthetics: true, yolo: true };
    settings.downloadedModels.value = ['clip', 'aesthetics', 'yolo'];
    expect(settings.totalModelRamEstimate.value).toBe('1.6 GB');
  });
});
