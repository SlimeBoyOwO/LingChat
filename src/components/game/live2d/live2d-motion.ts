export interface MotionLifecycleState {
  currentGroup?: string
  currentIndex?: number
  currentPriority: number
}

export interface MotionLifecycleManager {
  state: MotionLifecycleState
  on(event: string, listener: (...args: any[]) => void): unknown
  off(event: string, listener: (...args: any[]) => void): unknown
}

export function trackMotionLifecycle(
  manager: MotionLifecycleManager,
  group: string,
  index: number,
  priority: number,
  onFinish: () => void,
): () => void {
  let active = true

  const dispose = () => {
    if (!active) return
    active = false
    manager.off('motionStart', handleStart)
    manager.off('motionFinish', handleFinish)
  }

  const handleFinish = () => {
    if (
      manager.state.currentGroup !== group ||
      manager.state.currentIndex !== index ||
      manager.state.currentPriority !== priority
    ) {
      return
    }
    dispose()
    onFinish()
  }

  const handleStart = (startedGroup: string, startedIndex: number) => {
    if (
      startedGroup !== group ||
      startedIndex !== index ||
      manager.state.currentPriority !== priority
    ) {
      return
    }
    manager.off('motionStart', handleStart)
    manager.on('motionFinish', handleFinish)
  }

  manager.on('motionStart', handleStart)
  return dispose
}
