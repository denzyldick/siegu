import { describe, it, expect, beforeEach } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';
import { useSearchStore } from '@/stores/search';

describe('search store', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    localStorage.clear();
  });

  it('initializes with empty query', () => {
    const store = useSearchStore();
    expect(store.query).toBe('');
    expect(store.hasQuery).toBe(false);
  });

  it('hasQuery is true when query is set', () => {
    const store = useSearchStore();
    store.setQuery('dogs');
    expect(store.hasQuery).toBe(true);
  });

  it('clears query', () => {
    const store = useSearchStore();
    store.setQuery('dogs');
    store.clearQuery();
    expect(store.query).toBe('');
    expect(store.hasQuery).toBe(false);
  });

  it('manages recent searches', () => {
    const store = useSearchStore();
    store.addRecentSearch('cats');
    store.addRecentSearch('dogs');
    expect(store.recentSearches).toEqual(['dogs', 'cats']);
  });

  it('deduplicates recent searches', () => {
    const store = useSearchStore();
    store.addRecentSearch('cats');
    store.addRecentSearch('dogs');
    store.addRecentSearch('cats');
    expect(store.recentSearches).toEqual(['cats', 'dogs']);
  });

  it('limits recent searches to 10', () => {
    const store = useSearchStore();
    for (let i = 0; i < 15; i++) {
      store.addRecentSearch(`term${i}`);
    }
    expect(store.recentSearches.length).toBe(10);
    expect(store.recentSearches[0]).toBe('term14');
  });

  it('clears recent searches', () => {
    const store = useSearchStore();
    store.addRecentSearch('cats');
    store.clearRecentSearches();
    expect(store.recentSearches).toEqual([]);
  });

  it('ignores empty search terms', () => {
    const store = useSearchStore();
    store.addRecentSearch('');
    store.addRecentSearch('   ');
    expect(store.recentSearches).toEqual([]);
  });

  it('accumulates multiple person filters', () => {
    const store = useSearchStore();
    store.addFilter({ type: 'person', value: 'a', label: 'Alice' });
    store.addFilter({ type: 'person', value: 'b', label: 'Bob' });
    expect(store.activeFilters).toHaveLength(2);
    expect(store.personCount).toBe(2);
  });

  it('does not duplicate the same person filter', () => {
    const store = useSearchStore();
    store.addFilter({ type: 'person', value: 'a', label: 'Alice' });
    store.addFilter({ type: 'person', value: 'a', label: 'Alice' });
    expect(store.activeFilters).toHaveLength(1);
  });

  it('replaces a non-person filter of the same type', () => {
    const store = useSearchStore();
    store.addFilter({ type: 'location', value: 'Paris', label: 'Paris' });
    store.addFilter({ type: 'location', value: 'Rome', label: 'Rome' });
    expect(store.activeFilters).toEqual([{ type: 'location', value: 'Rome', label: 'Rome' }]);
  });

  it('togglePerson adds and removes a person', () => {
    const store = useSearchStore();
    store.togglePerson({ id: 'a', name: 'Alice' });
    expect(store.personCount).toBe(1);
    store.togglePerson({ id: 'a', name: 'Alice' });
    expect(store.personCount).toBe(0);
  });

  it('removeFilterValue removes a single person filter', () => {
    const store = useSearchStore();
    store.addFilter({ type: 'person', value: 'a', label: 'Alice' });
    store.addFilter({ type: 'person', value: 'b', label: 'Bob' });
    store.removeFilterValue('person', 'a');
    expect(store.activeFilters).toEqual([{ type: 'person', value: 'b', label: 'Bob' }]);
  });

  it('resets match/alone when the last person filter is removed', () => {
    const store = useSearchStore();
    store.addFilter({ type: 'person', value: 'a', label: 'Alice' });
    store.setPersonMatch('or');
    store.togglePersonAlone();
    expect(store.personMatch).toBe('or');
    expect(store.personAlone).toBe(true);
    store.removeFilterValue('person', 'a');
    expect(store.personMatch).toBe('and');
    expect(store.personAlone).toBe(false);
  });

  it('hasFilters reflects every media filter toggle (incl. nsfwOnly)', () => {
    const store = useSearchStore();
    expect(store.hasFilters).toBe(false);
    store.toggleNsfwOnly();
    expect(store.hasFilters).toBe(true);
    store.toggleFavoriteOnly();
    expect(store.hasFilters).toBe(true);
    store.toggleVideoOnly();
    store.toggleFacesOnly();
    store.togglePapersOnly();
    expect(store.hasFilters).toBe(true);
    store.clearFilters();
    expect(store.hasFilters).toBe(false);
  });

  it('clearFilters resets person match and alone state', () => {
    const store = useSearchStore();
    store.addFilter({ type: 'person', value: 'a', label: 'Alice' });
    store.setPersonMatch('or');
    store.togglePersonAlone();
    store.clearFilters();
    expect(store.personMatch).toBe('and');
    expect(store.personAlone).toBe(false);
    expect(store.personCount).toBe(0);
  });

  it('cycleStorageFilter cycles All -> Stored -> Not stored -> All', () => {
    const store = useSearchStore();
    expect(store.mediaFilters.storedOnly).toBe(false);
    expect(store.mediaFilters.notStoredOnly).toBe(false);

    store.cycleStorageFilter();
    expect(store.mediaFilters.storedOnly).toBe(true);
    expect(store.mediaFilters.notStoredOnly).toBe(false);
    expect(store.hasFilters).toBe(true);

    store.cycleStorageFilter();
    expect(store.mediaFilters.storedOnly).toBe(false);
    expect(store.mediaFilters.notStoredOnly).toBe(true);
    expect(store.hasFilters).toBe(true);

    store.cycleStorageFilter();
    expect(store.mediaFilters.storedOnly).toBe(false);
    expect(store.mediaFilters.notStoredOnly).toBe(false);
    expect(store.hasFilters).toBe(false);
  });

  it('clearFilters resets both storage tri-state flags', () => {
    const store = useSearchStore();
    store.cycleStorageFilter();
    expect(store.mediaFilters.storedOnly).toBe(true);
    store.clearFilters();
    expect(store.mediaFilters.storedOnly).toBe(false);
    expect(store.mediaFilters.notStoredOnly).toBe(false);
    expect(store.hasFilters).toBe(false);
  });
});
