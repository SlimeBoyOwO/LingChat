import type { IEventProcessor } from "../event-processor";
import type { ScriptBackgroundEffectEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";
import { canonicalEffectKey } from "../../../components/game/standard/particles";

export default class BackgroundEffectProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "background_effect";
  }

  async processEvent(event: ScriptBackgroundEffectEvent): Promise<void> {
    const gameStore = useGameStore();
    const uiStore = useUIStore();

    // 处理对话逻辑
    gameStore.currentStatus = "presenting";

    // 剧本只写一个特效值，落到氛围层还是天气层由注册表决定。
    // 顺带纠大小写：AI 写剧本常写成 thunderstorm，不纠就会被当成未知特效静默清空
    uiStore.applyEffectValue(canonicalEffectKey(event.effect) ?? event.effect);
  }
}
