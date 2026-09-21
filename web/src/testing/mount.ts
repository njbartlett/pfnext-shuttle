// Mounting helpers for component tests: the app's real router (guards
// included) on an in-memory history, so nothing touches window.location.
import { flushPromises, mount, type ComponentMountingOptions, type VueWrapper } from '@vue/test-utils'
import type { Component } from 'vue'
import { createMemoryHistory, type Router } from 'vue-router'
import { createAppRouter } from '@/router'

export function createTestRouter(): Router {
  return createAppRouter(createMemoryHistory())
}

// Mounts `component` at `path` on a fresh router and waits for the mocked
// API calls made during setup to settle. Pass `attachTo: document.body` for
// components that use Bootstrap modals or the data-api.
export async function mountAt<C extends Component>(
  component: C,
  path: string,
  options: ComponentMountingOptions<C> = {}
): Promise<{ wrapper: VueWrapper; router: Router }> {
  const router = createTestRouter()
  await router.push(path)
  const wrapper = mount(component, {
    ...options,
    global: { ...options.global, plugins: [...(options.global?.plugins ?? []), router] }
  } as ComponentMountingOptions<C>) as unknown as VueWrapper
  await flushPromises()
  return { wrapper, router }
}

// Waits until `check` stops throwing, for Bootstrap transitions that finish
// on a timer rather than a promise
export async function waitFor(check: () => void, timeoutMillis = 1000): Promise<void> {
  const deadline = Date.now() + timeoutMillis
  for (;;) {
    try {
      check()
      return
    } catch (error) {
      if (Date.now() > deadline) {
        throw error
      }
      await new Promise((resolve) => setTimeout(resolve, 10))
    }
  }
}
