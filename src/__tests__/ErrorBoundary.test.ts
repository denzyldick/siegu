import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import ErrorBoundary from '@/components/shared/ErrorBoundary.vue'

describe('ErrorBoundary', () => {
  it('renders slot content when no error', () => {
    const wrapper = mount(ErrorBoundary, {
      slots: {
        default: '<div class="child-content">Hello</div>',
      },
    })
    expect(wrapper.find('.child-content').exists()).toBe(true)
    expect(wrapper.find('.error-boundary').exists()).toBe(false)
  })

  it('shows retry button in error state', async () => {
    const wrapper = mount(ErrorBoundary)
    expect(wrapper.find('.error-boundary').exists()).toBe(false)

    const errorDiv = wrapper.find('.error-boundary')
    expect(errorDiv.exists()).toBe(false)

    const vm = wrapper.vm as unknown as { error: Error | null }
    vm.error = new Error('test error')
    await wrapper.vm.$nextTick()

    expect(wrapper.find('.error-boundary').exists()).toBe(true)
    expect(wrapper.text()).toContain('Something went wrong')
    expect(wrapper.text()).toContain('test error')
  })

  it('retry button clears the error', async () => {
    const wrapper = mount(ErrorBoundary)

    const vm = wrapper.vm as unknown as { error: Error | null; retry: () => void }
    vm.error = new Error('fail')
    await wrapper.vm.$nextTick()
    expect(wrapper.find('.error-boundary').exists()).toBe(true)

    vm.retry()
    await wrapper.vm.$nextTick()
    expect(wrapper.find('.error-boundary').exists()).toBe(false)
  })

  it('handles non-Error thrown values', async () => {
    const wrapper = mount(ErrorBoundary)

    const vm = wrapper.vm as unknown as { error: Error | null }
    vm.error = new Error('string error')
    await wrapper.vm.$nextTick()

    expect(wrapper.find('.error-boundary').exists()).toBe(true)
    expect(wrapper.text()).toContain('string error')
  })
})
