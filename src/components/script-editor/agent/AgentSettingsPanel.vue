<template>
  <div class="flex
    w-full
    flex-col
    gap-4">
    <div class="flex
      items-center
      justify-between">
      <p class="text-[0.78rem]
        text-white/50">
        {{ t('scriptEditor.agentSettings.intro') }}
      </p>
      <button
        class="inline-flex
          items-center
          gap-1
          rounded-lg
          border
          border-brand/45
          bg-brand/14
          px-4
          py-1.5
          text-[0.82rem]
          text-brand
          transition-colors
          hover:bg-brand/24"
        :disabled="store.loading"
        @click="store.saveSettings()"
      >
        {{ t('scriptEditor.agentSettings.save') }}
      </button>
    </div>

    <!-- LLM 模型 -->
    <MenuItem :title="t('scriptEditor.agentSettings.llmModel')">
      <template #header>
        <Icon
          icon="setting"
          :size="16"
          class="text-brand"
        />
      </template>
      <div class="flex
        flex-col
        gap-3">
        <div>
          <label class="mb-1.5
            inline-flex
            items-center
            font-medium
            text-brand
            text-[0.9rem]">{{
            t('scriptEditor.agentSettings.model')
          }}</label>
          <select
            v-model="providerId"
            class="glass-input
              w-full
              py-2"
            @change="applyProvider"
          >
            <option value="">{{ t('scriptEditor.agentSettings.followMain') }}</option>
            <option
              v-for="p in providers"
              :key="p.id"
              :value="p.id"
            >
              {{ p.label }}（{{ p.provider }} · {{ p.model }}）
            </option>
          </select>
          <p
            class="mt-1
              text-[0.72rem]
              text-white/40"
            v-html="t('scriptEditor.agentSettings.modelHint')"
          ></p>
        </div>
      </div>
    </MenuItem>

    <!-- 安全与沙箱 -->
    <MenuItem :title="t('scriptEditor.agentSettings.security')">
      <template #header>
        <Icon
          icon="hand"
          :size="16"
          class="text-brand"
        />
      </template>
      <div class="flex
        flex-col
        gap-4">
        <Toggle
          :checked="store.settings.autoApproveCommands"
          @change="(v: boolean) => (store.settings.autoApproveCommands = v)"
        >
          <span class="text-[0.86rem]">{{ t('scriptEditor.agentSettings.autoApprove') }}</span>
        </Toggle>
        <p
          class="-mt-2
            text-[0.72rem]
            text-white/40"
          v-html="t('scriptEditor.agentSettings.autoApproveHint')"
        ></p>

        <Toggle
          :checked="store.settings.allowAnyPath"
          @change="(v: boolean) => (store.settings.allowAnyPath = v)"
        >
          <span class="text-[0.86rem]">{{ t('scriptEditor.agentSettings.allowAnyPath') }}</span>
        </Toggle>
        <p
          v-if="store.settings.allowAnyPath"
          class="-mt-2
            rounded-lg
            border
            border-red-400/35
            bg-red-400/12
            px-3
            py-2
            text-[0.74rem]
            text-red-300"
          v-html="t('scriptEditor.agentSettings.allowAnyPathWarn')"
        ></p>
        <p
          v-else
          class="-mt-2
            text-[0.72rem]
            text-white/40"
          v-html="t('scriptEditor.agentSettings.allowAnyPathHint')"
        ></p>

        <div class="rounded-lg
          border
          border-white/10
          bg-black/20
          px-3
          py-2.5">
          <div class="mb-1
            text-[0.72rem]
            text-white/45">
            {{ t('scriptEditor.agentSettings.sandbox') }}
          </div>
          <div class="font-mono
            text-[0.78rem]
            text-white/85">
            {{
              store.defaultDirs?.sandboxDir ??
              store.settings.sandboxDir ??
              t('scriptEditor.agentSettings.sandboxDefault')
            }}
          </div>
          <p
            class="mt-1
              text-[0.68rem]
              text-white/35"
            v-html="t('scriptEditor.agentSettings.sandboxHint')"
          ></p>
        </div>
      </div>
    </MenuItem>

    <!-- 运行与提示 -->
    <MenuItem :title="t('scriptEditor.agentSettings.runtime')">
      <template #header>
        <Icon
          icon="text"
          :size="16"
          class="text-brand"
        />
      </template>
      <div class="flex
        flex-col
        gap-4">
        <div class="flex
          items-center
          gap-3">
          <label class="shrink-0
            text-[0.86rem]
            text-white/75">{{
            t('scriptEditor.agentSettings.maxToolRounds')
          }}</label>
          <input
            v-model.number="store.settings.maxToolRounds"
            type="number"
            min="-1"
            class="glass-input
              w-28
              py-1.5
              text-center"
          />
          <span class="text-[0.72rem]
            text-white/40">{{
            t('scriptEditor.agentSettings.unlimited')
          }}</span>
        </div>

        <div>
          <label class="mb-1.5
            inline-flex
            items-center
            font-medium
            text-brand
            text-[0.9rem]">
            {{ t('scriptEditor.agentSettings.thinkingMode') }}
          </label>
          <select
            v-model="enableThinkingValue"
            class="glass-input
              w-full
              py-2"
          >
            <option :value="null">{{ t('scriptEditor.agentSettings.thinkingFollow') }}</option>
            <option :value="true">{{ t('scriptEditor.agentSettings.thinkingOn') }}</option>
            <option :value="false">{{ t('scriptEditor.agentSettings.thinkingOff') }}</option>
          </select>
          <p
            class="mt-1
              text-[0.72rem]
              text-white/40"
            v-html="t('scriptEditor.agentSettings.thinkingHint')"
          ></p>
        </div>

        <div>
          <label class="mb-1.5
            inline-flex
            items-center
            font-medium
            text-brand
            text-[0.9rem]">
            {{ t('scriptEditor.agentSettings.systemPrompt') }}
          </label>
          <textarea
            v-model="systemPromptText"
            rows="4"
            class="glass-input
              w-full
              resize-y
              leading-relaxed"
            :placeholder="t('scriptEditor.agentSettings.systemPromptPlaceholder')"
          ></textarea>
          <p
            class="mt-1
              text-[0.72rem]
              text-white/40"
            v-html="t('scriptEditor.agentSettings.systemPromptHint')"
          ></p>
        </div>
      </div>
    </MenuItem>

    <!-- 技能库 -->
    <MenuItem :title="t('scriptEditor.agentSettings.skills')">
      <template #header>
        <Icon
          icon="package"
          :size="16"
          class="text-brand"
        />
      </template>
      <div class="flex
        flex-col
        gap-2">
        <p class="text-[0.72rem]
          text-white/40">
          {{
            t('scriptEditor.agentSettings.skillsHint', { dir: store.defaultDirs?.skillsDir ?? '…' })
          }}
        </p>
        <button
          v-for="s in store.skills"
          :key="s.name"
          class="flex
            items-center
            gap-2
            rounded-[10px]
            border
            border-white/10
            bg-white/6
            px-3
            py-2.5
            text-left
            transition-all
            duration-200
            hover:border-brand/40"
          @click="toggleSkill(s.name)"
        >
          <span class="text-[1rem]">{{ s.location === 'global' ? '🌐' : '📦' }}</span>
          <span class="flex
            min-w-0
            flex-1
            flex-col">
            <span class="font-mono
              text-[0.84rem]
              text-white/90">{{ s.name }}</span>
            <span class="truncate
              text-[0.72rem]
              text-white/45">{{
              s.description || t('scriptEditor.agentSettings.noDescription')
            }}</span>
          </span>
          <span class="text-[0.7rem]
            text-white/35">{{ s.location }}</span>
        </button>

        <div
          v-if="preview"
          class="mt-1
            overflow-hidden
            rounded-lg
            border
            border-white/10
            bg-black/25"
        >
          <div class="flex
            items-center
            justify-between
            border-b
            border-white/10
            px-3
            py-2">
            <span class="font-mono
              text-[0.78rem]
              text-brand">{{ preview.name }} · SKILL.md</span>
            <span
              class="cursor-pointer
                text-white/45
                hover:text-white/80"
              @click="preview = null"
              >✕</span
            >
          </div>
          <pre
            class="max-h-72
              overflow-y-auto
              whitespace-pre-wrap
              px-3
              py-2.5
              font-mono
              text-[0.72rem]
              leading-relaxed
              text-white/75"
            >{{ preview.content }}</pre
          >
        </div>
      </div>
    </MenuItem>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Icon, Toggle } from '@/components/base'
import { MenuItem } from '@/components/ui'
import { listLlmProviders } from '@/api/services/llm-providers'
import { getAgentDefaultDirs, readAgentSkill } from '@/api/services/agent'
import type { LlmProviderConfig } from '@/api/services/llm-providers'
import { useAgentStore } from '@/stores/modules/agent'

const { t } = useI18n()
const store = useAgentStore()

const providers = ref<LlmProviderConfig[]>([])
const preview = ref<{ name: string; content: string } | null>(null)

const providerId = ref<string>('')

const systemPromptText = computed({
  get: () => store.settings.systemPrompt ?? '',
  set: (v: string) => {
    store.settings.systemPrompt = v.trim() ? v : null
  },
})

/** 思考模式三态：null=跟随模型默认 / true=开启 / false=关闭。 */
const enableThinkingValue = computed({
  get: () => store.settings.enableThinking,
  set: (v: boolean | null) => {
    store.settings.enableThinking = v
  },
})

function applyProvider() {
  store.settings.providerId = providerId.value || null
}

async function toggleSkill(name: string) {
  if (preview.value?.name === name) {
    preview.value = null
    return
  }
  try {
    const res = await readAgentSkill(name)
    preview.value = { name: res.name, content: res.content }
  } catch (err) {
    console.error('readSkillFailed:', err)
    preview.value = { name, content: t('scriptEditor.agentSettings.readSkillFailed', { err }) }
  }
}

onMounted(async () => {
  await store.loadSettings()
  await store.loadSkills()
  if (!store.defaultDirs) {
    store.defaultDirs = await getAgentDefaultDirs()
  }
  try {
    const res = await listLlmProviders()
    providers.value = res.providers
  } catch (err) {
    console.error(t('scriptEditor.agentSettings.loadProvidersFailed', { err }))
  }
  providerId.value = store.settings.providerId ?? ''
})
</script>
