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
  return new Promise<void>((resolve) => {
    const input = document.createElement('input')
    input.type = 'file'
    input.accept = attrs.accept
    if (attrs.capture) input.setAttribute('capture', attrs.capture)
    input.style.position = 'fixed'
    input.style.left = '-9999px'
    input.style.top = '0'

    let settled = false
    const finish = () => {
      if (settled) return
      settled = true
      input.remove()
      window.removeEventListener('focus', onFocusBack, true)
      resolve()
    }

    // 用户从系统选择器回来,会触发 window focus 事件;用来清掉残留 input。
    const onFocusBack = () => {
      // 延迟一点再清,避免 change 还没派发就被回收。
      setTimeout(finish, 1500)
    }
    window.addEventListener('focus', onFocusBack, true)

    input.addEventListener(
      'change',
      async () => {
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
      { once: true },
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
