<template>
  <section
    class="overflow-hidden rounded-xl border transition-colors duration-200"
    :class="
      dangerous
        ? 'border-amber-400/40 bg-amber-400/5'
        : expanded
          ? 'border-brand/40 bg-white/8'
          : 'border-white/15 bg-white/5'
    "
  >
    <div
      class="flex cursor-pointer flex-wrap items-center gap-x-3 gap-y-1.5 px-4 py-3 select-none"
      role="button"
      tabindex="0"
      :aria-expanded="expanded"
      :aria-label="$t('ui.toolCalls.' + (expanded ? 'collapse' : 'expand')) + ' ' + title"
      @click="$emit('toggle-expand')"
      @keydown.enter.prevent="$emit('toggle-expand')"
      @keydown.space.prevent="$emit('toggle-expand')"
    >
      <div class="flex min-w-0 flex-1 items-center gap-1.5">
        <span class="truncate text-sm font-semibold text-white sm:text-base">{{ title }}</span>
        <ToolHelpPopover :text="help" @click.stop />
      </div>

      <span class="order-3 w-full text-right text-xs text-gray-400 sm:order-none sm:w-auto">
        {{ $t("ui.toolCalls.enabledCount", { enabled: enabledCount, total: tools.length }) }}
      </span>

      <div class="ml-auto flex shrink-0 items-center gap-2.5">
        <button
          v-if="hasSettings"
          type="button"
          class="flex h-7 w-7 items-center justify-center rounded-lg border border-white/20 bg-black/20 text-sm text-white/70 transition-colors duration-150 hover:border-white/40 hover:text-white"
          :aria-label="$t('ui.toolCalls.settingsFor', { name: title })"
          @click.stop="$emit('open-settings')"
        >
          ⚙
        </button>

        <input
          type="checkbox"
          class="accent-brand h-4 w-4 shrink-0 disabled:cursor-not-allowed disabled:opacity-40"
          :checked="allChecked"
          :indeterminate="partiallyChecked"
          :disabled="!supported || tools.length === 0"
          :aria-label="$t('ui.toolCalls.togglePackage', { name: title })"
          @click.stop
          @change="onPackageChange"
        />

        <span
          class="pack-arrow text-xs text-gray-400 transition-transform duration-200"
          :class="{ 'rotate-180': expanded }"
          aria-hidden="true"
        >
          ▾
        </span>
      </div>
    </div>

    <div
      class="pack-body grid transition-[grid-template-rows] duration-200 ease-out"
      :class="expanded ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]'"
    >
      <div class="min-h-0 overflow-hidden">
        <div class="border-t border-white/10 px-4 py-3">
          <p
            v-if="dangerous"
            class="mb-2 rounded-md border-l-4 border-amber-400/70 bg-amber-400/10 px-3 py-2 text-xs leading-relaxed text-amber-200/90"
          >
            {{ $t("ui.toolCalls.dangerPackWarning") }}
          </p>
          <p v-if="group === 'file_ops'" class="mb-2 px-1 text-xs text-gray-400">
            {{ $t(`ui.toolCalls.fileAccessByMode.${accessMode}`) }}
          </p>
          <p v-if="group === 'command'" class="mb-2 px-1 text-xs text-gray-400">
            {{ $t(`ui.toolCalls.commandAccessByMode.${accessMode}`) }}
          </p>
          <p
            v-if="group === 'command' && !supported"
            class="mb-2 rounded-md bg-amber-400/10 px-3 py-2 text-xs text-amber-300"
          >
            {{ $t("ui.toolCalls.commandWindowsOnly") }}
          </p>
          <p v-if="group === 'media'" class="mb-2 px-1 text-xs text-gray-400">
            {{ $t("ui.toolCalls.mediaTwoLayerHint") }}
          </p>

          <div class="grid gap-0.5 sm:grid-cols-2">
            <label
              v-for="tool in tools"
              :key="tool.name"
              class="flex items-start gap-2.5 rounded-lg px-2 py-1.5 transition-colors duration-150"
              :class="
                supported ? 'cursor-pointer hover:bg-white/5' : 'cursor-not-allowed opacity-50'
              "
            >
              <input
                type="checkbox"
                class="accent-brand mt-0.5 h-4 w-4 shrink-0"
                :checked="tool.checked"
                :disabled="!supported"
                @change="onToolChange(tool.name, $event)"
              />
              <span class="min-w-0">
                <span class="block text-sm text-gray-200">
                  <template v-for="(part, index) in highlight(toolLabel(tool.name))" :key="index">
                    <span :class="part.hit ? 'bg-brand/30 rounded text-white' : ''">{{
                      part.text
                    }}</span>
                  </template>
                </span>
                <span v-if="toolHint(tool.name)" class="mt-0.5 block text-xs text-gray-400">
                  {{ toolHint(tool.name) }}
                </span>
              </span>
            </label>
          </div>

          <div class="mt-2 flex justify-end border-t border-white/5 pt-2">
            <button
              type="button"
              class="rounded-md px-2 py-1 text-xs text-gray-400 transition-colors duration-150 hover:bg-white/5 hover:text-white"
              @click.stop="$emit('reset')"
            >
              {{ $t("ui.toolCalls.resetPackage") }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed } from "vue";
import {
  toolDescription,
  toolDisplayName,
  type ToolAccessMode,
} from "@/api/services/tool-settings";
import ToolHelpPopover from "./ToolHelpPopover.vue";

interface ToolRow {
  name: string;
  checked: boolean;
}

const props = defineProps<{
  group: string;
  title: string;
  help: string;
  tools: ToolRow[];
  supported: boolean;
  expanded: boolean;
  dangerous: boolean;
  hasSettings: boolean;
  accessMode: ToolAccessMode;
  query: string;
}>();

const emit = defineEmits<{
  "toggle-expand": [];
  "toggle-package": [enabled: boolean];
  "toggle-tool": [tool: string, enabled: boolean];
  "open-settings": [];
  reset: [];
}>();

const enabledCount = computed(() => props.tools.filter((tool) => tool.checked).length);
const allChecked = computed(
  () => props.tools.length > 0 && enabledCount.value === props.tools.length,
);
const partiallyChecked = computed(() => enabledCount.value > 0 && !allChecked.value);

function onPackageChange(event: Event) {
  emit("toggle-package", (event.target as HTMLInputElement).checked);
}

function onToolChange(tool: string, event: Event) {
  emit("toggle-tool", tool, (event.target as HTMLInputElement).checked);
}

function toolLabel(tool: string) {
  return toolDisplayName(tool);
}

function toolHint(tool: string) {
  return toolDescription(tool);
}

interface TextPart {
  text: string;
  hit: boolean;
}

/** 把命中的搜索片段切成可高亮的小段，避免用 v-html。 */
function highlight(text: string): TextPart[] {
  const query = props.query.trim().toLowerCase();
  if (!query) return [{ text, hit: false }];
  const lower = text.toLowerCase();
  const parts: TextPart[] = [];
  let cursor = 0;
  let index = lower.indexOf(query);
  while (index !== -1) {
    if (index > cursor) parts.push({ text: text.slice(cursor, index), hit: false });
    parts.push({ text: text.slice(index, index + query.length), hit: true });
    cursor = index + query.length;
    index = lower.indexOf(query, cursor);
  }
  if (cursor < text.length) parts.push({ text: text.slice(cursor), hit: false });
  return parts;
}
</script>

<style scoped>
@media (prefers-reduced-motion: reduce) {
  .pack-body,
  .pack-arrow {
    transition-duration: 0.01ms !important;
  }
}
</style>
