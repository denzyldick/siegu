import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount } from '@vue/test-utils';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

// Store event listeners for manual triggering in tests
const listeners = {};

// Override the listen mock to capture listeners
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event, handler) => {
    listeners[event] = handler;
    return Promise.resolve(vi.fn());
  }),
}));

// Mock IntersectionObserver as a constructor
vi.stubGlobal(
  'IntersectionObserver',
  vi.fn(function () {
    this.observe = vi.fn();
    this.disconnect = vi.fn();
  }),
);

// Import AFTER mocks
import Photos from '../components/Photos.vue';

describe('Photos.vue — data pipeline', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.keys(listeners).forEach((k) => delete listeners[k]);
    // Default: get_media_server_port returns null (no video playback needed)
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_media_server_port') return null;
      if (cmd === 'list_files') return '[]';
      return null;
    });
  });

  function createPhotos() {
    return mount(Photos, {
      props: {
        filters: { favoritesOnly: false, videosOnly: false, dateRange: 'all', folder: null },
      },
      global: {
        stubs: {
          PhotoViewer: true,
          'v-icon': { template: "<span class='v-icon-stub'><slot /></span>" },
          'v-btn': { template: "<button class='v-btn-stub'><slot /></button>" },
          'v-sheet': { template: "<div class='v-sheet-stub'><slot /></div>" },
          'v-spacer': { template: "<div class='v-spacer-stub' />" },
          'v-fade-transition': { template: "<div class='v-fade-transition-stub'><slot /></div>" },
          'v-progress-circular': { template: "<div class='v-progress-circular-stub' />" },
        },
        mocks: {
          $vuetify: { theme: { current: { dark: false } } },
          $t: (msg) => msg,
        },
      },
    });
  }

  async function addPhoto(wrapper, id, overrides = {}) {
    const photo = {
      id,
      location: '/test/photo.jpg',
      encoded: '',
      created: '2026-01-15 12:00:00',
      indexed: 0,
      objects: {},
      properties: {},
      caption: null,
      aesthetics_score: null,
      favorite: false,
      ...overrides,
    };
    await listeners['photos-discovered']({ payload: [photo] });
    wrapper.vm.scanBuffer = [photo];
    wrapper.vm.updateGroups(wrapper.vm.scanBuffer);
    wrapper.vm.scanBuffer = [];
  }

  it('registers event listeners on mount', async () => {
    createPhotos();
    await new Promise((r) => setTimeout(r, 0));
    expect(listen).toHaveBeenCalledWith('photos-discovered', expect.any(Function));
    expect(listen).toHaveBeenCalledWith('photo-received', expect.any(Function));
    expect(listen).toHaveBeenCalledWith('photo-analysis-result', expect.any(Function));
  });

  it('renders photo card without analysis when indexed=0', async () => {
    const wrapper = createPhotos();
    await new Promise((r) => setTimeout(r, 0));
    await addPhoto(wrapper, 'no-analysis');
    await new Promise((r) => setTimeout(r, 0));

    expect(wrapper.vm.imagesMap['no-analysis']).toBeDefined();
    expect(wrapper.vm.imagesMap['no-analysis'].indexed).toBe(0);
    // Should NOT show the check-circle icon (indexed badge)
    expect(wrapper.html()).not.toContain('mdi-check-circle');
  });

  it('renders photo card WITH analysis after photo-analysis-result event', async () => {
    const wrapper = createPhotos();
    await new Promise((r) => setTimeout(r, 0));
    await addPhoto(wrapper, 'with-analysis');
    await new Promise((r) => setTimeout(r, 0));

    // Simulate analysis completing
    const updatedData = {
      id: 'with-analysis',
      location: '/test/photo.jpg',
      encoded: '',
      created: '2026-01-15 12:00:00',
      indexed: 2,
      objects: { cat: 0.95, dog: 0.8, person: 0.6 },
      properties: { face_count: '2', nsfw: '0.001' },
      caption: 'a cat and a dog',
      aesthetics_score: 0.88,
      favorite: false,
      ai_status: {
        clip: 1,
        face: 1,
        ocr: 1,
        nsfw: 1,
        aesthetics: 1,
        yolo: 1,
        blip: 1,
        arcface: 1,
        midas: 1,
        whisper: 1,
        sam: 1,
        superres: 1,
      },
    };
    invoke.mockResolvedValueOnce(JSON.stringify(updatedData));
    await listeners['photo-analysis-result']({ payload: { id: 'with-analysis' } });
    await new Promise((r) => setTimeout(r, 0));

    // FIRST check: internal reactive state MUST be updated
    const img = wrapper.vm.imagesMap['with-analysis'];
    expect(img.indexed).toBe(2);
    expect(Object.keys(img.objects).length).toBe(3);

    // SECOND check: rendered HTML MUST contain analysis indicators
    const html = wrapper.html();
    expect(html).toContain('mdi-check-circle'); // indexed badge
    expect(html).toContain('cat'); // tag name
    expect(html).toContain('dog'); // tag name
    expect(html).toContain('0.88'); // aesthetics score
    // Note: caption and face count depend on Image.vue internals
  });
});
