<template>
  <div class="h-full overflow-y-auto p-6">
    <!-- 标题栏（与 SettingsAdvanceOther 一致：brand 色 + 下边框分隔） -->
    <header class="border-brand mb-6 flex items-center justify-between border-b pb-4">
      <h2 class="text-brand text-2xl font-semibold">{{ t("settings.asr.title") }}</h2>
      <span class="text-sm" :class="statusClass">{{ statusText }}</span>
    </header>

    <!-- 语音输入总开关（控制所有输入来源） -->
    <section class="mb-6">
      <Toggle
        :checked="localSettings.voice_input_enabled"
        @change="(v: boolean) => (localSettings.voice_input_enabled = v)"
      >
        <span class="font-medium">{{ t("settings.asr.voiceInput") }}</span>
        <span class="mt-0.5 block text-sm text-gray-300">{{
          t("settings.asr.voiceInputHint")
        }}</span>
      </Toggle>
    </section>

    <!-- 安卓端无全局快捷键，前端 keydown 也易被蓝牙键盘触发，故整体隐藏 -->
    <section v-if="!isAndroid()" class="mb-6">
      <div class="mb-1.5 flex items-center justify-between">
        <label class="text-sm font-medium">{{ t("settings.asr.pttKey") }}</label>
        <button
          type="button"
          class="hover:text-brand text-xs text-gray-300 transition-colors"
          @click="resetPttKey"
        >
          {{ t("settings.asr.pttKeyReset") }}
        </button>
      </div>
      <button
        type="button"
        :class="[
          'w-full rounded-lg border px-3 py-2.5 text-left text-sm transition-all duration-200',
          capturingPtt
            ? 'border-brand/60 text-brand bg-brand/10'
            : 'hover:border-brand/40 border-white/10 bg-white/10 text-white',
        ]"
        @click="startPttCapture"
      >
        {{ capturingPtt ? t("settings.asr.pttKeyCapture") : formatBinding(pttBinding) }}
      </button>
      <p class="mt-1.5 block text-sm text-gray-300">{{ t("settings.asr.pttKeyHint") }}</p>
      <p v-if="pttCaptureInvalid" class="mt-1 block text-sm text-red-400">
        {{ t("settings.asr.pttKeyInvalid") }}
      </p>
      <!-- 开启后在其他应用前台也能触发语音输入 -->
      <div class="mt-4 border-t border-white/10 pt-4">
        <Toggle
          :checked="localSettings.ptt_global"
          @change="(v: boolean) => (localSettings.ptt_global = v)"
        >
          <span class="font-medium">{{ t("settings.asr.pttGlobal") }}</span>
          <span class="mt-0.5 block text-sm text-gray-300">{{
            t("settings.asr.pttGlobalHint")
          }}</span>
        </Toggle>
        <p v-if="pttGlobalError" class="mt-1 block text-sm text-red-400">
          {{ pttGlobalError }}
        </p>
      </div>
    </section>

    <!-- 自动语音识别开关 -->
    <section class="mb-6">
      <Toggle
        :checked="localSettings.auto_listen"
        @change="(v: boolean) => (localSettings.auto_listen = v)"
      >
        <span class="font-medium">{{ t("settings.asr.autoListen") }}</span>
        <span class="mt-0.5 block text-sm text-gray-300">{{
          t("settings.asr.autoListenHint")
        }}</span>
      </Toggle>
    </section>

    <!-- VAD 静音计时（自动模式：停止说话后等多久才结束录音） -->
    <section class="mb-6">
      <label class="mb-1.5 block text-sm font-medium">{{ t("settings.asr.vadSilence") }}</label>
      <input
        type="number"
        min="100"
        max="3000"
        step="100"
        v-model.number="localSettings.vad_silence_ms"
        class="shadow-glass focus:border-brand focus:ring-brand/20 w-full rounded-lg border border-white/10 bg-white/10 px-3 py-2.5 text-sm text-white transition-all duration-200 focus:ring-2 focus:outline-none"
      />
      <p class="mt-1.5 block text-sm text-gray-300">{{ t("settings.asr.vadSilenceHint") }}</p>
    </section>

    <!-- 能量监测缓冲期（自动模式：TTS 播完恢复监听后多久内不触发录音） -->
    <section class="mb-6">
      <label class="mb-1.5 block text-sm font-medium">{{ t("settings.asr.energyWarmup") }}</label>
      <input
        type="number"
        min="0"
        max="2000"
        step="100"
        v-model.number="localSettings.energy_warmup_ms"
        class="shadow-glass focus:border-brand focus:ring-brand/20 w-full rounded-lg border border-white/10 bg-white/10 px-3 py-2.5 text-sm text-white transition-all duration-200 focus:ring-2 focus:outline-none"
      />
      <p class="mt-1.5 block text-sm text-gray-300">{{ t("settings.asr.energyWarmupHint") }}</p>
    </section>

    <!-- 详细 VAD 能量检测日志：逐帧打印 frame/prob/len，默认关闭，随本页自动保存 -->
    <section class="mb-6">
      <Toggle
        :checked="localSettings.vad_debug_log"
        @change="(v: boolean) => (localSettings.vad_debug_log = v)"
      >
        <span class="font-medium">{{ t("settings.asr.vadDebugLog") }}</span>
        <span class="mt-0.5 block text-sm text-gray-300">{{
          t("settings.asr.vadDebugLogHint")
        }}</span>
      </Toggle>
    </section>

    <!-- 识别完成后处理方式 -->
    <section class="mb-6">
      <div class="text-brand mb-3 font-medium">{{ t("settings.asr.sendMode.title") }}</div>
      <div class="space-y-2">
        <label
          v-for="opt in sendModeOptions"
          :key="opt.value"
          class="flex cursor-pointer items-center gap-2 text-sm"
        >
          <input
            type="radio"
            :value="opt.value"
            v-model="localSettings.send_mode"
            class="h-4 w-4 accent-(--accent-color)"
          />
          <span>{{ opt.label }}</span>
        </label>
      </div>
    </section>

    <!-- 全局热词：与场景角色热词取并集参与识别（同词取较高权重） -->
    <section class="mb-6">
      <div class="text-brand mb-3 font-medium">{{ t("settings.asr.globalHotword.title") }}</div>
      <button
        type="button"
        class="w-full cursor-pointer rounded-lg border border-white/10 bg-white/10 px-3 py-2.5 text-left text-sm text-white transition-all duration-200 hover:border-[#5e72e4]/40 hover:bg-white/15"
        @click="showGlobalHotword = true"
      >
        {{ t("settings.asr.globalHotword.open") }}
      </button>
      <p class="mt-1.5 text-sm text-gray-300">{{ t("settings.asr.globalHotword.hint") }}</p>
    </section>

    <!-- 热词库：素材库，供角色热词与全局热词挑选（拷贝式，不参与识别） -->
    <section class="mb-6">
      <div class="text-brand mb-3 font-medium">{{ t("settings.asr.hotwordLibrary.title") }}</div>
      <button
        type="button"
        class="w-full cursor-pointer rounded-lg border border-white/10 bg-white/10 px-3 py-2.5 text-left text-sm text-white transition-all duration-200 hover:border-[#5e72e4]/40 hover:bg-white/15"
        @click="showHotwordLibrary = true"
      >
        {{ t("settings.asr.hotwordLibrary.open") }}
      </button>
      <p class="mt-1.5 text-sm text-gray-300">{{ t("settings.asr.hotwordLibrary.hint") }}</p>
    </section>

    <!-- 识别服务 -->
    <section class="mb-6">
      <div class="text-brand mb-3 font-medium">{{ t("settings.asr.provider.title") }}</div>

      <!-- 识别服务选择：按模型平铺；模型在表单内选择的 provider 只占一项，判据数据驱动，不硬编码 provider id -->
      <label class="mb-1.5 block text-sm font-medium">{{
        t("settings.asr.provider.providerSelect")
      }}</label>
      <select
        v-model="activeServiceKey"
        class="shadow-glass focus:border-brand focus:ring-brand/20 mb-4 w-full rounded-lg border border-white/10 bg-white/10 px-3 py-2.5 text-sm text-sky-400 transition-all duration-200 focus:ring-2 focus:outline-none"
      >
        <optgroup v-for="g in serviceGroups" :key="g.provider.id" :label="g.provider.display_name">
          <option v-for="item in g.items" :key="item.key" :value="item.key">
            {{ item.label }}
          </option>
        </optgroup>
      </select>

      <!-- 热词容量是能力展示；llama-asr 无条数上限，只提示能力。须放在识别服务选择器下方，qwen 表单没有 model 字段 -->
      <p v-if="activeModel" class="mt-1.5 text-sm text-gray-300">
        {{
          activeModel.supports_inline_vocabulary && activeModel.max_hotword_count
            ? t("settings.asr.provider.inlineHotwordLimit", {
                count: activeModel.max_hotword_count,
              })
            : activeModel.supports_prompt_bias
              ? t("settings.asr.provider.promptBiasNoLimit")
              : t("settings.characterInfo.hotword.modelUnsupported", {
                  model: activeModel.id,
                })
        }}
      </p>

      <div v-if="activeProviderInfo" class="space-y-3">
        <div v-for="field in activeProviderInfo.config_fields" :key="field.key">
          <label class="mb-1.5 block text-sm font-medium">
            {{ field.label }}
            <!-- 分地域的字段标出当前地域，否则切地域后的空框容易被误读成 Key 丢失 -->
            <span v-if="field.kind === 'password_map' && activeRegion" class="text-gray-400">
              （{{ activeRegion.label }}）
            </span>
            <span v-if="field.required" class="text-red-500">*</span>
          </label>
          <!--
            field.key 是后端动态返回的字符串键（如 'api_key' / 'endpoint'），
            ProviderConfig 类型只声明了部分键，因此通过 unknown 双步转换为 Record<string, string>
            再索引（v-model 需要可写）。
            field.kind 与后端 ConfigFieldKind 对齐：text / password / number / boolean。
          -->
          <!-- 有动态模型清单时用下拉，拉取失败或无清单回退文本输入 -->
          <div v-if="field.key === 'model' && asrStore.models.length > 0" class="flex gap-2">
            <select
              v-model="providerCfgRecord[field.key]"
              class="shadow-glass focus:border-brand focus:ring-brand/20 flex-1 rounded-lg border border-white/10 bg-white/10 px-3 py-2.5 text-sm text-sky-400 transition-all duration-200 focus:ring-2 focus:outline-none"
            >
              <option v-for="m in asrStore.models" :key="m.id" :value="m.id">
                {{ m.display_name }}
              </option>
            </select>
            <button
              type="button"
              :title="t('settings.asr.provider.modelRefresh')"
              class="rounded-lg border border-white/15 bg-white/5 px-3 text-white/60 transition-colors hover:bg-white/10 hover:text-white/80"
              @click="refreshModels"
            >
              ↻
            </button>
          </div>
          <input
            v-else-if="field.kind === 'password'"
            type="password"
            v-model="providerCfgRecord[field.key]"
            class="shadow-glass focus:border-brand focus:ring-brand/20 w-full rounded-lg border border-white/10 bg-white/10 px-3 py-2.5 text-sm text-white transition-all duration-200 focus:ring-2 focus:outline-none"
          />
          <!-- 按地域分存的 API Key 输入框，只显示当前地域的值 -->
          <input
            v-else-if="field.kind === 'password_map'"
            type="password"
            v-model="activeApiKey"
            :placeholder="field.placeholder"
            class="shadow-glass focus:border-brand focus:ring-brand/20 w-full rounded-lg border border-white/10 bg-white/10 px-3 py-2.5 text-sm text-white transition-all duration-200 focus:ring-2 focus:outline-none"
          />
          <input
            v-else-if="field.kind === 'number'"
            type="number"
            v-model="providerCfgRecord[field.key]"
            :placeholder="field.placeholder"
            class="shadow-glass focus:border-brand focus:ring-brand/20 w-full rounded-lg border border-white/10 bg-white/10 px-3 py-2.5 text-sm text-white transition-all duration-200 focus:ring-2 focus:outline-none"
          />
          <label
            v-else-if="field.kind === 'boolean'"
            class="flex cursor-pointer items-center gap-2"
          >
            <input
              type="checkbox"
              v-model="providerCfgRecord[field.key]"
              class="h-4 w-4 accent-(--accent-color)"
            />
          </label>
          <!-- 下拉单选（地域等枚举字段）：选项由后端 config_fields[].options 提供 -->
          <select
            v-else-if="field.kind === 'select'"
            v-model="providerCfgRecord[field.key]"
            class="shadow-glass focus:border-brand focus:ring-brand/20 w-full rounded-lg border border-white/10 bg-white/10 px-3 py-2.5 text-sm text-sky-400 transition-all duration-200 focus:ring-2 focus:outline-none"
          >
            <option v-for="opt in field.options ?? []" :key="opt.value" :value="opt.value">
              {{ opt.label }}
            </option>
          </select>
          <input
            v-else
            type="text"
            v-model="providerCfgRecord[field.key]"
            :placeholder="field.placeholder"
            class="shadow-glass focus:border-brand focus:ring-brand/20 w-full rounded-lg border border-white/10 bg-white/10 px-3 py-2.5 text-sm text-white transition-all duration-200 focus:ring-2 focus:outline-none"
          />
          <!-- 后端 config_field 的 hint，地域、端点等字段依赖它 -->
          <p v-if="field.hint" class="mt-1.5 block text-sm text-gray-300">{{ field.hint }}</p>
          <p v-if="field.key === 'model' && modelListError" class="mt-1 text-sm text-red-400">
            {{ modelListError }}
          </p>
        </div>
        <div class="flex items-center gap-3">
          <button
            type="button"
            class="bg-brand rounded-lg px-4 py-2 text-sm text-white transition-colors duration-200 hover:bg-[#0056b3]"
            @click="testConnection"
          >
            {{
              testRecording
                ? t("settings.asr.provider.testingStop")
                : t("settings.asr.provider.test")
            }}
          </button>
          <p
            v-if="lastTestResult"
            class="max-w-md text-sm"
            :class="lastTestResult.ok ? 'text-green-400' : 'text-red-400'"
          >
            {{ lastTestResult.text }}
          </p>
        </div>
      </div>

      <!-- 流式识别开关：选中模型支持流式才可用 -->
      <div class="mt-4 border-t border-white/10 pt-4">
        <Toggle
          :checked="localSettings.stream_enabled"
          :disabled="!providerSupportsStreaming"
          @change="(v: boolean) => (localSettings.stream_enabled = v)"
        >
          <span class="font-medium">{{ t("settings.asr.streamMode") }}</span>
          <span class="mt-0.5 block text-sm text-gray-300">
            {{
              !providerSupportsStreaming
                ? t("settings.asr.streamNotSupported")
                : localSettings.active_provider === "llama-asr"
                  ? t("settings.asr.streamModeHintLocal")
                  : t("settings.asr.streamModeHint")
            }}
          </span>
        </Toggle>
      </div>
    </section>

    <!-- 状态面板：只保留 VAD 模型状态（init_asr 失败诊断的关键信号，
         麦克风状态在设置页恒为空闲无信息量，已移除） -->
    <section class="border-t border-white/10 pt-4 text-sm text-gray-300">
      <div>
        {{ t("settings.asr.status.vadLoaded") }}:
        <span :class="vadStateClass">{{ vadStateText }}</span>
      </div>
    </section>

    <GlobalHotwordDialog :visible="showGlobalHotword" @close="showGlobalHotword = false" />
    <HotwordLibraryDialog :visible="showHotwordLibrary" @close="showHotwordLibrary = false" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { useI18n } from "vue-i18n";

import {
  captureFromEvent,
  formatBinding,
  parsePttBinding,
  type ShortcutBinding,
} from "@/utils/shortcuts";
import { listen } from "@tauri-apps/api/event";
import { isAndroid } from "@/utils/platform";

import { Toggle } from "../../base";
import GlobalHotwordDialog from "@/components/settings/asr/GlobalHotwordDialog.vue";
import HotwordLibraryDialog from "@/components/settings/asr/HotwordLibraryDialog.vue";
import { useAsrStore } from "@/stores/modules/settings/asr";
import { useUIStore } from "@/stores/modules/ui/ui";
import { asrRecognizeWav, asrGetStatus } from "@/api/services/asr";
import { pcmToWavPcm16, trimSilencePcm } from "@/utils/asrAudio";
import { parseAsrError } from "@/utils/asrError";
import type {
  AsrSettings,
  SendMode,
  ProviderInfo,
  AsrRegionInfo,
  ProviderConfig,
  PttGlobalStatus,
} from "@/api/services/asr";

const { t, te } = useI18n();
const asrStore = useAsrStore();
const uiStore = useUIStore();

// 深拷贝表单副本（不能用 structuredClone —— Pinia reactive Proxy 会抛
// DataCloneError，导致 setup 崩溃整页空白；JSON 序列化无此问题）
const localSettings = ref<AsrSettings>(JSON.parse(JSON.stringify(asrStore.settings)));
const lastTestResult = ref<{ ok: boolean; text: string } | null>(null);
/** 全局热词与词库弹窗都走自己的命令，不写 AsrSettings，与本页自动保存无交互。 */
const showGlobalHotword = ref(false);
const showHotwordLibrary = ref(false);

/** 当前快捷键绑定：与 useAsrInput 同用 parsePttBinding，Enter / 非法 JSON 回退 F8 */
const pttBinding = computed<ShortcutBinding>(
  () => parsePttBinding(localSettings.value.ptt_key) ?? { key: "f8" },
);

const capturingPtt = ref(false);
const pttCaptureInvalid = ref(false);
let pttCaptureKeyHandler: ((e: KeyboardEvent) => void) | null = null;

function handlePttCaptureBlur() {
  // 失焦时退出捕获：避免 Alt+Tab 后捕获模式残留拦截按键
  endPttCapture();
}

function startPttCapture() {
  // 重入保护：捕获中再次点击（双击）不再重复注册监听器
  if (capturingPtt.value) return;
  capturingPtt.value = true;
  pttCaptureInvalid.value = false;
  window.addEventListener("blur", handlePttCaptureBlur);
  pttCaptureKeyHandler = (e: KeyboardEvent) => {
    // 捕获期间阻止默认行为（功能键系统行为等）
    e.preventDefault();
    e.stopPropagation();
    const result = captureFromEvent(e);
    if (result.kind === "ignore") return; // 纯修饰键，继续等待组合
    if (result.kind === "cancel") {
      endPttCapture();
      return;
    }
    let binding: ShortcutBinding;
    if (result.kind === "blocked") {
      // 无修饰字母/数字允许单键绑定；聊天输入时同时触发的问题由提示语说明
      binding = { key: e.key.toLowerCase() };
    } else {
      binding = result.binding;
    }
    // PTT 专用键位校验：仅 Enter 拒绝（聊天发送键，绑定后发消息误触发录音）
    if (binding.key === "enter") {
      pttCaptureInvalid.value = true;
      return;
    }
    // 写入 localSettings，现有 debounce watch 自动保存
    localSettings.value.ptt_key = JSON.stringify(binding);
    endPttCapture();
  };
  window.addEventListener("keydown", pttCaptureKeyHandler, true);
}

function endPttCapture() {
  capturingPtt.value = false;
  pttCaptureInvalid.value = false;
  window.removeEventListener("blur", handlePttCaptureBlur);
  if (pttCaptureKeyHandler) {
    window.removeEventListener("keydown", pttCaptureKeyHandler, true);
    pttCaptureKeyHandler = null;
  }
}

function resetPttKey() {
  endPttCapture();
  localSettings.value.ptt_key = '{"key":"f8"}';
}

/** 全局快捷键注册失败提示（后端按 state 判别；只有 failed 才提示，开关不自动回退） */
const pttGlobalError = ref("");
let unlistenGlobalStatus: (() => void) | null = null;
onMounted(async () => {
  unlistenGlobalStatus = await listen<PttGlobalStatus>("asr:ptt-global-status", (e) => {
    // 只有 failed 才提示；inactive（关开关 / 不在聊天界面）是正常状态，reason 为空
    pttGlobalError.value =
      e.payload.state === "failed"
        ? t("settings.asr.pttGlobalError", { reason: e.payload.reason })
        : "";
  });
});

onUnmounted(() => {
  unlistenGlobalStatus?.();
  endPttCapture();
});

// 设置抽屉用 v-show 隐藏（KeepAlive 不触发 onUnmounted），关闭时主动退出捕获，否则监听器会残留并吞掉聊天按键
watch(
  () => uiStore.showSettings,
  (open) => {
    if (!open) endPttCapture();
  },
);

let saveTimer: number | null = null;
/** 初始化完成标记：onMounted 赋值后置 true，跳过一次初始化触发的保存 */
let initialized = false;

const sendModeOptions = computed<{ value: SendMode; label: string }[]>(() => [
  { value: "fill_only", label: t("settings.asr.sendMode.fillOnly") },
  { value: "auto_send", label: t("settings.asr.sendMode.autoSend") },
]);

const activeProviderInfo = computed<ProviderInfo | undefined>(() =>
  asrStore.providers.find((p) => p.id === localSettings.value.active_provider),
);

// 扁平「识别服务」列表

/** provider 的 config_fields 含 model 字段时，模型在表单里选；判据数据驱动，不硬编码 provider id */
function picksModelInForm(p: ProviderInfo | undefined): boolean {
  return p?.config_fields.some((f) => f.key === "model") ?? false;
}

interface ServiceItem {
  /** 编码为 `${provider_id}::${model_id}`；model_id 为空 = 该 provider 的模型在表单里选 */
  key: string;
  label: string;
}

const serviceGroups = computed<{ provider: ProviderInfo; items: ServiceItem[] }[]>(() =>
  asrStore.providers.map((p) => {
    if (picksModelInForm(p)) {
      return { provider: p, items: [{ key: `${p.id}::`, label: p.display_name }] };
    }
    const models = asrStore.modelsByProvider[p.id] ?? [];
    // 模型清单为空（拉取失败 / 该地域无模型）也保留一项，否则该 provider 会从列表消失
    const items: ServiceItem[] =
      models.length > 0
        ? models.map((m) => ({ key: `${p.id}::${m.id}`, label: m.display_name }))
        : [{ key: `${p.id}::`, label: `${p.display_name}（模型未列出）` }];
    return { provider: p, items };
  }),
);

// config_field.key 是动态字符串，需桥接为 Record 才能用 v-model 写入任意键。
// 声明必须靠前：模板与 immediate watch 在 setup 阶段求值，晚声明会命中 TDZ 导致整页空白。
const providerCfg = computed(() => {
  const id = localSettings.value.active_provider;
  return localSettings.value.provider_configs[id] ?? { api_key: "", endpoint: "" };
});
const providerCfgRecord = computed(() => providerCfg.value as unknown as Record<string, string>);

/** 扁平列表选中值（provider::model），双向绑定；set 时同写 active_provider 与 model */
const activeServiceKey = computed({
  get(): string {
    const pid = localSettings.value.active_provider;
    const info = asrStore.providers.find((p) => p.id === pid);
    if (!info || picksModelInForm(info)) return `${pid}::`;
    const key = `${pid}::${providerCfgRecord.value["model"] ?? ""}`;
    // 配置的模型不在当前清单（历史模型或切地域后失效）时回退到首项，否则 select 会空白
    const group = serviceGroups.value.find((g) => g.provider.id === pid);
    return group?.items.some((i) => i.key === key) ? key : (group?.items[0]?.key ?? key);
  },
  set(value: string) {
    const sep = value.indexOf("::");
    const pid = sep < 0 ? value : value.slice(0, sep);
    const mid = sep < 0 ? "" : value.slice(sep + 2);
    localSettings.value.active_provider = pid;
    if (!mid) return;
    // 写入新 provider 自己的配置（providerCfg 此刻仍是旧 provider 的）
    ensureProviderConfig(pid);
    const record = localSettings.value.provider_configs[pid] as unknown as Record<string, string>;
    record["model"] = mid;
  },
});

// 地域端点默认值以后端 ProviderInfo.regions 为准，前端不硬编码域名

/** 当前 provider 的可选地域；无地域概念的 provider（如 llama-asr）为空数组 */
const providerRegions = computed(() => activeProviderInfo.value?.regions ?? []);

/** 当前生效的地域信息（按配置的 region id 查，缺失回退第一个） */
const activeRegion = computed<AsrRegionInfo | undefined>(() => {
  const regions = providerRegions.value;
  if (regions.length === 0) return undefined;
  return regions.find((r) => r.id === (providerCfg.value.region ?? "")) ?? regions[0];
});

/** 当前 provider 的原始配置记录；providerCfg 带兜底且类型联合，取不到 api_keys，也不该写入 */
function currentProviderCfg(): ProviderConfig | undefined {
  return localSettings.value.provider_configs[localSettings.value.active_provider];
}

/**
 * 当前地域的 API Key。Key 按地域存储在 api_keys[region.id]，未配置时返回空串而不回退其他地域。
 * 地域 id 必须与后端 DashScopeRegion::parse 的回退规则一致，否则会静默取空。
 */
const activeApiKey = computed({
  get(): string {
    const region = activeRegion.value;
    if (!region) return "";
    return currentProviderCfg()?.api_keys?.[region.id] ?? "";
  },
  set(value: string) {
    const cfg = currentProviderCfg();
    const region = activeRegion.value;
    if (!cfg || !region) return;
    cfg.api_keys = { ...(cfg.api_keys ?? {}), [region.id]: value };
  },
});

/** 该端点值是否恰好等于某个地域的默认端点（用于判断「未被用户手改过」） */
function isKnownRegionEndpoint(value: string): boolean {
  return providerRegions.value.some((r) => r.http_endpoint === value || r.ws_endpoint === value);
}

/** 把端点字段指向当前地域默认值；只改空值或已知地域默认端点，不覆盖用户手填的域名 / 代理 */
function applyRegionEndpoints() {
  const region = activeRegion.value;
  if (!region) return;
  const record = providerCfg.value as unknown as Record<string, string>;
  const pairs: [string, string][] = [
    ["endpoint", region.http_endpoint],
    ["ws_endpoint", region.ws_endpoint],
  ];
  for (const [key, next] of pairs) {
    const cur = (record[key] ?? "").trim();
    if (!cur || isKnownRegionEndpoint(cur)) {
      record[key] = next;
    }
  }
}

/**
 * 校验当前模型在新地域下可用，不可用时回退地域默认模型（否则请求会 400）。
 * 模型清单为空（拉取失败）时不改，避免误清用户配置。
 */
function ensureModelValidForRegion() {
  const models = asrStore.models;
  if (models.length === 0) return;
  const cfg = localSettings.value.provider_configs[localSettings.value.active_provider];
  if (!cfg) return;
  const cur = (cfg.model ?? "").trim();
  if (!cur || models.some((m) => m.id === cur)) return;
  const fallback = models.find((m) => m.is_default) ?? models[0];
  cfg.model = fallback.id;
}

/** 应用模型预设：填 model，并按当前地域补齐端点 */
function applyAsrPreset(model: string) {
  const cfg = localSettings.value.provider_configs[localSettings.value.active_provider];
  const m = asrStore.models.find((x) => x.id === model);
  if (!cfg || !m) return;
  cfg.model = m.id;
  applyRegionEndpoints();
}

/** 模型列表拉取失败信息（llama-asr 服务未启动等），显示在模型字段下方 */
const modelListError = ref("");

/** 拉取指定 provider 的模型清单；失败时记录错误（store 会把该 provider 的条目清空） */
async function loadModels(id: string) {
  try {
    // 传表单当前地域而非让后端读配置：保存有 500ms debounce，否则会拿到旧地域的清单
    await asrStore.reloadModels(id, localSettings.value.provider_configs[id]?.region);
    modelListError.value = "";
  } catch (e) {
    const info = parseAsrError(e);
    modelListError.value = t("settings.asr.provider.modelListFailed", {
      err: info.detail ?? info.code,
    });
  }
}

/** 手动刷新模型列表（llama-server 换模型/重启后重新拉取） */
function refreshModels() {
  void loadModels(localSettings.value.active_provider);
}

/** 当前生效模型：配置非空取配置，否则默认模型 */
const activeModel = computed(() => {
  const id = localSettings.value.provider_configs[localSettings.value.active_provider]?.model ?? "";
  return asrStore.models.find((m) => m.id === id) ?? asrStore.models.find((m) => m.is_default);
});
// 流式开关可用性：当前生效模型的流式能力（模型级权威判定）
const providerSupportsStreaming = computed(() => activeModel.value?.supports_streaming ?? false);

// 切到不支持流式的模型 → 自动关闭流式开关（避免录音时后端报错）
watch(activeModel, (m) => {
  if (!m?.supports_streaming && localSettings.value.stream_enabled) {
    localSettings.value.stream_enabled = false;
  }
});

// 流式开关与模型同步：打开切到流式模型，关闭切到非流式模型（模型与协议强绑定）
watch(
  () => localSettings.value.stream_enabled,
  (on) => {
    const m = activeModel.value;
    if (!m) return;
    if (on && !m.supports_streaming) {
      const sm = asrStore.models.find((x) => x.supports_streaming);
      if (sm) applyAsrPreset(sm.id);
    } else if (!on && m.supports_streaming) {
      const nm = asrStore.models.find((x) => !x.supports_streaming);
      if (nm) applyAsrPreset(nm.id);
    }
  },
);

// provider 切换 / 挂载时显式初始化缺失配置（不在渲染期突变 state）
function ensureProviderConfig(id: string) {
  const cfg = localSettings.value.provider_configs[id] ?? {
    api_key: "",
    endpoint: "",
    model: "",
    extra: {},
  };
  localSettings.value.provider_configs[id] = cfg;
  // 用后端 default_value 补齐空字段以开箱即用，已有值不覆盖
  const record = cfg as unknown as Record<string, string>;
  const info = asrStore.providers.find((p) => p.id === id);
  info?.config_fields.forEach((f) => {
    if (f.default_value && !record[f.key]) {
      record[f.key] = f.default_value;
    }
  });
}
// provider / 地域变更统一入口。必须是单个 watch 且先 ensureProviderConfig 再
// applyRegionEndpoints，否则默认端点可能被判为用户手填而留下错误地址。
watch(
  [() => localSettings.value.active_provider, () => providerCfg.value.region],
  async ([id]) => {
    ensureProviderConfig(id);
    applyRegionEndpoints();
    await loadModels(id);
    ensureModelValidForRegion();
  },
  { immediate: true },
);

const statusText = computed(() => {
  if (!asrStore.lastError) return t("settings.asr.status.ready");
  // 有错误：显示错误摘要（i18n 文案 + 原始 code 兜底）
  const errKey = `settings.asr.errors.${asrStore.lastError}`;
  const errText = te(errKey) ? t(errKey) : asrStore.lastError;
  return `${t("settings.asr.status.notReady")}（${errText}）`;
});
const statusClass = computed(() => (asrStore.lastError ? "text-red-400" : "text-green-400"));

const vadStateText = computed(() =>
  asrStore.vadLoaded ? t("settings.asr.status.vadLoadedOk") : t("settings.asr.status.vadLoadedNo"),
);
const vadStateClass = computed(() => (asrStore.vadLoaded ? "text-green-400" : "text-red-400"));

onMounted(async () => {
  await asrStore.load();
  // 顶层浅拷贝设置，v-model 写入不影响 store
  localSettings.value = { ...asrStore.settings };
  // 初始化赋值不算用户更改：先赋值后置 initialized，sync watch 在赋值时仍看到 false，
  // 避免打开设置页就触发一次无意义的保存
  initialized = true;
  // 用查询获取 VAD 状态：asr://vad_ready 发射早于监听器注册，事件不缓存
  asrGetStatus()
    .then((s) => {
      asrStore.setVadLoaded(s.vad_loaded);
      // 同样用查询获取全局注册状态：asr:ptt-global-status 不缓存，可能错过失败事件
      if (localSettings.value.ptt_global && !s.ptt_global_ok) {
        pttGlobalError.value = t("settings.asr.pttGlobalNotRegistered");
      }
    })
    .catch((e) => console.warn("[ASR] 查询状态失败:", e));
});

watch(
  localSettings,
  (s) => {
    // 初始化赋值不触发保存；flush:'sync' 保证赋值时 initialized 仍为 false
    if (!initialized) return;
    // 用户改动后清除旧的注册失败提示，若仍失败事件会重新带来新原因
    pttGlobalError.value = "";
    if (saveTimer !== null) clearTimeout(saveTimer);
    saveTimer = window.setTimeout(() => {
      void asrStore.save(s).catch((e) => console.warn("[ASR] autosave failed:", e));
    }, 500);
  },
  { deep: true, flush: "sync" },
);

// ── 测试连接：完整识别链路（录音 4 秒 → 16k PCM → recognize → 显示文本） ──
// 不用 MediaRecorder（webm 在 WebView2 decodeAudioData 会失败），
// 与 useAsrInput 同路径：ScriptProcessor 直接采 16k f32 PCM → pcmToWavPcm16。
const testRecording = ref(false);
let testStream: MediaStream | null = null;
let testCtx: AudioContext | null = null;
let testProcessor: ScriptProcessorNode | null = null;
let testPcm: number[] = [];
let testTimer: number | null = null;

async function testConnection() {
  try {
    // 先保存表单值：确保后端 registry 用的是用户刚填的 api_key（消除 500ms debounce 竞态）
    await asrStore.save(localSettings.value);

    if (!testRecording.value) {
      // 第一阶段：开始录音（4 秒后自动停止）
      testStream = await navigator.mediaDevices.getUserMedia({
        audio: {
          sampleRate: 16000,
          channelCount: 1,
          echoCancellation: true,
          noiseSuppression: true,
        },
      });
      testCtx = new AudioContext({ sampleRate: 16000 });
      const src = testCtx.createMediaStreamSource(testStream);
      testProcessor = testCtx.createScriptProcessor(1024, 1, 1);
      src.connect(testProcessor);
      // 输出接零增益节点而非 destination，避免把采集流回放
      const silence = testCtx.createGain();
      silence.gain.value = 0;
      testProcessor.connect(silence);
      silence.connect(testCtx.destination);
      testPcm = [];
      testProcessor.onaudioprocess = (e) => {
        testPcm.push(...e.inputBuffer.getChannelData(0));
      };
      testRecording.value = true;
      lastTestResult.value = { ok: true, text: t("settings.asr.provider.testing") };
      testTimer = window.setTimeout(() => void finishTestRecording(), 4000);
      return;
    }

    // 第二阶段：手动停止（点按钮提前结束）
    await finishTestRecording();
  } catch (e: unknown) {
    // 录音初始化失败（权限等）或识别失败
    const info = parseAsrError(e);
    const key = `settings.asr.errors.${info.code}`;
    let text = te(key) ? t(key) : info.code || String(e);
    if (info.detail) {
      text += `（${info.detail}）`;
    }
    lastTestResult.value = { ok: false, text };
    cleanupTestRecording();
  }
}

/** 停止录音 → PCM 合成 WAV → 走完整识别链路 → 显示识别文本 */
async function finishTestRecording() {
  if (testTimer !== null) {
    clearTimeout(testTimer);
    testTimer = null;
  }
  const pcm = testPcm;
  cleanupTestRecording();
  try {
    // 裁剪首尾静音，只送语音段
    const wav = pcmToWavPcm16(trimSilencePcm(pcm));
    if (wav.byteLength <= 44) {
      lastTestResult.value = { ok: false, text: t("settings.asr.provider.testNoSpeech") };
      return;
    }
    const result = await asrRecognizeWav({
      providerId: localSettings.value.active_provider,
      wavBytes: Array.from(wav),
    });
    lastTestResult.value = {
      ok: true,
      text: t("settings.asr.provider.testResult", {
        text: result.text || t("settings.asr.provider.testNoSpeech"),
      }),
    };
  } catch (e: unknown) {
    const info = parseAsrError(e);
    const key = `settings.asr.errors.${info.code}`;
    let text = te(key) ? t(key) : info.code || String(e);
    if (info.detail) {
      text += `（${info.detail}）`;
    }
    lastTestResult.value = { ok: false, text };
  }
}

function cleanupTestRecording() {
  if (testTimer !== null) {
    clearTimeout(testTimer);
    testTimer = null;
  }
  try {
    testProcessor?.disconnect();
  } catch {
    /* ignore */
  }
  testProcessor = null;
  void testCtx?.close().catch(() => {});
  testCtx = null;
  testStream?.getTracks().forEach((t) => t.stop());
  testStream = null;
  testPcm = [];
  testRecording.value = false;
}
</script>
