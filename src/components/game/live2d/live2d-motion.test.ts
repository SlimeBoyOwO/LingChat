import { describe, expect, it, vi } from 'vitest'

import { trackMotionLifecycle, type MotionLifecycleManager } from './live2d-motion'

class FakeMotionManager implements MotionLifecycleManager {
  state = { currentGroup: '', currentIndex: -1, currentPriority: 0 }
  private listeners = new Map<string, Set<(...args: any[]) => void>>()

  on(event: string, listener: (...args: any[]) => void) {
    const listeners = this.listeners.get(event) ?? new Set()
    listeners.add(listener)
    this.listeners.set(event, listeners)
  }

  off(event: string, listener: (...args: any[]) => void) {
    this.listeners.get(event)?.delete(listener)
  }

  emit(event: string, ...args: any[]) {
    for (const listener of [...(this.listeners.get(event) ?? [])]) listener(...args)
  }
}

describe('trackMotionLifecycle', () => {
  it('ignores an old idle finish before the tracked reaction starts', () => {
    const manager = new FakeMotionManager()
    const finished = vi.fn()
    trackMotionLifecycle(manager, 'Reactions', 4, 3, finished)

    manager.state = { currentGroup: 'Idle', currentIndex: 0, currentPriority: 1 }
    manager.emit('motionFinish')
    expect(finished).not.toHaveBeenCalled()

    manager.state = { currentGroup: 'Reactions', currentIndex: 4, currentPriority: 3 }
    manager.emit('motionStart', 'Reactions', 4)
    manager.emit('motionFinish')
    expect(finished).toHaveBeenCalledOnce()
  })

  it('attributes finish events by group, index, and priority', () => {
    const manager = new FakeMotionManager()
    const finished = vi.fn()
    trackMotionLifecycle(manager, 'Reactions', 2, 3, finished)

    manager.state = { currentGroup: 'Reactions', currentIndex: 2, currentPriority: 2 }
    manager.emit('motionStart', 'Reactions', 2)
    manager.emit('motionFinish')
    expect(finished).not.toHaveBeenCalled()

    manager.state = { currentGroup: 'Reactions', currentIndex: 2, currentPriority: 3 }
    manager.emit('motionStart', 'Reactions', 2)
    manager.emit('motionFinish')
    expect(finished).toHaveBeenCalledOnce()
  })

  it('removes both listeners when disposed', () => {
    const manager = new FakeMotionManager()
    const finished = vi.fn()
    const dispose = trackMotionLifecycle(manager, 'Reactions', 1, 3, finished)
    dispose()

    manager.state = { currentGroup: 'Reactions', currentIndex: 1, currentPriority: 3 }
    manager.emit('motionStart', 'Reactions', 1)
    manager.emit('motionFinish')
    expect(finished).not.toHaveBeenCalled()
  })
})
