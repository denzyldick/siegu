import { describe, it, expect, beforeEach, vi } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';
import { useScanStore } from '@/stores/scan';

const handlers = vi.hoisted(() => new Map<string, (payload: unknown) => void>());

vi.mock('@/services/events', () => ({
  listenEvent: vi.fn((eventName: string, handler: (payload: unknown) => void) => {
    handlers.set(eventName, handler);
    return Promise.resolve(() => {});
  }),
}));

function emit(event: string, payload: unknown): void {
  handlers.get(event)!(payload);
}

describe('scan store', () => {
  beforeEach(() => {
    handlers.clear();
    setActivePinia(createPinia());
  });

  it('initializes with idle state', () => {
    const store = useScanStore();
    expect(store.status).toBe('idle');
    expect(store.scanning).toBe(false);
    expect(store.indexingCount).toBe(0);
    expect(store.isActive).toBe(false);
    expect(store.progress).toBe(0);
  });

  it('computes progress correctly', () => {
    const store = useScanStore();
    store.filesFound = 100;
    store.filesProcessed = 50;
    expect(store.progress).toBe(50);
  });

  it('returns 0 progress when no files found', () => {
    const store = useScanStore();
    store.filesFound = 0;
    store.filesProcessed = 0;
    expect(store.progress).toBe(0);
  });

  it('isActive is true when scanning', () => {
    const store = useScanStore();
    store.scanning = true;
    expect(store.isActive).toBe(true);
  });

  it('isActive is true when indexing', () => {
    const store = useScanStore();
    store.status = 'indexing';
    expect(store.isActive).toBe(true);
  });

  it('isActive is false when idle', () => {
    const store = useScanStore();
    store.status = 'idle';
    store.scanning = false;
    expect(store.isActive).toBe(false);
  });

  it('starts scanning on scan-progress discovering', () => {
    const store = useScanStore();
    emit('scan-progress', { status: 'discovering', progress: 10, current: 1, total: 5 });
    expect(store.status).toBe('scanning');
    expect(store.scanning).toBe(true);
    expect(store.isActive).toBe(true);
  });

  it('switches to indexing on scan-progress indexing', () => {
    const store = useScanStore();
    emit('scan-progress', { status: 'indexing', progress: 100 });
    expect(store.status).toBe('indexing');
    expect(store.scanning).toBe(false);
  });

  it('does not get stuck when scan-progress complete has no file counts', () => {
    const store = useScanStore();
    emit('scan-progress', { status: 'indexing', progress: 100 });
    expect(store.isActive).toBe(true);
    emit('scan-progress', { status: 'complete', progress: 100 });
    expect(store.status).toBe('completed');
    expect(store.scanning).toBe(false);
    expect(store.isActive).toBe(false);
  });

  it('keeps file counts when scan-progress carries them', () => {
    const store = useScanStore();
    emit('scan-progress', {
      status: 'discovering',
      files_found: 200,
      files_processed: 50,
      current_file: '/a.jpg',
    });
    expect(store.filesFound).toBe(200);
    expect(store.filesProcessed).toBe(50);
    expect(store.currentFile).toBe('/a.jpg');
  });

  it('shows indexing state and remaining count from indexing-job running', () => {
    const store = useScanStore();
    emit('indexing-job', { status: 'running', completed: 3, total: 10 });
    expect(store.status).toBe('indexing');
    expect(store.scanning).toBe(false);
    expect(store.indexingCount).toBe(7);
    expect(store.isActive).toBe(true);
  });

  it('clears indexing state on indexing-job idle', () => {
    const store = useScanStore();
    emit('indexing-job', { status: 'running', completed: 3, total: 10 });
    expect(store.isActive).toBe(true);
    emit('indexing-job', { status: 'idle' });
    expect(store.status).toBe('completed');
    expect(store.scanning).toBe(false);
    expect(store.isActive).toBe(false);
    expect(store.indexingEta).toBeNull();
  });

  it('reads remaining count from indexing-progress', () => {
    const store = useScanStore();
    emit('indexing-progress', { remaining: 42 });
    expect(store.status).toBe('indexing');
    expect(store.indexingCount).toBe(42);
  });

  it('reads eta from indexing-eta', () => {
    const store = useScanStore();
    emit('indexing-eta', { eta: 5000 });
    expect(store.indexingEta).toBe(5000);
  });

  it('stop sets state to completed and clears counters', async () => {
    const store = useScanStore();
    emit('indexing-job', { status: 'running', completed: 3, total: 10 });
    await store.stop();
    expect(store.status).toBe('completed');
    expect(store.scanning).toBe(false);
    expect(store.indexingCount).toBe(0);
    expect(store.indexingEta).toBeNull();
    expect(store.stoppedMessage).toBe(true);
    expect(store.isActive).toBe(false);
  });
});
