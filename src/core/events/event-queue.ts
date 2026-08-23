import type { ScriptEventType } from '../../types'
import { eventProcessorManager } from './event-processor'
import { useGameStore } from '../../stores/modules/game'
import { useUIStore } from '../../stores/modules/ui/ui'

export class EventQueue {
  private queue: ScriptEventType[] = []
  private isProcessing = false
  private paused = true
  private currentEvent: ScriptEventType | null = null
  private currentResolve: (() => void) | null = null
  private durationResolve: (() => void) | null = null
  private durationTimer: number | null = null
  private generation = 0
  private abortController = new AbortController()

  addEvent(event: ScriptEventType) {
    if (event.type === 'error' || event.type === 'status_reset') {
      // Error/reset preempts both click waits and timed visual beats.
      this.generation += 1
      this.abortController.abort()
      this.abortController = new AbortController()
      this.queue = []
      this.isProcessing = false
      if (this.currentResolve) {
        this.currentResolve()
        this.currentResolve = null
      }
      if (this.durationTimer !== null) {
        window.clearTimeout(this.durationTimer)
        this.durationTimer = null
      }
      if (this.durationResolve) {
        this.durationResolve()
        this.durationResolve = null
      }
      useUIStore().resetHorrorEffects()
    }
    this.queue.push(event)
    if (!this.isProcessing && !this.paused) {
      this.processQueue()
    }
  }

  private async processQueue() {
    const runGeneration = this.generation
    const signal = this.abortController.signal
    this.isProcessing = true
    try {
      while (runGeneration === this.generation && this.queue.length > 0) {
        const event = this.queue.shift()
        if (event) {
          // 如果当前事件是thinking类型，且队列后面还有别的事件，则跳过
          if (event.type === 'thinking' && this.queue.length > 0) {
            continue
          }
          this.currentEvent = event
          try {
            await this.processSingleEvent(event, signal)
          } catch (error) {
            console.error('处理事件失败:', error, event)
            this.resetToInputState()
          }
          // clear() 会推进代号并解除当前 Promise；旧循环不得复活消费新队列。
          if (runGeneration !== this.generation) return
        }
      }
    } finally {
      if (runGeneration === this.generation) {
        this.isProcessing = false
        if (this.currentEvent?.isFinal) {
          this.resetToInputState()
        }
      }
    }
  }

  private async processSingleEvent(event: ScriptEventType, signal: AbortSignal): Promise<void> {
    // 处理事件并等待完成；clear() 后异步 processor 不得继续进入等待阶段。
    await eventProcessorManager.processEvent(event, signal)
    if (signal.aborted) return

    // 立绘闪现由独立覆盖层计时，可与紧随其后的音效/背景效果叠加。
    // 背景特效和突脸则必须占住队列的 authored duration，否则下一项
    // 会立即覆盖它们，玩家只能看到空白帧甚至完全看不到崩坏场景。
    if (this.isSelfTimedVisual(event)) return

    // 如果事件需要等待用户继续，就等待
    if (this.shouldWaitForUser(event)) {
      await this.waitForUserContinue()
    } else {
      await this.waitForDuration(event.duration)
      console.log('等待' + event.duration + '秒')
    }
  }

  private isSelfTimedVisual(event: ScriptEventType): boolean {
    return event.type === 'modify_character' && event.flash === true
  }

  private shouldWaitForUser(event: ScriptEventType): boolean {
    // 明确检查 duration 是否为 null 或 undefined
    if (event.duration === null || event.duration === undefined) {
      return true // 没有设置 duration，等待用户
    }

    // duration 为负数时等待用户
    if (event.duration < 0) {
      return true
    }

    // duration 为 0 或正数时，不等待用户
    return false
  }

  private waitForUserContinue(): Promise<void> {
    return new Promise((resolve) => {
      this.currentResolve = resolve
      // 设置游戏状态为等待用户输入
      const gameStore = useGameStore()
      gameStore.currentStatus = 'responding'
    })
  }

  // 用户继续的方法
  public continue(): boolean {
    let needWait = false // 这个用于标记下个消息是否还没到来，要想继续还需要等待的信号

    if (this.currentResolve) {
      this.currentResolve()
      this.currentResolve = null
    }

    // 假如当前消息不是最后一个，但是队列事件已经没了
    if (!this.currentEvent?.isFinal && this.queue.length === 0) {
      needWait = true
      console.log('后面的消息还没到，请稍等，最后一个消息是:', this.currentEvent)
    }

    return needWait
  }

  clear() {
    this.generation += 1
    this.abortController.abort()
    this.abortController = new AbortController()
    this.queue = []
    this.isProcessing = false
    this.paused = true
    if (this.currentResolve) {
      this.currentResolve()
      this.currentResolve = null
    }
    if (this.durationTimer !== null) {
      window.clearTimeout(this.durationTimer)
      this.durationTimer = null
    }
    if (this.durationResolve) {
      this.durationResolve()
      this.durationResolve = null
    }
    useUIStore().resetHorrorEffects()
    this.resetToInputState()
  }

  /** 恢复事件队列消费（MainChat 就绪后调用） */
  resume() {
    this.paused = false
    if (this.queue.length > 0 && !this.isProcessing) {
      this.processQueue()
    }
  }

  private resetToInputState() {
    this.currentEvent = null

    const gameStore = useGameStore()
    gameStore.currentStatus = 'input'
    gameStore.currentLine = ''
  }

  getState() {
    return {
      queueLength: this.queue.length,
      isProcessing: this.isProcessing,
      isWaitingForUser: this.currentResolve !== null,
    }
  }

  private waitForDuration(duration: number): Promise<void> {
    return new Promise((resolve) => {
      this.durationResolve = resolve
      this.durationTimer = window.setTimeout(() => {
        this.durationTimer = null
        this.durationResolve = null
        resolve()
      }, duration * 1000)
    })
  }
}

export const eventQueue = new EventQueue()
