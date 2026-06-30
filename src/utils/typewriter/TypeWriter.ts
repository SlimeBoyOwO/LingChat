import { useUIStore } from '../../stores/modules/ui/ui'

export type TypeWriterStatus = 'idle' | 'typing' | 'completed'

export class TypeWriter {
  private element: HTMLInputElement | HTMLTextAreaElement
  private timer: ReturnType<typeof setTimeout> | null = null
  private speed: number
  private generation: number
  private textBuffer: string

  private onFinishCallback: (() => void) | null
  private onTextUpdateCallback: ((text: string) => void) | null

  // Audio
  private audioContext: AudioContext | null
  private soundBuffers: AudioBuffer[]
  private readonly soundUrls: string[]

  // State
  private _status: TypeWriterStatus = 'idle'

  constructor(
    element: HTMLInputElement | HTMLTextAreaElement,
    onTextUpdateCallback?: (text: string) => void,
    soundUrls?: string[],
  ) {
    this.element = element
    this.timer = null
    this.speed = 50
    this.generation = 0
    this.textBuffer = ''
    this.onFinishCallback = null
    this.onTextUpdateCallback = onTextUpdateCallback || null
    this.audioContext = null
    this.soundBuffers = []
    this.soundUrls = soundUrls ?? ['../audio_effects/对话.wav']
  }

  /** Current typewriter state, queryable externally at any time. */
  public get status(): TypeWriterStatus {
    return this._status
  }

  // ─── Audio ───────────────────────────────────────────────

  private async initAudio(): Promise<void> {
    try {
      this.audioContext = new (window.AudioContext || (window as any).webkitAudioContext)()
      // Modern browsers require user gesture; resume if suspended
      if (this.audioContext.state === 'suspended') {
        await this.audioContext.resume()
      }
      await this.loadSounds()
    } catch (e) {
      console.warn('音频初始化失败:', e)
      const uiStore = useUIStore()
      uiStore.setEnableChatEffectSound(false)
    }
  }

  private async loadSounds(): Promise<void> {
    if (!this.audioContext) return

    try {
      const promises = this.soundUrls.map(async (url) => {
        const response = await fetch(url)
        const arrayBuffer = await response.arrayBuffer()
        return this.audioContext!.decodeAudioData(arrayBuffer)
      })
      this.soundBuffers = await Promise.all(promises)
    } catch (e) {
      console.warn('音效加载失败:', e)
    }
  }

  /** Safe wrapper — never throws, so the timer chain cannot be broken. */
  private playRandomSound(): void {
    try {
      const uiStore = useUIStore()
      if (!uiStore.enableChatEffectSound || !this.audioContext || this.soundBuffers.length === 0) {
        return
      }
      if (uiStore.currentAvatarAudio !== 'None') return

      // Ensure context is running (may have been suspended by browser policy)
      if (this.audioContext.state === 'suspended') {
        this.audioContext.resume()
      }

      const buffer = this.soundBuffers[Math.floor(Math.random() * this.soundBuffers.length)]
      if (!buffer) return

      const source = this.audioContext.createBufferSource()
      source.buffer = buffer
      source.playbackRate.value = 1.0 + (Math.random() - 0.5) * 0.01 // slight variation

      const gainNode = this.audioContext.createGain()
      gainNode.gain.value = 0.8
      source.connect(gainNode)
      gainNode.connect(this.audioContext.destination)
      source.start()
    } catch {
      // Silently ignore audio playback failures — never break the typing animation
    }
  }

  public setSoundEnabled(enabled: boolean): void {
    const uiStore = useUIStore()
    uiStore.setEnableChatEffectSound(enabled)
    if (enabled && !this.audioContext) {
      this.initAudio() // fire-and-forget: audio is optional enhancement
    }
  }

  // ─── Core Typing ─────────────────────────────────────────

  /**
   * Start typing the given text character by character.
   *
   * Returns a Promise that resolves when:
   *   - All characters have been displayed (natural completion), OR
   *   - The animation is cancelled by a subsequent `start()` call
   *
   * Safe to call while a previous animation is still running — the old one
   * is cancelled cleanly via generation counter before the new one begins.
   */
  public start(text: string, speed?: number): Promise<void> {
    // Cancel any previous animation and advance generation
    this.stop()
    this.generation++
    const currentGen = this.generation

    this._status = 'typing'
    this.textBuffer = ''

    // Parse speed
    if (speed !== undefined) {
      this.speed = Number.isInteger(speed) ? speed : parseInt(String(speed), 10) || 50
    }

    // Init audio if needed (fire-and-forget — audio is non-critical)
    const uiStore = useUIStore()
    if (uiStore.enableChatEffectSound && !this.audioContext) {
      this.initAudio()
    }

    let i = 0

    return new Promise<void>((resolve) => {
      const typing = (): void => {
        // Guard: stale generation means a newer start() has taken over
        if (this.generation !== currentGen) {
          resolve()
          return
        }

        if (i < text.length) {
          this.textBuffer += text.charAt(i)
          this.element.value = this.textBuffer
          if (this.onTextUpdateCallback) {
            this.onTextUpdateCallback(this.textBuffer)
          }
          i++
          this.element.scrollTop = this.element.scrollHeight

          this.playRandomSound()

          //timer接收delay的是延迟（越大越慢），而传入的speed是速度（越大越快）
          //此处按照Text.vue（速度演示文本）中的方式重新计算延迟值
          //并保留了原本的微量随机偏移设计
          const maxDelay = 200
          const minDelay = 10
          const randomVariation = this.speed * 0.2
          const delay = maxDelay - ((this.speed - 1) / 99) * (maxDelay - minDelay) + Math.random() * randomVariation

          this.timer = setTimeout(typing, delay)
        } else {
          this.finish()
          resolve()
        }
      }

      // Start the typing loop immediately
      typing()
    })
  }

  /** Immediately complete the current typing animation (show all text). */
  public finish(): void {
    this.stopTimer()
    this._status = 'completed'
    this.element.style.setProperty('border-right', 'none')
    if (this.onFinishCallback) {
      this.onFinishCallback()
    }
  }

  /**
   * Cancel the current typing animation.
   * Resets status to idle but does NOT clear the displayed text.
   */
  public stop(): void {
    this.stopTimer()
    this.generation++ // invalidate any lingering typing closures
    this._status = 'idle'
  }

  /** Clear the DOM element and internal text buffer. */
  public clear(): void {
    this.element.value = ''
    this.textBuffer = ''
  }

  /** Full cleanup: stop animation, clear text, close audio resources. */
  public destroy(): void {
    this.stop()
    this.clear()
    if (this.audioContext) {
      this.audioContext.close().catch(() => {})
      this.audioContext = null
    }
    this.soundBuffers = []
  }

  // ─── Callback registration ───────────────────────────────

  public onFinish(callback: () => void): void {
    this.onFinishCallback = callback
  }

  // ─── Private helpers ─────────────────────────────────────

  private stopTimer(): void {
    if (this.timer) {
      clearTimeout(this.timer)
      this.timer = null
    }
  }
}
