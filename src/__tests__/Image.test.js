import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import Image from '../components/Image.vue';

describe('Image.vue', () => {
  function mountImage(pathData) {
    return mount(Image, {
      props: { path: pathData },
      global: {
        mocks: {
          $t: (msg) => msg,
        },
      },
    });
  }

  it('renders no tags when objects is empty', () => {
    const wrapper = mountImage({
      id: 'test-1',
      indexed: 0,
      objects: {},
      properties: {},
      location: '/test/photo.jpg',
    });
    expect(wrapper.html()).toContain('image-item-container');
  });

  it('renders tags when objects are present', () => {
    const wrapper = mountImage({
      id: 'test-2',
      indexed: 2,
      objects: { cat: 0.95, dog: 0.8, person: 0.6 },
      properties: { face_count: '2' },
      caption: 'a cat and a dog',
      aesthetics_score: 0.85,
      location: '/test/photo.jpg',
    });
    // Tags should show top 3 objects
    const tags = wrapper.findAll('.info-tag');
    expect(tags.length).toBe(3);
    expect(tags[0].text()).toBe('cat');
  });

  it('computes faceCount from properties', () => {
    const wrapper = mountImage({
      id: 'test-3',
      indexed: 2,
      objects: {},
      properties: { face_count: '3' },
      location: '/test/photo.jpg',
    });
    // Face count should show
    expect(wrapper.html()).toContain('3');
  });

  it('shows aesthetics score when present', () => {
    const wrapper = mountImage({
      id: 'test-4',
      indexed: 2,
      objects: {},
      properties: {},
      aesthetics_score: 0.75,
      location: '/test/photo.jpg',
    });
    expect(wrapper.text()).toContain('0.75');
  });

  it('hasResults is true when indexed=2 even without objects', () => {
    const wrapper = mountImage({
      id: 'test-5',
      indexed: 2,
      objects: {},
      properties: {},
      location: '/test/photo.jpg',
    });
    // The check-circle icon should show when indexed=2
    expect(wrapper.html()).toContain('mdi-check-circle');
  });

  it('hasResults is false for unindexed photo with no data', () => {
    const wrapper = mountImage({
      id: 'test-6',
      indexed: 0,
      objects: {},
      properties: {},
      location: '/test/photo.jpg',
    });
    // No results elements should render
    expect(wrapper.html()).not.toContain('mdi-check-circle');
    expect(wrapper.html()).not.toContain('mdi-star');
    expect(wrapper.html()).not.toContain('mdi-face');
  });
});
