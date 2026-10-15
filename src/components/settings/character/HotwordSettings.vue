<template>
  <div class="space-y-4">
    <!-- 模型不吃热词：当前模型的两条链路（即时热词 / prompt 偏置）都不支持 -->
    <p
      v-if="modelCapability && !modelCapability.supportsHotwords"
      class="rounded-xl border border-amber-400/30 bg-amber-500/10 px-3 py-2 text-[15px] leading-relaxed text-amber-200"
    >
      ⚠
      {{
        $t("settings.characterInfo.hotword.modelUnsupported", { model: modelCapability.modelId })
      }}
    </p>

    <!-- 条数超限同样会被后端整表拒（用户粘大段文本批量新增时最容易撞上） -->
    <p
      v-if="draft.hotwords.length > MAX_HOTWORDS_PER_ROLE"
      class="rounded-xl border border-red-400/30 bg-red-500/10 px-3 py-2 text-[12px] leading-relaxed text-red-200"
    >
      {{
        $t("settings.characterInfo.hotword.tooMany", {
          count: draft.hotwords.length,
          max: MAX_HOTWORDS_PER_ROLE,
        })
      }}
    </p>

    <div class="flex items-center justify-between">
      <h3 class="text-sm font-bold text-white/70">
        {{ $t("settings.characterInfo.hotword.listTitle") }}
      </h3>
      <div class="flex items-center gap-2">
        <button
          class="cursor-pointer rounded-lg border border-white/15 bg-white/10 px-3 py-1.5 text-xs text-white transition-colors hover:bg-white/20"
          @click="showPicker = true"
        >
          {{ $t("settings.asr.hotwordLibrary.pickButton") }}
        </button>
        <button
          class="cursor-pointer rounded-lg border-none bg-[#5e72e4] px-3 py-1.5 text-xs text-white transition-colors hover:bg-[#4a5acf]"
          @click="addHotword"
        >
          + {{ $t("settings.characterInfo.hotword.add") }}
        </button>
      </div>
    </div>

    <!-- 紧凑列：词条占满剩余宽度，权重是固定宽的小控件 -->
    <div v-if="draft.hotwords.length > 0" class="flex items-center gap-2 text-[13px] text-white/40">
      <span class="flex-1">{{ $t("settings.characterInfo.hotword.colText") }}</span>
      <span class="w-24 shrink-0">{{ $t("settings.characterInfo.hotword.colWeight") }}</span>
      <span class="w-6 shrink-0"></span>
    </div>

    <div v-for="(item, idx) in draft.hotwords" :key="idx">
      <div class="flex items-center gap-2">
        <input
          v-model="item.text"
          type="text"
          class="hotword-control flex-1 rounded-xl border bg-black/20 px-3 py-2 text-sm text-white transition-all duration-200 outline-none"
          :class="isHotwordTextOverlong(item.text) ? 'border-red-400/60' : 'border-white/10'"
          @change="commitText(item)"
        />
        <!-- 权重是离散 6 档（`[1, 5] ∪ {50}`），数字框会让人以为 6..49 也有效 -->
        <select
          v-model.number="item.weight"
          class="hotword-control w-24 shrink-0 rounded-xl border border-white/10 bg-black/20 px-2 py-2 text-sm text-white transition-all duration-200 outline-none"
          @change="clampWeight(item)"
        >
          <option
            v-for="w in HOTWORD_WEIGHT_OPTIONS"
            :key="w"
            :value="w"
            class="bg-[#333] text-white"
          >
            {{ weightLabel(w) }}
          </option>
        </select>
        <button
          :title="$t('settings.characterInfo.hotword.delete')"
          class="h-6 w-6 shrink-0 cursor-pointer rounded-full border-none bg-red-500/20 text-xs text-red-400 transition-colors hover:bg-red-500/40"
          @click="removeHotword(idx)"
        >
          ×
        </button>
      </div>
      <!-- 超长条目保存必被后端拒（整表回滚）；在点保存前就标出来。两套判据各自的
           计数口径不同（字符数 vs 片段数），提示文案必须跟着分支走。 -->
      <p v-if="hotwordLimitKind(item.text) !== 'ok'" class="mt-1 text-[12px] text-red-300">
        {{
          hotwordLimitKind(item.text) === "segments"
            ? $t("settings.characterInfo.hotword.tooLongSegments", {
                count: hotwordSegmentCount(item.text),
                max: MAX_HOTWORD_SEGMENTS,
              })
            : $t("settings.characterInfo.hotword.tooLongChars", {
                count: hotwordCharCount(item.text),
                max: MAX_HOTWORD_CHARS,
              })
        }}
      </p>
    </div>

    <div v-if="draft.hotwords.length === 0" class="py-8 text-center text-sm text-white/40">
      {{ $t("settings.characterInfo.hotword.empty") }}
    </div>

    <!-- 选择器嵌在宿主弹窗之上：宿主可能是角色设置弹窗（z-50）或全局弹窗（z-[10000]） -->
    <HotwordLibraryPicker
      :visible="showPicker"
      :existing="draft.hotwords"
      @close="showPicker = false"
      @add="appendFromLibrary"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import {
  MAX_HOTWORD_CHARS,
  MAX_HOTWORD_SEGMENTS,
  MAX_HOTWORDS_PER_ROLE,
  hotwordCharCount,
  hotwordLimitKind,
  hotwordSegmentCount,
  isHotwordTextOverlong,
  type LibraryWord,
  type RoleHotword,
  type RoleHotwordDraft,
} from "@/api/services/hotword";
import HotwordLibraryPicker from "@/components/settings/asr/HotwordLibraryPicker.vue";
import {
  HOTWORD_DEFAULT_WEIGHT,
  HOTWORD_WEIGHT_OPTIONS,
  clampRowWeight,
  commitRowText,
  // 整表去重：与 Task 6 抽出的共享实现同源（本文件不再自带单条版 dedupe）
  dedupeRows,
} from "@/utils/hotwordRows";

const props = defineProps<{
  /** 弹窗正在编辑的角色：新建热词行的 `role_id` 取此值（角色侧是真实 roleId，全局侧是 0 占位）。 */
  roleId: number;
  /**
   * 当前**实际生效**的识别模型能力（父组件按同步 / 流式分别取值）。
   * `null` = 模型清单尚未加载——不提示，宁可不提示也不误报。
   */
  modelCapability: { modelId: string; supportsHotwords: boolean } | null;
  /** 草稿：编辑只进这里，提交由父组件负责。 */
  modelValue: RoleHotwordDraft;
}>();

// v-model 的草稿**就地编辑**（与 clothes 那段的 `v-model="item.name"` 同款）：父组件
// 的 ref 是深响应式的，嵌套变更会同步回去；不需要回传事件，提交由父组件把握。
const draft = computed(() => props.modelValue);

const { t } = useI18n();

const addHotword = () => {
  props.modelValue.hotwords.push({
    text: "",
    weight: HOTWORD_DEFAULT_WEIGHT,
    role_id: props.roleId,
  });
};

const removeHotword = (idx: number) => {
  props.modelValue.hotwords.splice(idx, 1);
};

// 下面是共享实现的薄包装：模板里的调用点保持不变
const commitText = (item: RoleHotword) => commitRowText(props.modelValue.hotwords, item);
const clampWeight = (item: RoleHotword) => clampRowWeight(item);

/** 档位标签：50 是「超级热词」（厂商口径），其余显示数字。 */
const weightLabel = (w: number) =>
  w === 50 ? t("settings.characterInfo.hotword.weight.super") : String(w);

const showPicker = ref(false);

/** 从词库添加：追加到列表尾部，`role_id` 沿用宿主给的值（角色侧是真实 roleId，
 *  全局侧是 0 占位——绝不能在角色侧用 0，那是 User 的真实 role_id）。 */
const appendFromLibrary = (words: LibraryWord[]) => {
  for (const word of words) {
    props.modelValue.hotwords.push({
      text: word.text,
      weight: word.weight,
      role_id: props.roleId,
    });
  }
  // 并入后可能与草稿里已有的撞键（选择器挡的是**打开那一刻**的状态，词条期间可能被改）。
  // 复用共享的整表去重（泛型签名同时吃 `RoleHotword` 与 `LibraryWord`）。
  dedupeRows(props.modelValue.hotwords);
};
</script>

<style scoped>
/* 与弹窗内联表单同款聚焦态（父组件的 .form-control 是 scoped 的，子组件拿不到） */
.hotword-control:focus {
  border-color: #79d9ff;
  background: rgba(0, 0, 0, 0.3);
  box-shadow: 0 0 0 3px rgba(121, 217, 255, 0.2);
}
</style>
