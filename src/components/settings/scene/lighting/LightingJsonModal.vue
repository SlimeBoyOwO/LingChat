<script setup lang="ts">
import { ref, watch } from "vue";

const props = defineProps<{
  isOpen: boolean;
  initialJson: string;
  allPresetsJson: string;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "apply", jsonString: string): void;
}>();

const textContent = ref("");
const statusMsg = ref("");
const fileInputRef = ref<HTMLInputElement | null>(null);

watch(
  () => props.initialJson,
  (val) => {
    textContent.value = val;
    statusMsg.value = "";
  },
);

function handleCopy() {
  if (!textContent.value.trim()) return;
  navigator.clipboard.writeText(textContent.value).then(() => {
    statusMsg.value = "✓ 已复制到剪贴板";
  });
}

function handleDownloadSingle() {
  if (!textContent.value.trim()) return;
  const blob = new Blob([textContent.value], {
    type: "application/json;charset=utf-8",
  });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = "scene-lighting-config.json";
  document.body.appendChild(a);
  a.click();
  a.remove();
  statusMsg.value = "✓ 单项配置已下载";
}

function handleDownloadAll() {
  textContent.value = props.allPresetsJson;
  const blob = new Blob([props.allPresetsJson], {
    type: "application/json;charset=utf-8",
  });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = "presets-v4.0-calibrated.json";
  document.body.appendChild(a);
  a.click();
  a.remove();
  statusMsg.value = "✓ 已打包全量 15 款预设并下载";
}

function handlePickFile() {
  fileInputRef.value?.click();
}

function onFileSelected(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file) return;
  const reader = new FileReader();
  reader.onload = (evt) => {
    textContent.value = String(evt.target?.result || "");
    statusMsg.value = `✓ 已载入本地文件: ${file.name}`;
  };
  reader.readAsText(file, "utf-8");
}

function handleApply() {
  const raw = textContent.value.trim();
  if (!raw) {
    statusMsg.value = "⚠ 请先输入或载入有效 JSON 内容";
    return;
  }
  try {
    JSON.parse(raw);
    emit("apply", raw);
    emit("close");
  } catch (err: unknown) {
    statusMsg.value = `⚠ JSON 格式错误: ${(err as Error).message}`;
  }
}
</script>

<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-4 backdrop-blur-sm"
    @click.self="emit('close')"
  >
    <div
      class="flex h-[520px] w-full max-w-[680px] flex-col overflow-hidden rounded-xl border border-[#2b3347] bg-[#10131a] shadow-2xl"
    >
      <!-- Head -->
      <div class="flex h-12 shrink-0 items-center justify-between border-b border-[#1f2636] px-4">
        <span class="text-xs font-bold text-slate-200"> 导入 / 导出光影参数与预设 JSON </span>
        <button type="button" @click="emit('close')" class="text-slate-400 hover:text-white">
          ✕
        </button>
      </div>

      <!-- Body -->
      <div class="flex flex-1 flex-col gap-2 p-4">
        <div class="flex items-center justify-between text-[11px]">
          <span class="text-slate-400"> 支持直接粘贴光影配置、预设数组或全量预设集包： </span>
          <span class="font-medium text-purple-400">{{ statusMsg }}</span>
        </div>

        <textarea
          v-model="textContent"
          placeholder="在此粘贴光影 JSON，或点击下方选择本地文件..."
          class="h-full w-full resize-none rounded-md border border-[#232b3d] bg-[#090b10] p-3 font-mono text-[11px] text-slate-200 outline-none focus:border-purple-500"
        ></textarea>
      </div>

      <!-- Foot -->
      <div
        class="flex h-14 shrink-0 items-center justify-between border-t border-[#1f2636] bg-[#0c0e14] px-4"
      >
        <div class="flex items-center gap-2">
          <input
            ref="fileInputRef"
            type="file"
            accept=".json,application/json"
            class="hidden"
            @change="onFileSelected"
          />
          <button
            type="button"
            @click="handlePickFile"
            class="rounded border border-[#242b3d] bg-[#141822] px-2.5 py-1 text-xs text-slate-300 hover:bg-[#1c2230] hover:text-white"
          >
            📁 选择文件
          </button>
          <button
            type="button"
            @click="handleCopy"
            class="rounded border border-[#242b3d] bg-[#141822] px-2.5 py-1 text-xs text-slate-300 hover:bg-[#1c2230] hover:text-white"
          >
            📋 复制内容
          </button>
          <button
            type="button"
            @click="handleDownloadSingle"
            class="rounded border border-[#242b3d] bg-[#141822] px-2.5 py-1 text-xs text-slate-300 hover:bg-[#1c2230] hover:text-white"
          >
            💾 下载配置
          </button>
          <button
            type="button"
            @click="handleDownloadAll"
            class="rounded border border-purple-500/30 bg-purple-500/10 px-2.5 py-1 text-xs text-purple-300 hover:bg-purple-500/20"
          >
            📦 打包预设集
          </button>
        </div>

        <div class="flex items-center gap-2">
          <button
            type="button"
            @click="emit('close')"
            class="rounded px-3 py-1 text-xs text-slate-400 hover:text-slate-200"
          >
            取消
          </button>
          <button
            type="button"
            @click="handleApply"
            class="rounded bg-purple-600 px-4 py-1 text-xs font-semibold text-white shadow-sm hover:bg-purple-500"
          >
            确认应用
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
