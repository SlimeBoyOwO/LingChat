let runtimePromise: Promise<Live2dRuntime> | null = null
let pluginRegistered = false
let sdkConfigured = false

export interface Live2dRuntime {
  pixi: typeof import('pixi.js')
  engine: typeof import('untitled-pixi-live2d-engine/cubism')
}

declare global {
  interface Window {
    Live2DCubismCore?: unknown
  }
}

function loadCubismCore(): Promise<void> {
  if (window.Live2DCubismCore) return Promise.resolve()
  return new Promise((resolve, reject) => {
    const existing = document.querySelector<HTMLScriptElement>('script[data-live2d-cubism-core]')
    if (existing) {
      existing.addEventListener('load', () => resolve(), { once: true })
      existing.addEventListener('error', () => reject(new Error('Cubism Core failed to load')), {
        once: true,
      })
      return
    }
    const script = document.createElement('script')
    script.src = `${import.meta.env.BASE_URL}vendor/live2d/live2dcubismcore.min.js`
    script.async = true
    script.dataset.live2dCubismCore = 'true'
    script.onload = () => resolve()
    script.onerror = () => {
      script.remove()
      reject(new Error('Cubism Core failed to load'))
    }
    document.head.appendChild(script)
  })
}

export function loadLive2dRuntime(): Promise<Live2dRuntime> {
  if (!runtimePromise) {
    runtimePromise = (async () => {
      await loadCubismCore()
      const [pixi, engine] = await Promise.all([
        import('pixi.js'),
        import('untitled-pixi-live2d-engine/cubism'),
      ])
      if (!pluginRegistered) {
        pixi.extensions.add(engine.Live2DPlugin)
        pluginRegistered = true
      }
      if (!sdkConfigured) {
        engine.configureCubismSDK({ memorySizeMB: 64 })
        await engine.cubismReady()
        sdkConfigured = true
      }
      return { pixi, engine }
    })().catch((error) => {
      runtimePromise = null
      throw error
    })
  }
  return runtimePromise
}
