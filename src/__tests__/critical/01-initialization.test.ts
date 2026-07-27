import { describe, it, expect, beforeEach, vi } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'

vi.mock('@/services/tauri', () => ({
  isInitialized: vi.fn(),
  getOs: vi.fn(),
  listDirectories: vi.fn(),
  getLastScanTime: vi.fn(),
  scanFiles: vi.fn(),
}))

import { isInitialized, getOs } from '@/services/tauri'
import { useAppStore } from '@/stores/app'

describe('critical path: app initialization', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
    vi.clearAllMocks()
  })

  it('starts in uninitialized state', () => {
    const store = useAppStore()
    expect(store.initialized).toBe(false)
    expect(store.isNewInstall).toBe(false)
  })

  it('checkInitialized sets initialized to true when app exists', async () => {
    vi.mocked(isInitialized).mockResolvedValue(true as never)

    const store = useAppStore()
    await store.checkInitialized()

    expect(store.initialized).toBe(true)
    expect(store.isNewInstall).toBe(false)
  })

  it('checkInitialized detects new install', async () => {
    vi.mocked(isInitialized).mockResolvedValue(false as never)

    const store = useAppStore()
    await store.checkInitialized()

    expect(store.initialized).toBe(true)
    expect(store.isNewInstall).toBe(true)
  })

  it('completeOnboarding clears new install state', async () => {
    vi.mocked(isInitialized).mockResolvedValue(false as never)

    const store = useAppStore()
    await store.checkInitialized()
    expect(store.isNewInstall).toBe(true)

    store.completeOnboarding()
    expect(store.isNewInstall).toBe(false)
  })

  it('detectOs sets the os value', async () => {
    vi.mocked(getOs).mockResolvedValue('linux' as never)

    const store = useAppStore()
    await store.detectOs()

    expect(store.os).toBe('linux')
  })
})
