import { describe, it, expect, beforeEach } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { useSearchStore } from '@/stores/search'

describe('critical path: search returns results', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
  })

  it('query updates correctly', () => {
    const store = useSearchStore()
    store.setQuery('dog')
    expect(store.query).toBe('dog')
    expect(store.hasQuery).toBe(true)
  })

  it('clear returns to empty', () => {
    const store = useSearchStore()
    store.setQuery('dog')
    store.clearQuery()
    expect(store.query).toBe('')
    expect(store.hasQuery).toBe(false)
  })

  it('recent searches persist across sessions', () => {
    const store = useSearchStore()
    store.addRecentSearch('cats')
    store.addRecentSearch('dogs')

    const stored = localStorage.getItem('siegu_recent_searches')
    expect(stored).toBeTruthy()
    const parsed = JSON.parse(stored!)
    expect(parsed).toContain('cats')
    expect(parsed).toContain('dogs')
  })

  it('loads recent searches from localStorage', () => {
    localStorage.setItem('siegu_recent_searches', JSON.stringify(['saved1', 'saved2']))

    const store = useSearchStore()
    expect(store.recentSearches).toContain('saved1')
    expect(store.recentSearches).toContain('saved2')
  })
})
