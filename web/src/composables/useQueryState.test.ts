import { beforeEach, describe, expect, it } from 'vitest'
import { createApp, defineComponent, h } from 'vue'
import { createMemoryHistory, createRouter, type Router } from 'vue-router'
import { useQueryState } from './useQueryState'

type QueryState = ReturnType<typeof useQueryState>

let router: Router
let state: QueryState

// Mounts a throwaway component so the composable has a route to work with
async function mountAt(path: string) {
  router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: '/page.html', component: { render: () => h('div') } }]
  })
  const Probe = defineComponent({
    setup() {
      state = useQueryState()
      return () => h('div')
    }
  })
  await router.push(path)
  createApp(Probe).use(router).mount(document.createElement('div'))
}

// router.replace resolves asynchronously; let the batched update land
async function settle() {
  await new Promise((resolve) => setTimeout(resolve, 0))
  await router.isReady()
}

beforeEach(async () => {
  await mountAt('/page.html?week=2025-06-02&tags=a&tags=b')
})

describe('useQueryState', () => {
  it('reads single values, the first of repeated keys, and null for missing ones', () => {
    expect(state.get('week')).toBe('2025-06-02')
    expect(state.get('tags')).toBe('a')
    expect(state.get('missing')).toBeNull()
  })

  it('sets and removes keys while leaving the others in place', async () => {
    state.set('edit', '5')
    await settle()
    expect(router.currentRoute.value.query).toMatchObject({ week: '2025-06-02', edit: '5' })

    state.set('week', null)
    await settle()
    expect(router.currentRoute.value.query.week).toBeUndefined()
    expect(router.currentRoute.value.query.edit).toBe('5')
  })

  it('merges changes made in the same tick into one navigation', async () => {
    state.set('from', '2025-01-01')
    state.set('to', '2025-01-31')
    state.update({ trainer_id: '3', week: null })
    await settle()
    expect(router.currentRoute.value.query).toEqual({ from: '2025-01-01', to: '2025-01-31', trainer_id: '3', tags: ['a', 'b'] })
  })

  it('replaces rather than pushes history', async () => {
    const before = router.options.history.state.position
    state.set('edit', '5')
    await settle()
    expect(router.options.history.state.position).toBe(before)
  })

  it('exposes the live query', async () => {
    state.set('week', '2025-06-09')
    await settle()
    expect(state.query.value.week).toBe('2025-06-09')
  })
})
