<template>
  <div>
    <p class="mb-4 px-1 text-sm text-gray-300">{{ $t("ui.toolCalls.mediaHint") }}</p>

    <div class="border-brand/30 bg-brand/10 my-3 rounded-xl border px-4 py-3">
      <p class="text-sm text-gray-200">{{ $t("ui.toolCalls.mediaVisionModelHint") }}</p>
    </div>

    <div class="grid gap-2 sm:grid-cols-2">
      <ToolToggleRow
        variant="card"
        :checked="settings.media_file.image_enabled"
        :label="$t('ui.toolCalls.mediaImages')"
        @change="(value) => (settings.media_file.image_enabled = value)"
      />
      <ToolToggleRow
        variant="card"
        :checked="settings.media_file.video_enabled"
        :label="$t('ui.toolCalls.mediaVideos')"
        @change="(value) => (settings.media_file.video_enabled = value)"
      />
    </div>

    <div class="mt-4 grid gap-4 sm:grid-cols-2">
      <ToolField :label="$t('ui.toolCalls.mediaMaxFileMb')">
        <ToolInput
          v-model="settings.media_file.max_file_mb"
          type="number"
          min="1"
          max="100"
          step="1"
        />
      </ToolField>
      <ToolField :label="$t('ui.toolCalls.mediaOutputTokens')">
        <ToolInput
          v-model="settings.media_file.max_output_tokens"
          type="number"
          min="128"
          max="4096"
          step="128"
        />
      </ToolField>
      <ToolField :label="$t('ui.toolCalls.mediaImageMaxEdge')">
        <ToolInput
          v-model="settings.media_file.image_max_edge"
          type="number"
          min="512"
          max="4096"
          step="128"
          :disabled="!settings.media_file.image_enabled"
        />
      </ToolField>
      <ToolField :label="$t('ui.toolCalls.mediaJpegQuality')">
        <ToolInput
          v-model="settings.media_file.jpeg_quality"
          type="number"
          min="50"
          max="95"
          step="1"
          :disabled="!settings.media_file.image_enabled"
        />
      </ToolField>
    </div>

    <ToolField :label="$t('ui.toolCalls.mediaDefaultPrompt')" class="mt-4">
      <ToolTextarea v-model="settings.media_file.default_prompt" :rows="3" />
    </ToolField>

    <p v-if="settings.media_file.video_enabled" class="mt-3 px-1 text-sm text-amber-300/90">
      {{ $t("ui.toolCalls.mediaVideoCompatibility") }}
    </p>
  </div>
</template>

<script setup lang="ts">
import type { ToolSettings } from "@/api/services/tool-settings";
import ToolField from "@/components/settings/tools/fields/ToolField.vue";
import ToolInput from "@/components/settings/tools/fields/ToolInput.vue";
import ToolTextarea from "@/components/settings/tools/fields/ToolTextarea.vue";
import ToolToggleRow from "@/components/settings/tools/ToolToggleRow.vue";

const settings = defineModel<ToolSettings>({ required: true });
</script>
