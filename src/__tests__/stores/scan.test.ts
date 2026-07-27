import { describe, it, expect, beforeEach } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { useScanStore } from '@/stores/scan'

describe('scan store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('initializes with idle state', () => {
    const store = useScanStore()
    expect(store.status).toBe('idle')
    expect(store.scanning).toBe(false)
    expect(store.indexingCount).toBe(0)
    expect(store.isActive).toBe(false)
    expect(store.progress).toBe(0)
  })

  it('computes progress correctly', () => {
    const store = useScanStore()
    store.filesFound = 100
    store.filesProcessed = 50
    expect(store.progress).toBe(50)
  })

  it('returns 0 progress when no files found', () => {
    const store = useScanStore()
    store.filesFound = 0
    store.filesProcessed = 0
    expect(store.progress).toBe(0)
  })

  it('isActive is true when scanning', () => {
    const store = useScanStore()
    store.scanning = true
    expect(store.isActive).toBe(true)
  })

  it('isActive is true when indexing', () => {
    const store = useScanStore()
    store.status = 'indexing'
    expect(store.isActive).toBe(true)
  })

  it('isActive is false when idle', () => {
    const store = useScanStore()
    store.status = 'idle'
    store.scanning = false
    expect(store.isActive).toBe(false)
  })
})
