import { describe, it, expect, vi, afterEach } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import Image from '../components/MediaCard.vue';

// The card only renders its overlays once the IntersectionObserver reports
// the card as visible; happy-dom's observer never fires, so stub one that
// reports every observed card as intersecting immediately.
function stubIntersectionObserver() {
  vi.stubGlobal(
    'IntersectionObserver',
    class {
      constructor(cb) {
        this.cb = cb;
      }
      observe() {
        this.cb([{ isIntersecting: true }], this);
      }
      unobserve() {}
      disconnect() {}
    },
  );
}

describe('MediaCard.vue', () => {
  function mountImage(pathData, propsData = {}) {
    stubIntersectionObserver();
    const wrapper = mount(Image, {
      props: { path: pathData, ...propsData },
      global: {
        mocks: {
          $t: (msg) => msg,
        },
      },
    });
    return wrapper;
  }

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it('renders the card for an unanalyzed photo', () => {
    const wrapper = mountImage({
      id: 'test-1',
      indexed: 0,
      objects: {},
      properties: {},
      location: '/test/photo.jpg',
    });
    expect(wrapper.html()).toContain('media-card-container');
    expect(wrapper.html()).not.toContain('mdi-auto-fix');
  });

  it('shows the AI badge when objects are present', async () => {
    const wrapper = mountImage({
      id: 'test-2',
      indexed: 2,
      objects: { cat: 0.95, dog: 0.8, person: 0.6 },
      properties: {},
      location: '/test/photo.jpg',
    });
    await nextTick();
    expect(wrapper.find('.ai-badge').exists()).toBe(true);
  });

  it('no longer renders tags, scores or face counts under the picture', async () => {
    const wrapper = mountImage({
      id: 'test-3',
      indexed: 2,
      objects: { cat: 0.95 },
      properties: { face_count: '3' },
      caption: 'a cat and a dog',
      aesthetics_score: 0.85,
      location: '/test/photo.jpg',
    });
    await nextTick();
    expect(wrapper.find('.media-card-info').exists()).toBe(false);
    expect(wrapper.findAll('.info-tag').length).toBe(0);
    expect(wrapper.findAll('.detail-item').length).toBe(0);
    expect(wrapper.text()).not.toContain('75%');
  });

  it('hides the AI badge for an analyzed photo without caption', async () => {
    const wrapper = mountImage({
      id: 'test-4',
      indexed: 0,
      objects: {},
      properties: {},
      location: '/test/photo.jpg',
    });
    await nextTick();
    expect(wrapper.find('.ai-badge').exists()).toBe(false);
  });

  it('shows the AI badge when fully indexed even without other data', async () => {
    const wrapper = mountImage({
      id: 'test-5',
      indexed: 2,
      objects: {},
      properties: {},
      location: '/test/photo.jpg',
    });
    await nextTick();
    expect(wrapper.find('.ai-badge').exists()).toBe(true);
  });

  it('renders the favorite heart overlay only for favorites', async () => {
    const fav = mountImage({
      id: 'fav-1',
      indexed: 0,
      objects: {},
      properties: {},
      favorite: true,
      location: '/test/photo.jpg',
    });
    await nextTick();
    expect(fav.find('.favorite-heart').exists()).toBe(true);

    const plain = mountImage({
      id: 'fav-2',
      indexed: 0,
      objects: {},
      properties: {},
      favorite: false,
      location: '/test/photo.jpg',
    });
    await nextTick();
    expect(plain.find('.favorite-heart').exists()).toBe(false);
  });

  it('double-tap toggles favorite instead of opening the viewer', async () => {
    vi.useFakeTimers();
    const wrapper = mountImage({
      id: 'tap-1',
      indexed: 0,
      objects: {},
      properties: {},
      location: '/test/photo.jpg',
    });
    await wrapper.trigger('click');
    await wrapper.trigger('click');
    vi.advanceTimersByTime(500);

    expect(wrapper.emitted('toggle-favorite')).toHaveLength(1);
    expect(wrapper.emitted('click')).toBeUndefined();
  });

  it('single tap opens the viewer after the double-tap window', async () => {
    vi.useFakeTimers();
    const wrapper = mountImage({
      id: 'tap-2',
      indexed: 0,
      objects: {},
      properties: {},
      location: '/test/photo.jpg',
    });
    await wrapper.trigger('click');
    vi.advanceTimersByTime(260);

    expect(wrapper.emitted('click')).toHaveLength(1);
    expect(wrapper.emitted('toggle-favorite')).toBeUndefined();
  });

  it('taps select immediately in selection mode', async () => {
    const wrapper = mountImage(
      { id: 'sel-1', indexed: 0, objects: {}, properties: {}, location: '/p.jpg' },
      { selectionMode: true, selected: false },
    );
    await wrapper.trigger('click');
    await wrapper.trigger('click');

    expect(wrapper.emitted('select')).toHaveLength(2);
    expect(wrapper.emitted('toggle-favorite')).toBeUndefined();
  });
});
