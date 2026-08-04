<template>
  <div class="flex w-full flex-col gap-4">
    <div class="flex items-center justify-between">
      <p class="text-[0.78rem] text-white/50">
        配置剧本导师使用的模型、安全策略与文件沙箱范围。保存后下一次对话生效。
      </p>
      <button
        class="inline-flex items-center gap-1 rounded-lg border border-brand/45 bg-brand/14 px-4 py-1.5 text-[0.82rem] text-brand transition-colors hover:bg-brand/24"
        :disabled="store.loading"
        @click="store.saveSettings()"
      >
        保存设置
      </button>
    </div>

    <!-- LLM 模型 -->
    <MenuItem title="LLM 模型">
      <template #header>
        <Icon icon="setting" :size="16" class="text-brand" />
      </template>
      <div class="flex flex-col gap-3">
        <div>
          <label class="mb-1.5 inline-flex items-center font-medium text-brand text-[0.9rem]">使用的模型</label>
          <select
            v-model="providerId"
            class="glass-input w-full py-2"
            @change="applyProvider"
          >
            <option value="">跟随主对话模型（推荐）</option>
            <option v-for="p in providers" :key="p.id" :value="p.id">
              {{ p.label }}（{{ p.provider }} · {{ p.model }}）
            </option>
          </select>
          <p class="mt-1 text-[0.72rem] text-white/40">
            留空则使用「LLM 设置」中配置的主对话模型。可在<strong class="text-white/60">设置 → LLM 多供应商</strong>中添加模型。
          </p>
        </div>
      </div>
    </MenuItem>

    <!-- 安全与沙箱 -->
    <MenuItem title="命令与路径安全">
      <template #header>
        <Icon icon="hand" :size="16" class="text-brand" />
      </template>
      <div class="flex flex-col gap-4">
        <Toggle
          :checked="store.settings.autoApproveCommands"
          @change="(v: boolean) => (store.settings.autoApproveCommands = v)"
        >
          <span class="text-[0.86rem]">命令自动执行（无需确认）</span>
        </Toggle>
        <p class="-mt-2 text-[0.72rem] text-white/40">
          开启后 <code class="font-mono text-brand">execute_command</code> 不再弹审批，直接运行。建议保持关闭。
        </p>

        <Toggle
          :checked="store.settings.allowAnyPath"
          @change="(v: boolean) => (store.settings.allowAnyPath = v)"
        >
          <span class="text-[0.86rem]">允许任意路径</span>
        </Toggle>
        <p
          v-if="store.settings.allowAnyPath"
          class="-mt-2 rounded-lg border border-red-400/35 bg-red-400/12 px-3 py-2 text-[0.74rem] text-red-300"
        >
          ⚠ 开启后剧本导师可读写沙箱之外的任意文件，风险极高。仅在可信场景使用。
        </p>
        <p v-else class="-mt-2 text-[0.72rem] text-white/40">
          关闭时文件操作被限制在沙箱根目录内。开启后 AI 可访问沙箱之外的任意路径。
        </p>

        <div class="rounded-lg border border-white/10 bg-black/20 px-3 py-2.5">
          <div class="mb-1 text-[0.72rem] text-white/45">文件沙箱根目录</div>
          <div class="font-mono text-[0.78rem] text-white/85">
            {{ store.defaultDirs?.sandboxDir ?? store.settings.sandboxDir ?? '（默认 data/）' }}
          </div>
          <p class="mt-1 text-[0.68rem] text-white/35">
            助手可读写此目录内文件（含剧本、角色、素材）。自定义沙箱目录可通过配置键
            <code class="font-mono text-brand">agent.sandbox_dir</code> 修改，留空默认 <code class="font-mono">data/</code>。
          </p>
        </div>
      </div>
    </MenuItem>

    <!-- 运行与提示 -->
    <MenuItem title="运行与提示">
      <template #header>
        <Icon icon="text" :size="16" class="text-brand" />
      </template>
      <div class="flex flex-col gap-4">
        <div class="flex items-center gap-3">
          <label class="shrink-0 text-[0.86rem] text-white/75">最大工具调用轮数</label>
          <input
            v-model.number="store.settings.maxToolRounds"
            type="number"
            min="-1"
            class="glass-input w-28 py-1.5 text-center"
          />
          <span class="text-[0.72rem] text-white/40">-1 表示无上限（默认）</span>
        </div>

        <div>
          <label class="mb-1.5 inline-flex items-center font-medium text-brand text-[0.9rem]">
            思考模式
          </label>
          <select v-model="enableThinkingValue" class="glass-input w-full py-2">
            <option :value="null">跟随模型默认</option>
            <option :value="true">开启</option>
            <option :value="false">关闭</option>
          </select>
          <p class="mt-1 text-[0.72rem] text-white/40">
            独立于主对话的 LLM 设置；开启后模型思考链会以折叠「思考/规划…」块显示。
          </p>
        </div>

        <div>
          <label class="mb-1.5 inline-flex items-center font-medium text-brand text-[0.9rem]">
            自定义系统提示（可选）
          </label>
          <textarea
            v-model="systemPromptText"
            rows="4"
            class="glass-input w-full resize-y leading-relaxed"
            placeholder="留空使用内置默认提示。技能列表与当前剧本信息始终会自动追加。"
          ></textarea>
          <p class="mt-1 text-[0.72rem] text-white/40">
            在默认提示的基础上可补充你的偏好（如语气、产出格式）。技能列表与剧本上下文不受影响。
          </p>
        </div>
      </div>
    </MenuItem>

    <!-- 技能库 -->
    <MenuItem title="技能库">
      <template #header>
        <Icon icon="package" :size="16" class="text-brand" />
      </template>
      <div class="flex flex-col gap-2">
        <p class="text-[0.72rem] text-white/40">
          助手通过读取 SKILL.md 技能指令来编写剧本。技能目录：{{ store.defaultDirs?.skillsDir ?? '…' }}
        </p>
        <button
          v-for="s in store.skills"
          :key="s.name"
          class="flex items-center gap-2 rounded-[10px] border border-white/10 bg-white/6 px-3 py-2.5 text-left transition-all duration-200 hover:border-brand/40"
          @click="toggleSkill(s.name)"
        >
          <span class="text-[1rem]">{{ s.location === 'global' ? '🌐' : '📦' }}</span>
          <span class="flex min-w-0 flex-1 flex-col">
            <span class="font-mono text-[0.84rem] text-white/90">{{ s.name }}</span>
            <span class="truncate text-[0.72rem] text-white/45">{{ s.description || '（无描述）' }}</span>
          </span>
          <span class="text-[0.7rem] text-white/35">{{ s.location }}</span>
        </button>

        <div
          v-if="preview"
          class="mt-1 overflow-hidden rounded-lg border border-white/10 bg-black/25"
        >
          <div class="flex items-center justify-between border-b border-white/10 px-3 py-2">
            <span class="font-mono text-[0.78rem] text-brand">{{ preview.name }} · SKILL.md</span>
            <span class="cursor-pointer text-white/45 hover:text-white/80" @click="preview = null">✕</span>
          </div>
          <pre class="max-h-72 overflow-y-auto whitespace-pre-wrap px-3 py-2.5 font-mono text-[0.72rem] leading-relaxed text-white/75">{{ preview.content }}</pre>
        </div>
      </div>
    </MenuItem>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Icon, Toggle } from '@/components/base'
import { MenuItem } from '@/components/ui'
import { listLlmProviders } from '@/api/services/llm-providers'
import { getAgentDefaultDirs, readAgentSkill } from '@/api/services/agent'
import type { LlmProviderConfig } from '@/api/services/llm-providers'
import { useAgentStore } from '@/stores/modules/agent'

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
    console.error('读取技能失败:', err)
    preview.value = { name, content: `读取失败: ${err}` }
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
    console.error('加载 LLM provider 列表失败:', err)
  }
  providerId.value = store.settings.providerId ?? ''
})
</script>
