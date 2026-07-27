import type { MediaItem, MediaProperties } from '@/types/media'
import type { Person } from '@/types/person'

let nextId = 1

export function createMediaItem(overrides: Partial<MediaItem> = {}): MediaItem {
  return {
    id: nextId++,
    location: '/photos/test.jpg',
    encoded: null,
    created: '2024-01-15T10:30:00Z',
    indexed: 0,
    objects: null,
    properties: null,
    caption: null,
    aesthetics_score: null,
    favorite: false,
    ai_status: null,
    latitude: null,
    longitude: null,
    ...overrides,
  }
}

export function createVideoItem(overrides: Partial<MediaItem> = {}): MediaItem {
  return createMediaItem({
    location: '/videos/test.mp4',
    ...overrides,
  })
}

export function createPerson(overrides: Partial<Person> = {}): Person {
  return {
    id: nextId++,
    name: 'Test Person',
    face_count: 3,
    representative_crop: null,
    encoded: null,
    representative_face_id: null,
    ...overrides,
  }
}

export function createProperties(overrides: Partial<MediaProperties> = {}): MediaProperties {
  return {
    width: 1920,
    height: 1080,
    fileSize: 5242880,
    ...overrides,
  }
}

export function resetIdCounter(): void {
  nextId = 1
}
