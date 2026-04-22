import { registerSW } from 'virtual:pwa-register'

const UPDATE_CHECK_INTERVAL_MS = 60 * 60 * 1000

export function registerServiceWorker(): void {
  if (import.meta.env.DEV) return

  registerSW({
    immediate: true,
    onRegisteredSW(_swUrl, registration) {
      if (registration) {
        setInterval(() => {
          void registration.update()
        }, UPDATE_CHECK_INTERVAL_MS)
      }
    },
  })
}