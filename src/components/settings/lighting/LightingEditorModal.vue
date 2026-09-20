<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="show"
        class="fixed inset-0 z-9999 flex items-center justify-center bg-black/50 p-4
          backdrop-blur-md"
        @click="close"
      >
        <div
          class="relative flex max-h-[92dvh] w-full max-w-5xl flex-col overflow-hidden rounded-3xl
            border border-white/20 bg-slate-900/85 shadow-2xl backdrop-blur-2xl md:flex-row"
          @click.stop
        >
          <!-- ============ 左栏：调参 ============ -->
          <div
            class="flex min-w-0 flex-1 flex-col border-r border-white/10 md:w-[440px] md:shrink-0"
          >
            <div
              class="flex shrink-0 items-center justify-between border-b border-white/10 bg-white/10
                p-4"
            >
              <h3 class="text-base leading-none font-bold text-white">
                {{
                  editing
                    ? $t("settings.background.lighting.editor.titleEdit")
                    : $t("settings.background.lighting.editor.titleNew")
                }}
              </h3>
              <button
                @click="close"
                class="flex items-center justify-center rounded-full p-2 text-white/50
                  transition-colors hover:bg-red-500/20 hover:text-white"
              >
                <svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </button>
            </div>

            <div class="flex-1 space-y-4 overflow-y-auto p-4">
              <!-- 内置预设改不动，先说清楚保存下来的是新的一条 -->
              <div
                v-if="isBuiltinSource"
                class="rounded-xl border border-amber-400/30 bg-amber-400/10 px-3 py-2 text-[11px]
                  leading-snug text-amber-100/80"
              >
                {{ $t("settings.background.lighting.editor.builtinForkTip") }}
              </div>
              <!-- 名称 / 描述 / 心情 -->
              <div class="space-y-2">
                <input
                  v-model="name"
                  :placeholder="$t('settings.background.lighting.editor.namePlaceholder')"
                  maxlength="24"
                  class="w-full rounded-xl border border-white/10 bg-black/40 px-3 py-2 text-sm
                    text-white placeholder-white/30 focus:border-amber-400/60 focus:ring-1
                    focus:ring-amber-400/40 focus:outline-none"
                />
                <input
                  v-model="description"
                  :placeholder="$t('settings.background.lighting.editor.descriptionPlaceholder')"
                  maxlength="60"
                  class="w-full rounded-xl border border-white/10 bg-black/40 px-2.5 py-1.5 text-xs
                    text-white placeholder-white/30 focus:border-amber-400/60 focus:outline-none"
                />
                <input
                  v-model="mood"
                  :placeholder="$t('settings.background.lighting.editor.moodPlaceholder')"
                  class="w-full rounded-xl border border-white/10 bg-black/40 px-2.5 py-1.5 text-xs
                    text-white placeholder-white/30 focus:border-amber-400/60 focus:outline-none"
                />
                <div class="text-[11px] leading-snug text-white/35">
                  {{ $t("settings.background.lighting.editor.moodHint") }}
                </div>
              </div>

              <!-- 各个光影层：与场景编辑器共用同一套控件 -->
              <LightingControls :params="draft" />

              <button
                @click="resetAll"
                class="text-[11px] text-white/40 underline transition-colors hover:text-white/80"
              >
                {{ $t("settings.background.lighting.editor.reset") }}
              </button>
            </div>

            <div
              class="flex shrink-0 items-center justify-between gap-2 border-t border-white/10
                bg-white/5 p-3"
            >
              <button
                @click="revert"
                class="text-[11px] text-white/40 underline transition-colors hover:text-white/80"
              >
                {{ $t("settings.background.lighting.editor.revert") }}
              </button>
              <div class="flex items-center gap-2">
                <Button
                  @click="close"
                  class="border border-white/20 bg-transparent! text-sm! text-white/70!
                    hover:bg-white/10! hover:text-white!"
                >
                  {{ $t("settings.background.lighting.editor.cancel") }}
                </Button>
                <Button
                  @click="save"
                  :disabled="!name.trim() || saving"
                  class="min-w-[70px] border-none bg-amber-500! text-sm! text-white
                    shadow-[0_0_10px_rgba(245,158,11,0.5)] hover:bg-amber-400! disabled:opacity-50
                    disabled:shadow-none"
                >
                  {{
                    saving
                      ? $t("settings.background.lighting.editor.saving")
                      : $t("settings.background.lighting.editor.save")
                  }}
                </Button>
              </div>
            </div>
          </div>

          <!-- ============ 右栏：实时预览 ============ -->
          <div class="flex min-w-0 flex-1 flex-col bg-black/30 p-5">
            <span class="mb-1 shrink-0 text-xs font-bold tracking-widest text-white/40 uppercase">
              {{ $t("settings.background.lighting.editor.previewTitle") }}
            </span>
            <div class="mb-3 shrink-0 text-[11px] leading-snug text-white/35">
              {{ $t("settings.background.lighting.editor.previewHint") }}
              <span v-if="!bgSrc" class="text-amber-200/70">
                {{ $t("settings.background.lighting.editor.previewNoBg") }}
              </span>
            </div>
            <div
              class="relative min-h-[16rem] flex-1 overflow-hidden rounded-xl border border-white/10
                bg-slate-800"
              style="isolation: isolate"
            >
              <!-- 混合层要有一张不透明的底才能算出「打光」；没有场景图时透明底会让
                   screen / soft-light 直接变成一层色块，预览和真实画面完全对不上。 -->
              <div class="absolute inset-0 bg-slate-800"></div>
              <!-- 背景 + 背景滤镜；没有背景图时用中性底代替，滤镜照样看得见 -->
              <div class="absolute inset-0" :style="bgFilterStyle">
                <img v-if="bgSrc" :src="bgSrc" class="h-full w-full object-cover" alt="" />
                <div v-else class="h-full w-full" :style="neutralBackdrop"></div>
              </div>
              <div
                v-if="previewPlan.bloom"
                class="pointer-events-none absolute inset-0"
                :class="previewPlan.bloom.className"
                :style="previewPlan.bloom.style"
              >
                <img v-if="bgSrc" :src="bgSrc" class="h-full w-full object-cover" alt="" />
                <div v-else class="h-full w-full" :style="neutralBackdrop"></div>
              </div>
              <div
                v-if="previewPlan.bgOverlay"
                class="pointer-events-none absolute inset-0"
                :style="previewPlan.bgOverlay"
              ></div>
              <img
                v-if="avatarSrc"
                :src="avatarSrc"
                class="absolute bottom-0 left-1/2 h-[88%] -translate-x-1/2 object-contain"
                :style="previewPlan.characterFilter ? { filter: previewPlan.characterFilter } : {}"
                alt=""
              />
              <div
                v-if="previewPlan.stageOverlay"
                class="pointer-events-none absolute inset-0"
                :style="previewPlan.stageOverlay"
              ></div>
              <template v-if="previewPlan.directional">
                <div
                  class="pointer-events-none absolute inset-0 z-20"
                  :class="previewPlan.directional.lit.className"
                  :style="previewPlan.directional.lit.style"
                ></div>
                <div
                  class="pointer-events-none absolute inset-0 z-20"
                  :class="previewPlan.directional.shadow.className"
                  :style="previewPlan.directional.shadow.style"
                ></div>
              </template>
              <div
                v-if="previewPlan.grade"
                class="pointer-events-none absolute inset-0 z-20"
                :class="previewPlan.grade.className"
                :style="previewPlan.grade.style"
              ></div>
              <div
                v-if="previewPlan.vignette"
                class="pointer-events-none absolute inset-0 z-20"
                :class="previewPlan.vignette.className"
                :style="previewPlan.vignette.style"
              ></div>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
  import { computed, nextTick, reactive, ref, watch } from "vue";
  import type { CSSProperties } from "vue";
  import { useI18n } from "vue-i18n";
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { Button } from "../../base";
  import LightingControls from "./LightingControls.vue";
  import type { LightingPreset } from "../../../api/services/lighting";
  import { applyLighting, clearLighting, saveLightingPreset } from "../../../api/services/lighting";
  import type { LightingParams } from "../../../api/services/scene";
  import { useGameStore } from "../../../stores/modules/game";
  import { useLightingStore } from "../../../stores/modules/lighting";
  import { useUIStore } from "../../../stores/modules/ui/ui";
  import { useDialogStore } from "../../../stores/modules/ui/dialog";
  import { EMOTION_CONFIG_EMO } from "../../../controllers/emotion/config";
  import {
    angleTowardLight,
    blankLighting,
    cloneLighting,
    planLighting,
  } from "../../../utils/lighting";

  const props = defineProps<{ show: boolean; editing: LightingPreset | null }>();
  const emit = defineEmits<{ close: []; saved: [id: string] }>();

  const { t } = useI18n();
  const gameStore = useGameStore();
  const uiStore = useUIStore();
  const lightingStore = useLightingStore();
  const dialogStore = useDialogStore();

  const L = "settings.background.lighting.editor";

  const draft = reactive<LightingParams>(blankLighting());
  const name = ref("");
  const description = ref("");
  const mood = ref("");
  const saving = ref(false);
  /** 底稿来自内置预设：内置的不能改，保存只能另存成用户自己的新预设。 */
  const isBuiltinSource = ref(false);

  /** 打开时的画面状态：取消要还回去，保存则交给父组件套用新预设。 */
  let snapshot: { preset: string | null; params: LightingParams } | null = null;
  /** 打开时那盏灯的参数，「重置为原来效果」退回到这里。 */
  let initial: LightingParams = blankLighting();
  let dirty = false;
  let savedFlag = false;
  let suspend = false;

  // ---- 实时预览：走运行时覆盖通道，主窗口和投屏窗口都会跟着变 ----

  const previewPlan = computed(() => planLighting(draft));

  const bgFilterStyle = computed<CSSProperties>(() =>
    previewPlan.value.backgroundFilter ? { filter: previewPlan.value.backgroundFilter } : {}
  );

  let timer: ReturnType<typeof setTimeout> | undefined;
  watch(
    () => draft,
    () => {
      if (suspend) return;
      dirty = true;
      clearTimeout(timer);
      timer = setTimeout(pushPreview, 120);
    },
    { deep: true }
  );

  async function pushPreview() {
    try {
      await applyLighting({ params: buildParams() });
    } catch (e) {
      console.warn("[Lighting] 预览推送失败:", e);
    }
  }

  function buildParams(): LightingParams {
    return {
      ...cloneLighting(draft),
      light_angle: Math.round(angleTowardLight(draft.light_x, draft.light_y) * 10) / 10,
    };
  }

  // ---- 预览素材：拿当前场景背景 + 立绘，没有就只按纯色底看 ----

  /**
   * 背景必须跟 `GameBackground.vue` 取同一份：主画面用的是 `uiStore.currentBackground`
   * （切场景、换背景、角色默认图都会写它），而场景表里的 `background` 字段经常是空的。
   * 取后者会让预览没有背景图，于是背景滤镜和泛光全都作用不到任何东西上——用户调完
   * 亮度对比度看不到反应，以为功能坏了。
   */
  const bgSrc = computed(() => {
    const bg = uiStore.currentBackground;
    if (
      !bg ||
      bg.startsWith("http://") ||
      bg.startsWith("https://") ||
      bg.startsWith("@/") ||
      bg.startsWith("data:")
    ) {
      return bg || "";
    }
    return convertFileSrc(bg);
  });

  /**
   * 没有背景图时的中性底：留出亮部、中间调和暗部，亮度 / 对比度 / 饱和度 / 暖色调
   * 才有可分辨的对象。纯灰或纯色的底看不出染色，也看不出压暗到什么程度。
   */
  const neutralBackdrop: CSSProperties = {
    background:
      "radial-gradient(circle at 72% 22%, #e2bd8c 0%, rgba(226,189,140,0) 42%)," +
      "linear-gradient(158deg, #7f90a8 0%, #47516600 52%)," +
      "linear-gradient(158deg, #475166 0%, #1a1f29 100%)",
  };
  const avatarSrc = ref("");

  async function resolveAvatar() {
    avatarSrc.value = "";
    const roleId = gameStore.currentInteractRoleId;
    const role = roleId ? gameStore.gameRoles[roleId] : null;
    if (!role) return;
    const clothesName =
      role.clothesName === "默认" || !role.clothesName ? "default" : role.clothesName;
    try {
      const path: string = await invoke("get_avatar_file", {
        characterFolder: role.character_folder,
        emotion: EMOTION_CONFIG_EMO[role.emotion] || "正常",
        clothesName,
      });
      avatarSrc.value = convertFileSrc(path);
    } catch {
      avatarSrc.value = "";
    }
  }

  // ---- 打开 / 关闭 ----

  watch(
    () => props.show,
    (v) => {
      if (!v) return;
      suspend = true;
      savedFlag = false;
      dirty = false;
      const from = props.editing?.params ?? lightingStore.baseParams ?? blankLighting();
      // 两份独立克隆：共用一个对象会让改 draft 连带改掉「重置为原来效果」的基准。
      initial = cloneLighting(from);
      Object.assign(draft, cloneLighting(from));
      isBuiltinSource.value = !!props.editing && !props.editing.custom;
      name.value = props.editing
        ? isBuiltinSource.value
          ? t(`${L}.forkName`, { name: props.editing.name })
          : props.editing.name
        : "";
      description.value = props.editing?.description ?? "";
      mood.value = (props.editing?.mood ?? []).join(" ");
      const ov = lightingStore.override;
      snapshot = ov ? { preset: ov.preset, params: cloneLighting(ov.params) } : null;
      resolveAvatar();
      // 等这一轮 deep watch 走完再解锁，否则初始化会被当成用户改动而推出预览
      void nextTick(() => {
        suspend = false;
      });
    }
  );

  function resetAll() {
    Object.assign(draft, blankLighting());
  }

  /** 退回打开编辑器时那盏灯：编辑已有预设时不能退到「当前全局预设」，那是另一盏。 */
  function revert() {
    Object.assign(draft, cloneLighting(initial));
  }
  async function close() {
    clearTimeout(timer);
    if (savedFlag) {
      emit("close");
      return;
    }
    if (dirty) {
      try {
        if (snapshot) await applyLighting({ preset: snapshot.preset, params: snapshot.params });
        else await clearLighting();
      } catch (e) {
        console.error("[Lighting] 还原预览前状态失败:", e);
      }
    }
    emit("close");
  }

  async function save() {
    const trimmed = name.value.trim();
    if (!trimmed) {
      dialogStore.alert(t(`${L}.nameRequired`));
      return;
    }
    saving.value = true;
    try {
      const saved = await saveLightingPreset({
        name: trimmed,
        description: description.value.trim(),
        mood: mood.value
          .split(/[,，\s]+/)
          .map((s) => s.trim())
          .filter(Boolean)
          .slice(0, 8),
        params: buildParams(),
        // 内置预设不许覆盖：以它为底稿调出来的东西只能是一条新的「我的预设」。
        replaceId: props.editing?.custom ? props.editing.id : null,
      });
      await lightingStore.refreshPresets();
      savedFlag = true;
      emit("saved", saved.id);
    } catch (e) {
      dialogStore.alert(t(`${L}.saveFailed`, { msg: String(e) }));
    } finally {
      saving.value = false;
    }
  }
</script>

<style scoped>
  .modal-enter-active,
  .modal-leave-active {
    transition: opacity 0.25s ease;
  }

  .modal-enter-from,
  .modal-leave-to {
    opacity: 0;
  }
</style>
