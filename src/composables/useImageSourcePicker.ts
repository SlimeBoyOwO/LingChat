// useImageSourcePicker
//
// Android 端的"选图"流程:
//   1. 后端 Rust 在 #[cfg(target_os = "android")] 下,start_screenshot 不再抓屏、
//      创建覆盖层,而是 emit `screenshot:request-source` 事件。
//   2. 前端监听这个事件 -> 弹出底部 sheet(拍照 / 相册 / 取消)。
//   3. 用户选择后,动态创建 <input type="file">:
//        - 拍照: capture="environment" accept="image/*"
//        - 相册: 不带 capture, accept="image/*"
//      FileReader.readAsDataURL -> 拿到 base64 -> invoke('confirm_screenshot', { base64Cropped })。
//   4. 用户取消 -> invoke('cancel_screenshot')。
//
// 桌面端不进这条路径(后端不 emit 该事件)。
//
// 数据契约:
//   confirm_screenshot 接收任意合法 base64 字符串(原 desktop 路径是裁剪后的 jpeg;
//   这里直接把整张图片传过去,后端只做 base64 解码校验,不强制 jpeg)。

import { ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'

const isOpen = ref(false)

let unlistenRequest: UnlistenFn | null = null
let initCount = 0

// 当前进行中的选图会话 controller,每次 pickFromCamera / pickFromGallery
// 都会被新的 controller 取代。旧 controller.abort() 同步撤销所有挂在它
// signal 上的 listener,避免上一次未完成的 session cleanup 错杀这次的 input。
let activeController: AbortController | null = null

// 30s 兑底超时:用户进入系统相机 / 相册后即使扣 home 键再也不回 webview,
// 也能 30s 后强制 cleanup,避免残留 input 和永远不会触发的 focus listener。
const PICK_TIMEOUT_MS = 30_000

export function useImageSourcePicker() {
  function init() {
    if (initCount++ > 0) return
    listen('screenshot:request-source', () => {
      isOpen.value = true
    }).then((fn) => {
      unlistenRequest = fn
    })
  }

  function destroy() {
    if (--initCount > 0) return
    if (unlistenRequest) {
      unlistenRequest()
      unlistenRequest = null
    }
  }

  function close() {
    isOpen.value = false
  }

  async function pickFromCamera(): Promise<void> {
    close()
    await readFileAsBase64({ accept: 'image/*', capture: 'environment' })
  }

  async function pickFromGallery(): Promise<void> {
    close()
    await readFileAsBase64({ accept: 'image/*' })
  }

  async function cancel(): Promise<void> {
    close()
    try {
      await invoke('cancel_screenshot')
    } catch (e) {
      console.error('[ImageSourcePicker] cancel_screenshot failed:', e)
    }
  }

  return {
    isOpen,
    init,
    destroy,
    close,
    pickFromCamera,
    pickFromGallery,
    cancel,
  }
}

// --- internals ---

interface InputAttrs {
  accept: string
  capture?: 'environment' | 'user'
}

async function readFileAsBase64(attrs: InputAttrs): Promise<void> {
  // 撤销上一次还在飞行的 session,同步清掉旧 focus listener / 超时器。
  // 避免上一次的 finish() 误杀这一次的 input / change。
  activeController?.abort()
  const controller = new AbortController()
  activeController = controller
  const { signal } = controller

  // 30s 兜底超时:用户进入系统选择器后即使扣 home 不回,
  // 也能强制 cleanup。
  // finish / timeoutId 提到外面,让 setTimeout 与 Promise 体都能访问。
  let timeoutId: ReturnType<typeof setTimeout> | null = null
  let finish: () => void = () => {}

  timeoutId = setTimeout(() => {
    if (signal.aborted) return
    console.warn('[ImageSourcePicker] pick timeout, forcing cancel')
    invoke('cancel_screenshot').catch((e) =>
      console.error('[ImageSourcePicker] cancel_screenshot failed:', e),
    )
    finish()
  }, PICK_TIMEOUT_MS)

  return new Promise<void>((resolve) => {
    const input = document.createElement('input')
    input.type = 'file'
    input.accept = attrs.accept
    if (attrs.capture) input.setAttribute('capture', attrs.capture)
    input.style.position = 'fixed'
    input.style.left = '-9999px'
    input.style.top = '0'

    let settled = false
    finish = () => {
      if (settled) return
      settled = true
      if (timeoutId !== null) clearTimeout(timeoutId)
      input.remove()
      resolve()
      // 不需要手动 removeEventListener:
      // focus / cancel / change 都是挂在 signal 上的,
      // controller.abort() 会同步卸载。
      // 下一次 pickFromXxx 调用时 controller.abort() 会一并处理。
    }

    // 用户从系统选择器回 webview 会触发 window focus 事件,
    // 此时 input 如果没被收起(change 已处理完),清掉它。
    // 为了避免 “change 还未派发就被收”,
    // 实际上上一个 click() 后 change 是同步调度到的,
    // 所以这里不再延迟 finish，完全靠 controller.abort 走。
    window.addEventListener('focus', finish, { signal, capture: true })

    input.addEventListener(
      'change',
      async () => {
        if (signal.aborted) return
        const file = input.files?.[0]
        if (!file) {
          // 用户没选,直接走 cancel。
          try {
            await invoke('cancel_screenshot')
          } catch (e) {
            console.error('[ImageSourcePicker] cancel_screenshot failed:', e)
          }
          finish()
          return
        }

        try {
          const dataUrl = await fileToDataUrl(file)
          const base64 = stripDataUrlPrefix(dataUrl)
          await invoke('confirm_screenshot', { base64Cropped: base64 })
        } catch (e) {
          console.error('[ImageSourcePicker] confirm_screenshot failed:', e)
          try {
            await invoke('cancel_screenshot')
          } catch (e2) {
            console.error('[ImageSourcePicker] cancel_screenshot failed:', e2)
          }
        } finally {
          finish()
        }
      },
      { once: true, signal },
    )

    document.body.appendChild(input)
    // 在 Android WebView 里,input.click() 必须同步调用。
    input.click()
  })
}

function fileToDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const result = reader.result
      if (typeof result === 'string') resolve(result)
      else reject(new Error('FileReader did not return a string'))
    }
    reader.onerror = () => reject(reader.error ?? new Error('FileReader error'))
    reader.readAsDataURL(file)
  })
}

function stripDataUrlPrefix(dataUrl: string): string {
  const idx = dataUrl.indexOf(',')
  return idx >= 0 ? dataUrl.slice(idx + 1) : dataUrl
}
