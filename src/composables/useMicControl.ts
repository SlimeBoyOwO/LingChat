/**
 * 麦克风按钮的状态与行为 —— 主界面 GameDialog 与桌宠 GameRolesStage 共用。
 *
 * 此前两处各写了一份三层语义完全相同的实现：
 *   - auto_listen 模式开 **且** 总开关开 → mic 按钮 = 功能开关（暂停/恢复监听，只动
 *     运行态，不改模式设置）
 *   - 总开关关（自动模式已停）→ 退化为手动录音（idle 开始 / recording 停止）
 *   - 总开关是语音输入的总闸，关掉时手动 mic 一并禁用
 *
 * 唯一真实差异是图标形态：主界面把图标当字符串名用（模板里再映射），桌宠直接用
 * lucide 组件。因此这里只给语义名 `micIconName`，图标由各组件自行映射。
 *
 * 文案两处本来就用同一批 `game.dialog.*` 词条，故一并放在这里。
 */
import { computed, type ComputedRef } from "vue";
import { useI18n } from "vue-i18n";
import { useAsrStore } from "@/stores/modules/settings/asr";
import { useAsrInput } from "@/composables/useAsrInput";
import type { AsrPhase } from "@/api/services/asr";

/** 图标语义名：各组件映射到自己的图标体系 */
export type MicIconName = "mic" | "mic-off";

export interface UseMicControlApi {
  phase: ComputedRef<AsrPhase>;
  /** auto_listen 模式是否开启（设置项，非运行态） */
  autoListenOn: ComputedRef<boolean>;
  /** 自动监听当前是否真的在跑（运行态） */
  autoListenActive: ComputedRef<boolean>;
  /** mic 按钮是否可用（原 canStartMic） */
  micEnabled: ComputedRef<boolean>;
  micTitle: ComputedRef<string>;
  micIconName: ComputedRef<MicIconName>;
  /** 点按 mic：切换功能开关，或手动起停录音。异常内部吞掉，不向外抛 */
  toggleRecording: () => void;
}

export function useMicControl(): UseMicControlApi {
  const { t } = useI18n();
  const asrInput = useAsrInput();
  const asrStore = useAsrStore();

  const phase = computed(() => asrInput.phase.value as AsrPhase);
  const autoListenOn = computed(() => asrStore.settings.auto_listen);
  const autoListenActive = computed(() => asrInput.autoListenActive.value);

  // 此时 mic 按钮表达的是「功能开关」而非录音：监听中 → 点击暂停；已暂停 → 点击恢复
  const functionSwitchOn = computed(
    () => autoListenOn.value && asrStore.settings.voice_input_enabled,
  );

  const micIconName = computed<MicIconName>(() =>
    functionSwitchOn.value && autoListenActive.value ? "mic-off" : "mic",
  );

  const micTitle = computed(() => {
    if (functionSwitchOn.value) {
      return autoListenActive.value
        ? t("game.dialog.asrAutoOff") // 监听中：暂停
        : t("game.dialog.asrAutoResume"); // 已暂停：恢复
    }
    return phase.value === "recording"
      ? t("game.dialog.recordingStop")
      : t("game.dialog.voiceInput");
  });

  // enabled 条件（与 useAsrInput.canStartAsr 对齐）：
  // - 功能开关可用时始终可点
  // - 录音中可点（用于停止）
  // - 总开关关 → 整体禁用；显示锁只挡 auto 触发，手动不受限（故 forManual = true）
  const micEnabled = computed(
    () =>
      functionSwitchOn.value || phase.value === "recording" || asrInput.canStartAsr(false, true),
  );

  const toggleRecording = () => {
    if (functionSwitchOn.value) {
      asrInput.toggleAutoListenFunction();
      return;
    }
    if (phase.value === "idle") {
      // 会话忙时 start() 会 reject，静默忽略（与重构前一致）
      void asrInput.start("button").catch((err) => console.warn("[ASR] toggle failed:", err));
    } else if (phase.value === "recording") {
      asrInput.stop();
    }
  };

  return {
    phase,
    autoListenOn,
    autoListenActive,
    micEnabled,
    micTitle,
    micIconName,
    toggleRecording,
  };
}
