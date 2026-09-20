<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="show"
        class="fixed inset-0 z-9998 flex items-center justify-center bg-black/40 p-4
          backdrop-blur-md"
        @click="$emit('close')"
      >
        <div
          class="relative flex max-h-[92dvh] w-full max-w-5xl flex-col overflow-hidden rounded-3xl
            border border-white/20 bg-slate-900/85 shadow-2xl backdrop-blur-2xl md:flex-row"
          @click.stop
        >
          <!-- ====== 左栏：表单 ====== -->
          <!-- 尺寸与光影编辑器同一套：两个弹窗调的是同一份参数，窗口不一样大会很割裂 -->
          <div
            class="flex min-w-0 flex-1 flex-col border-r border-white/10 md:w-[440px] md:shrink-0"
          >
            <!-- Header -->
            <div
              class="flex shrink-0 items-center justify-between border-b border-white/10 bg-white/10
                p-4"
            >
              <h3 class="text-lg leading-none font-bold text-white">
                {{
                  mode === "create"
                    ? $t("settings.sceneEdit.title.create")
                    : $t("settings.sceneEdit.title.update")
                }}
              </h3>
              <button
                @click="$emit('close')"
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

            <!-- Scrollable body -->
            <div class="flex-1 space-y-4 overflow-y-auto p-4">
              <!-- 场景名称 -->
              <section>
                <label
                  class="mb-1.5 flex items-center gap-2 text-xs font-bold tracking-widest
                    text-gray-200/90 uppercase opacity-80"
                >
                  <span class="h-3 w-1 rounded-full bg-orange-500"></span>
                  {{ $t("settings.sceneEdit.label.sceneName") }}
                </label>
                <input
                  v-model="formData.sceneName"
                  :placeholder="$t('settings.sceneEdit.placeholder.sceneName')"
                  class="w-full rounded-xl border border-white/10 bg-black/40 px-3 py-2 text-sm
                    text-white placeholder-white/30 transition-all focus:border-indigo-400
                    focus:ring-1 focus:ring-indigo-400/50 focus:outline-none"
                />
              </section>

              <!-- 场景图片 -->
              <section>
                <label
                  class="mb-1.5 flex items-center gap-2 text-xs font-bold tracking-widest
                    text-gray-200/90 uppercase opacity-80"
                >
                  <span class="h-3 w-1 rounded-full bg-indigo-500"></span>
                  {{ $t("settings.sceneEdit.label.sceneImage") }}
                </label>
                <div class="flex gap-2">
                  <select
                    v-model="formData.sceneImage"
                    class="flex-1 appearance-none rounded-xl border border-white/10 bg-black/40 px-3
                      py-2 text-sm text-white transition-all focus:border-indigo-400 focus:ring-1
                      focus:ring-indigo-400/50 focus:outline-none"
                  >
                    <option value="" class="bg-slate-800 text-white/50">
                      {{ $t("settings.sceneEdit.option.selectBackground") }}
                    </option>
                    <option
                      v-for="bg in backgrounds"
                      :key="bg.url"
                      :value="bg.url"
                      class="bg-slate-800 text-white"
                    >
                      {{ bg.title }}
                    </option>
                  </select>
                  <Button
                    @click="$emit('upload')"
                    class="rounded-xl border border-white/20 bg-white/10! px-4 text-sm!
                      whitespace-nowrap text-white! hover:bg-white/20!"
                  >
                    {{ $t("settings.sceneEdit.button.upload") }}
                  </Button>
                </div>
              </section>

              <!-- 场景描述 -->
              <section>
                <label
                  class="mb-1.5 flex items-center gap-2 text-xs font-bold tracking-widest
                    text-gray-200/90 uppercase opacity-80"
                >
                  <span class="h-3 w-1 rounded-full bg-emerald-500"></span>
                  {{ $t("settings.sceneEdit.label.sceneDescription") }}
                </label>
                <textarea
                  v-model="formData.sceneDescription"
                  :placeholder="$t('settings.sceneEdit.placeholder.sceneDescription')"
                  rows="2"
                  class="w-full resize-none rounded-xl border border-white/10 bg-black/40 px-3 py-2
                    text-sm text-white placeholder-white/30 transition-all focus:border-indigo-400
                    focus:ring-1 focus:ring-indigo-400/50 focus:outline-none"
                ></textarea>
              </section>

              <!-- 光影参数（可折叠） -->
              <section>
                <button
                  @click="showLighting = !showLighting"
                  class="mb-1.5 flex w-full items-center justify-between gap-2 text-xs font-bold
                    tracking-widest text-gray-200/90 uppercase opacity-80 transition-opacity
                    hover:opacity-100"
                >
                  <span class="flex items-center gap-2">
                    <span class="h-3 w-1 rounded-full bg-purple-500"></span>
                    {{ $t("settings.sceneEdit.lighting.title") }}
                  </span>
                  <svg
                    :class="['h-3.5 w-3.5 transition-transform', showLighting ? 'rotate-90' : '']"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                  >
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M9 5l7 7-7 7"
                    />
                  </svg>
                </button>

                <div v-show="showLighting" class="space-y-2.5 pl-3">
                  <label class="flex cursor-pointer items-center gap-2">
                    <input
                      v-model="formData.lightingEnabled"
                      type="checkbox"
                      class="h-3.5 w-3.5 rounded accent-purple-500"
                    />
                    <span class="text-xs text-white/70">{{
                      $t("settings.sceneEdit.lighting.enableForScene")
                    }}</span>
                  </label>

                  <!-- 预设收成一条下拉：挑一盏灯抄进本场景，挑完照样能继续手动微调 -->
                  <LightingPresetSelect
                    :current-label="presetCurrentLabel"
                    :none-label="$t('settings.sceneEdit.lighting.presetNone')"
                    :selected-id="matchedPreset?.id ?? ''"
                    @picked="applyPreset"
                    @saved="applyPreset"
                  />

                  <div
                    class="rounded-xl border border-amber-400/30 bg-amber-400/10 px-3 py-2
                      text-[11px] leading-snug text-amber-100/80"
                  >
                    {{ $t("settings.sceneEdit.lighting.priorityTip") }}
                  </div>

                  <template v-if="formData.lightingEnabled">
                    <!-- 与光影编辑器共用同一套控件：以前这里只认最早 11 个字段，
                         进阶层看不到，保存还会把它们抹掉。 -->
                    <LightingControls :params="lightingDraft" />

                    <button
                      @click="resetLighting"
                      class="text-[11px] text-white/40 underline transition-colors
                        hover:text-white/80"
                    >
                      {{ $t("settings.sceneEdit.button.resetDefault") }}
                    </button>
                  </template>
                </div>
              </section>
            </div>

            <!-- Footer -->
            <div
              class="flex shrink-0 items-center justify-end gap-2 border-t border-white/10
                bg-white/5 p-3"
            >
              <Button
                @click="$emit('close')"
                class="border border-white/20 bg-transparent! text-sm! text-white/70!
                  hover:bg-white/10! hover:text-white!"
              >
                {{ $t("settings.sceneEdit.button.cancel") }}
              </Button>
              <Button
                @click="handleSubmit"
                :disabled="!formData.sceneName.trim()"
                class="min-w-[70px] border-none bg-indigo-500! text-sm! text-white
                  shadow-[0_0_10px_rgba(99,102,241,0.5)] hover:bg-indigo-400! disabled:opacity-50
                  disabled:shadow-none"
              >
                {{
                  mode === "create"
                    ? $t("settings.sceneEdit.button.create")
                    : $t("settings.sceneEdit.button.update")
                }}
              </Button>
            </div>
          </div>

          <!-- ====== 右栏：实时预览 ====== -->
          <div class="flex min-w-0 flex-1 flex-col bg-black/30 p-5">
            <span class="mb-3 shrink-0 text-xs font-bold tracking-widest text-white/40 uppercase">{{
              $t("settings.sceneEdit.preview.title")
            }}</span>
            <!-- 与光影编辑器同样：混合层要有一张不透明的底才算得出「打光」 -->
            <div
              class="relative min-h-[16rem] flex-1 overflow-hidden rounded-xl border border-white/10
                bg-slate-800"
              style="isolation: isolate"
            >
              <div class="absolute inset-0 bg-slate-800"></div>
              <!-- 背景图（应用背景滤镜） -->
              <div class="absolute inset-0" :style="previewBgFilterStyle">
                <img
                  v-if="selectedBackgroundPreview"
                  :src="selectedBackgroundPreview"
                  class="h-full w-full object-cover"
                  alt=""
                />
                <div
                  v-else
                  class="flex h-full w-full items-center justify-center text-sm text-white/20"
                >
                  {{ $t("settings.sceneEdit.preview.placeholder") }}
                </div>
              </div>
              <!-- bloom 泛光：同一张背景复制一层，模糊提亮后 screen 叠回去 -->
              <div
                v-if="previewPlan.bloom && selectedBackgroundPreview"
                class="pointer-events-none absolute inset-0"
                :class="previewPlan.bloom.className"
                :style="previewPlan.bloom.style"
              >
                <img :src="selectedBackgroundPreview" class="h-full w-full object-cover" alt="" />
              </div>
              <div
                v-if="previewPlan.bgOverlay"
                class="pointer-events-none absolute inset-0"
                :style="previewPlan.bgOverlay"
              ></div>
              <!-- 角色立绘（角色滤镜 + 位置/缩放） -->
              <img
                v-if="previewAvatarUrl"
                :src="previewAvatarUrl"
                class="absolute"
                :style="previewCharStyle"
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
            <!-- 信息标签 -->
            <div class="mt-2 flex shrink-0 gap-3 text-xs text-white/40">
              <span v-if="!selectedBackgroundPreview">{{
                $t("settings.sceneEdit.preview.noBackground")
              }}</span>
              <span v-if="previewAvatarUrl">{{
                $t("settings.sceneEdit.preview.avatarLoaded")
              }}</span>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
  import { computed, reactive, ref, watch } from "vue";
  import type { CSSProperties } from "vue";
  import { useI18n } from "vue-i18n";
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { Button } from "../../base";
  import LightingControls from "../lighting/LightingControls.vue";
  import LightingPresetSelect from "../lighting/LightingPresetSelect.vue";
  import { useGameStore } from "../../../stores/modules/game";
  import { useLightingStore } from "../../../stores/modules/lighting";
  import { EMOTION_CONFIG_EMO } from "../../../controllers/emotion/config";
  import type { BackgroundImageInfo } from "../../../types";
  import type { LightingParams } from "../../../api/services/scene";
  import type { LightingPreset } from "../../../api/services/lighting";
  import {
    angleTowardLight,
    blankLighting,
    cloneLighting,
    planLighting,
  } from "../../../utils/lighting";

  const props = defineProps<{
    show: boolean;
    mode: "create" | "update";
    backgrounds: BackgroundImageInfo[];
    initialData?: {
      sceneName: string;
      sceneImage: string | null;
      sceneDescription: string;
      lighting?: LightingParams | null;
    };
  }>();

  const emit = defineEmits<{
    close: [];
    submit: [
      data: {
        sceneName: string;
        sceneImage: string | null;
        sceneDescription: string;
        lighting?: LightingParams | null;
      },
    ];
    upload: [];
  }>();

  const gameStore = useGameStore();
  const lightingStore = useLightingStore();
  const { t } = useI18n();

  // ---- reactive form ----

  // 只用 interface 保持类型安全
  interface FormData {
    sceneName: string;
    sceneImage: string;
    sceneDescription: string;
    lightingEnabled: boolean;
    [key: string]: string | number | boolean; // 索引签名以支持 v-model="formData[key]"
  }

  const formData = reactive<FormData>({
    sceneName: "",
    sceneImage: "",
    sceneDescription: "",
    lightingEnabled: false,
  });

  /**
   * 场景自带的那盏灯。所有光影字段都住在这里，交给共用的 LightingControls 直接改；
   * 之前拆成十几个散装表单字段，旧字段名对不上进阶层，保存时就把新层抹掉了。
   */
  const lightingDraft = reactive<LightingParams>(blankLighting());

  // ---- sub-state ----

  const showLighting = ref(false);
  const previewAvatarUrl = ref("");

  // ---- reset ----

  function resetLighting() {
    Object.assign(lightingDraft, blankLighting());
  }

  // ---- 预设下拉 ----

  /** 选中的预设是「抄一份参数进场景」，不是引用：之后预设怎么改都不影响本场景。 */
  function applyPreset(preset: LightingPreset | null) {
    if (!preset) {
      formData.lightingEnabled = false;
      resetLighting();
      return;
    }
    showLighting.value = true;
    formData.lightingEnabled = true;
    // 老预设可能缺新加的字段，用空白灯光垫底补齐，滑块才不会停在 0。
    Object.assign(lightingDraft, cloneLighting({ ...blankLighting(), ...preset.params }));
  }

  /**
   * 草稿是否与某个预设逐字段相同 —— 只用于回显名字。
   * `light_angle` 由灯位算出来，不参与比对，否则拖一下灯位就算改过。
   */
  function sameLighting(a: unknown, b: unknown): boolean {
    if (typeof a === "number" || typeof b === "number") {
      return typeof a === "number" && typeof b === "number" && Math.abs(a - b) < 1e-4;
    }
    if (a === b) return true;
    if (typeof a !== "object" || typeof b !== "object" || !a || !b) return false;
    const x = a as Record<string, unknown>;
    const y = b as Record<string, unknown>;
    const keys = new Set([...Object.keys(x), ...Object.keys(y)]);
    for (const k of keys) {
      if (k === "light_angle") continue;
      if (!sameLighting(x[k], y[k])) return false;
    }
    return true;
  }

  const matchedPreset = computed(() =>
    formData.lightingEnabled
      ? (lightingStore.presets.find((p) => sameLighting(p.params, lightingDraft)) ?? null)
      : null
  );

  const presetCurrentLabel = computed(() => {
    if (matchedPreset.value) return matchedPreset.value.name;
    return formData.lightingEnabled ? t("settings.sceneEdit.lighting.presetCustom") : "";
  });

  // ---- role avatar ----

  async function resolveAvatar() {
    previewAvatarUrl.value = "";
    const roleId = gameStore.currentInteractRoleId;
    if (!roleId) return;
    const role = gameStore.gameRoles[roleId];
    if (!role) return;
    const clothesName =
      role.clothesName === "默认" || !role.clothesName ? "default" : role.clothesName;
    const mappedEmotion = EMOTION_CONFIG_EMO[role.emotion] || "正常";
    try {
      const path: string = await invoke("get_avatar_file", {
        characterFolder: role.character_folder,
        emotion: mappedEmotion,
        clothesName,
      });
      previewAvatarUrl.value = convertFileSrc(path);
    } catch {
      previewAvatarUrl.value = "";
    }
  }

  // ---- init from initialData ----

  watch(
    () => props.show,
    (val) => {
      if (val && props.initialData) {
        formData.sceneName = props.initialData.sceneName;
        formData.sceneImage = props.initialData.sceneImage || "";
        formData.sceneDescription = props.initialData.sceneDescription;
        const l = props.initialData.lighting;
        formData.lightingEnabled = !!l;
        // 老场景存的时候还没有进阶层，用空白灯光垫底补齐，免得滑块停在 0 而不是默认值。
        Object.assign(lightingDraft, cloneLighting({ ...blankLighting(), ...l }));
        showLighting.value = !!l;
        resolveAvatar();
      } else if (val) {
        formData.sceneName = "";
        formData.sceneImage = "";
        formData.sceneDescription = "";
        formData.lightingEnabled = false;
        resetLighting();
        showLighting.value = false;
        resolveAvatar();
      }
    }
  );

  // ---- preview computed ----

  const selectedBackgroundPreview = computed(() => {
    if (!formData.sceneImage) return null;
    return convertFileSrc(formData.sceneImage);
  });

  /** 预览与真实画面走同一个渲染器：这里看到的灯，保存后就是那个样子。 */
  const previewPlan = computed(() => planLighting(formData.lightingEnabled ? lightingDraft : null));

  const previewBgFilterStyle = computed<CSSProperties | undefined>(() => {
    if (!formData.lightingEnabled) return undefined;
    const filter = previewPlan.value.backgroundFilter;
    return filter ? { filter } : undefined;
  });

  const previewCharPosition = computed(() => {
    const roleId = gameStore.currentInteractRoleId;
    if (!roleId) return {};
    const role = gameStore.gameRoles[roleId];
    if (!role) return {};
    return {
      left: `calc(50% + ${role.offsetX || 0}px)`,
      top: `${role.offsetY || 0}px`,
      transform: `translateX(-50%) scale(${role.scale || 1})`,
      transformOrigin: "center 0%",
    };
  });

  const previewCharStyle = computed(() => {
    const pos = previewCharPosition.value;
    const style: Record<string, string> = {
      left: pos.left || "50%",
      top: pos.top || "0px",
      transform: pos.transform || "translateX(-50%) scale(1)",
      transformOrigin: pos.transformOrigin || "center 0%",
      maxHeight: "100%",
      maxWidth: "100%",
    };
    const filter = previewPlan.value.characterFilter;
    if (filter) {
      style.filter = filter;
    }
    return style;
  });

  // ---- submit ----

  const handleSubmit = () => {
    let lighting: LightingParams | null = null;
    if (formData.lightingEnabled) {
      // 整份草稿原样带走，一个字段都不挑：漏掉哪个，哪个就在下次读取时清零。
      lighting = {
        ...cloneLighting(lightingDraft),
        light_angle:
          Math.round(angleTowardLight(lightingDraft.light_x, lightingDraft.light_y) * 10) / 10,
      };
    }
    emit("submit", {
      sceneName: formData.sceneName.trim(),
      sceneImage: formData.sceneImage || null,
      sceneDescription: formData.sceneDescription.trim(),
      lighting,
    });
  };
</script>
