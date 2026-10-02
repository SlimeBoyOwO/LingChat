import type { IEventProcessor } from "../event-processor";
import type { ScriptBackgroundEffectEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";
import { useSettingsStore } from "../../../stores/modules/settings";
import { canonicalEffectKey, isWeatherEffect } from "../../../components/game/standard/particles";

// Only the newest effect may restore/reset the shared layer.
let effectFlashSeq = 0;
let activeTimer: number | null = null;
let detachAbort: (() => void) | null = null;
let stableEffect: string | null = null;
let stableWeather: string | null = null;

function detachCurrentAbort() {
  detachAbort?.();
  detachAbort = null;
}

export default class BackgroundEffectProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === "background_effect";
  }

  async processEvent(event: ScriptBackgroundEffectEvent, signal?: AbortSignal): Promise<void> {
    if (signal?.aborted) return;
    const gameStore = useGameStore();
    const uiStore = useUIStore();

    gameStore.currentStatus = "presenting";

    // A new effect owns the layer; stale timers/listeners may no longer mutate it.
    detachCurrentAbort();
    if (activeTimer !== null) {
      window.clearTimeout(activeTimer);
      activeTimer = null;
    }

    // BSOD 的剧本自带彩蛋文本（trace 行/独白）；切到非 BSOD 特效时清掉
    if (event.effect.split("+").includes("BSOD")) {
      uiStore.bsodText = event.text ?? "";
      uiStore.bsodEcho = event.echo ?? "";
    } else if (uiStore.bsodText || uiStore.bsodEcho) {
      uiStore.bsodText = "";
      uiStore.bsodEcho = "";
    }

    // 落到氛围层还是天气层由注册表决定，还原时也要按同一层判断，所以先算一次
    const appliedEffect = canonicalEffectKey(event.effect) ?? event.effect;
    const appliedIsWeather = isWeatherEffect(appliedEffect);

    if (stableEffect === null) {
      const display = useSettingsStore().display;
      stableEffect = display.backgroundEffect || "None";
      stableWeather = display.weatherEffect || "None";
    }
    const duration = event.duration;
    if (duration <= 0) {
      // 常驻特效：基线跟着它走（applyEffectValue 会把另一层清成 None），
      // 之后的限时特效就还原到这里
      stableEffect = appliedIsWeather ? "None" : appliedEffect;
      stableWeather = appliedIsWeather ? appliedEffect : "None";
    }

    const mySeq = ++effectFlashSeq;
    // 剧本只写一个特效值，落到氛围层还是天气层由注册表决定。
    // 顺带纠大小写：AI 写剧本常写成 thunderstorm，不纠就会被当成未知特效静默清空。
    // 恐怖层组合（"Glitch+BloodDrip"）canonicalEffectKey 逐段规范化后整体走氛围层。
    uiStore.applyEffectValue(appliedEffect);

    if (signal) {
      const onAbort = () => {
        if (mySeq !== effectFlashSeq) return;
        effectFlashSeq += 1;
        if (activeTimer !== null) {
          window.clearTimeout(activeTimer);
          activeTimer = null;
        }
        // Error/manual exit must remove the active horror layer immediately and
        // forget this run's stable baseline.
        stableEffect = null;
        stableWeather = null;
        uiStore.resetHorrorEffects();
      };
      signal.addEventListener("abort", onAbort, { once: true });
      detachAbort = () => signal.removeEventListener("abort", onAbort);
    }

    if (duration > 0) {
      activeTimer = window.setTimeout(() => {
        activeTimer = null;
        detachCurrentAbort();
        if (mySeq !== effectFlashSeq) return;
        // 值可能落在天气层（applyEffectValue 会把另一层清成 None），
        // 所以按落点那一层判断限时特效是否还在，两层都还原回基线
        const display = useSettingsStore().display;
        const current = appliedIsWeather ? display.weatherEffect : display.backgroundEffect;
        if (current === appliedEffect) {
          uiStore.setBackgroundEffect(stableEffect || "None");
          uiStore.setWeatherEffect(stableWeather || "None");
        }
      }, duration * 1000);
    }
  }
}
