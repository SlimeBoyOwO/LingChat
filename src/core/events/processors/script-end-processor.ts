import type { IEventProcessor } from '../event-processor'
import type { ScriptEndEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { WebSocketMessageTypes } from '../../../types'
import { useAdventureStore } from '@/stores/modules/adventure'
import router from '@/router'

export default class ScriptEndProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === WebSocketMessageTypes.SCRIPT_END
  }

  async processEvent(event: ScriptEndEvent): Promise<void> {
    // completed === false 表示剧本是因为出错被中止的。仍然要退出剧情模式把
    // UI 放出来，但不能把羁绊记为完成。旧版本没有这个字段，按完成处理。
    const completed = event.completed !== false

    const adventureStore = useAdventureStore()
    if (completed && adventureStore.inProgressAdventures.length > 0) {
      // 按道理来讲，应该只有一个进行的剧本哈，但是为了保险起见，还是遍历一下
      for (const adventure of adventureStore.inProgressAdventures) {
        adventureStore.markAdventureCompleted(adventure.adventure_folder)
      }
    }

    const gameStore = useGameStore()
    gameStore.exitStoryMode()
    const uiStore = useUIStore()
    uiStore.showPlayerHintLine = ''
    // 恐怖特效/突脸不得带出剧本外
    uiStore.resetHorrorEffects()

    // 剧本自然结束：回主菜单收场（DDLC 式 full_restart 回标题），而不是无缝
    // 落进自由对话——剧本的收场叙事（"你回到了剧本列表"）与自由对话界面不搭，
    // 且玩家可能误以为剧本还在跑。玩家手动切自由对话的路径不经过这里，不受影响。
    if (router.currentRoute.value.path === '/chat') {
      await router.push('/')
    }
  }
}
