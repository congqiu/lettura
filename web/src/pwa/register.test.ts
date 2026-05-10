import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { registerSW } from './__mocks__/pwa-register'

describe('registerServiceWorker', () => {
  beforeEach(() => {
    registerSW.mockReset()
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllEnvs()
  })

  it('does not register SW in DEV mode', async () => {
    vi.stubEnv('DEV', true)
    const { registerServiceWorker } = await import('./register')
    registerServiceWorker()
    expect(registerSW).not.toHaveBeenCalled()
  })

  it('registers SW in PROD mode with immediate: true', async () => {
    vi.stubEnv('DEV', false)
    const { registerServiceWorker } = await import('./register')
    registerServiceWorker()
    expect(registerSW).toHaveBeenCalledTimes(1)
    const options = registerSW.mock.calls[0][0] as { immediate: boolean }
    expect(options.immediate).toBe(true)
  })

  it('installs hourly update timer via onRegisteredSW', async () => {
    vi.stubEnv('DEV', false)
    const { registerServiceWorker } = await import('./register')
    registerServiceWorker()

    const options = registerSW.mock.calls[0][0] as {
      onRegisteredSW: (
        url: string,
        reg: { update: () => Promise<void> } | undefined,
      ) => void
    }
    const update = vi.fn()
    options.onRegisteredSW('/sw.js', { update })

    vi.advanceTimersByTime(60 * 60 * 1000 - 1)
    expect(update).not.toHaveBeenCalled()
    vi.advanceTimersByTime(1)
    expect(update).toHaveBeenCalledTimes(1)
  })

  it('tolerates missing registration in onRegisteredSW', async () => {
    vi.stubEnv('DEV', false)
    const { registerServiceWorker } = await import('./register')
    registerServiceWorker()

    const options = registerSW.mock.calls[0][0] as {
      onRegisteredSW: (url: string, reg: undefined) => void
    }
    expect(() => options.onRegisteredSW('/sw.js', undefined)).not.toThrow()
  })
})
