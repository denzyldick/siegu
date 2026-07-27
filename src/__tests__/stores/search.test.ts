import { describe, it, expect, beforeEach } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { useSearchStore } from '@/stores/search'

describe('search store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
  })

  it('initializes with empty query', () => {
    const store = useSearchStore()
    expect(store.query).toBe('')
    expect(store.hasQuery).toBe(false)
  })

  it('hasQuery is true when query is set', () => {
    const store = useSearchStore()
    store.setQuery('dogs')
    expect(store.hasQuery).toBe(true)
  })

  it('clears query', () => {
    const store = useSearchStore()
    store.setQuery('dogs')
    store.clearQuery()
    expect(store.query).toBe('')
    expect(store.hasQuery).toBe(false)
  })

  it('manages recent searches', () => {
    const store = useSearchStore()
    store.addRecentSearch('cats')
    store.addRecentSearch('dogs')
    expect(store.recentSearches).toEqual(['dogs', 'cats'])
  })

  it('deduplicates recent searches', () => {
    const store = useSearchStore()
    store.addRecentSearch('cats')
    store.addRecentSearch('dogs')
    store.addRecentSearch('cats')
    expect(store.recentSearches).toEqual(['cats', 'dogs'])
  })

  it('limits recent searches to 10', () => {
    const store = useSearchStore()
    for (let i = 0; i < 15; i++) {
      store.addRecentSearch(`term${i}`)
    }
    expect(store.recentSearches.length).toBe(10)
    expect(store.recentSearches[0]).toBe('term14')
  })

  it('clears recent searches', () => {
    const store = useSearchStore()
    store.addRecentSearch('cats')
    store.clearRecentSearches()
    expect(store.recentSearches).toEqual([])
  })

  it('ignores empty search terms', () => {
    const store = useSearchStore()
    store.addRecentSearch('')
    store.addRecentSearch('   ')
    expect(store.recentSearches).toEqual([])
  })
})
