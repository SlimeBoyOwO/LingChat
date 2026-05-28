<template>
  <div class="flex-1 flex flex-col h-full min-h-0">
    <!-- ========== VIEW: List ========== -->
    <template v-if="view === 'list'">
      <div class="flex items-center justify-between mb-4">
        <h3 class="text-white text-base font-semibold">已配置的模型</h3>
        <button
          class="px-4 py-2 bg-brand text-white rounded-lg text-sm font-medium hover:bg-brand/80 transition-colors"
          @click="startAdd"
        >
          + 添加模型
        </button>
      </div>

      <!-- Provider table -->
      <div v-if="store.providers.length === 0" class="text-white/50 text-sm py-8 text-center">
        暂无配置的模型，点击"添加模型"开始配置
      </div>
      <div v-else class="flex flex-col gap-2 overflow-y-auto flex-1">
        <div
          v-for="p in store.providers"
          :key="p.id"
          class="flex items-center gap-3 px-4 py-3 rounded-lg bg-white/5 border border-white/10 hover:border-white/20 transition-colors"
        >
          <!-- Info -->
          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-2 mb-0.5">
              <span class="text-sm font-semibold text-white truncate">{{ p.label || '(未命名)' }}</span>
              <span class="text-[10px] px-1.5 py-0.5 rounded bg-brand/20 text-brand/90">{{ p.provider }}</span>
            </div>
            <div class="text-xs text-white/40 truncate">{{ p.model || '未设置模型' }}</div>
          </div>

          <!-- Role badges -->
          <div class="flex gap-1.5 shrink-0">
            <span
              v-if="store.chatProviderId === p.id"
              class="text-[10px] px-2 py-0.5 rounded-full bg-green-500/20 text-green-300 border border-green-500/30"
            >对话</span>
            <span
              v-if="store.translateProviderId === p.id"
              class="text-[10px] px-2 py-0.5 rounded-full bg-blue-500/20 text-blue-300 border border-blue-500/30"
            >翻译</span>
          </div>

          <!-- Actions -->
          <div class="flex gap-1 shrink-0">
            <button
              class="px-3 py-1.5 text-xs rounded-lg bg-white/10 text-white/70 hover:bg-white/20 hover:text-white transition-colors"
              @click="startEdit(p)"
            >编辑</button>
            <button
              class="px-3 py-1.5 text-xs rounded-lg bg-white/10 text-white/70 hover:bg-blue-500/20 hover:text-blue-300 transition-colors"
              @click="startTest(p)"
            >测试</button>
            <button
              class="px-3 py-1.5 text-xs rounded-lg bg-white/10 text-white/70 hover:bg-red-500/20 hover:text-red-300 transition-colors"
              @click="confirmDelete(p)"
            >删除</button>
          </div>
        </div>
      </div>

      <!-- Role assignment (at bottom of list) -->
      <div class="mt-4 pt-4 border-t border-white/10">
        <div class="grid grid-cols-2 gap-4">
          <div class="flex flex-col gap-1.5">
            <label class="text-xs font-medium text-white/60">对话模型</label>
            <div class="relative">
              <select
                :value="store.chatProviderId"
                @change="onChatRoleChange(($event.target as HTMLSelectElement).value)"
                class="w-full appearance-none pl-3 pr-8 py-2 rounded-lg bg-white/10 border border-white/20 text-white text-sm outline-none focus:border-brand transition-colors cursor-pointer"
              >
                <option :value="null" class="bg-gray-800 text-white">未选择</option>
                <option
                  v-for="p in store.providers"
                  :key="p.id"
                  :value="p.id"
                  class="bg-gray-800 text-white"
                >{{ p.label || p.model || '(未命名)' }}</option>
              </select>
              <div class="pointer-events-none absolute inset-y-0 right-0 flex items-center pr-2.5">
                <svg class="w-4 h-4 text-white/40" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/></svg>
              </div>
            </div>
          </div>
          <div class="flex flex-col gap-1.5">
            <label class="text-xs font-medium text-white/60">翻译模型</label>
            <div class="relative">
              <select
                :value="store.translateProviderId ?? '__follow__'"
                @change="onTranslateRoleChange(($event.target as HTMLSelectElement).value)"
                class="w-full appearance-none pl-3 pr-8 py-2 rounded-lg bg-white/10 border border-white/20 text-white text-sm outline-none focus:border-brand transition-colors cursor-pointer"
              >
                <option value="__follow__" class="bg-gray-800 text-white">跟随对话模型</option>
                <option
                  v-for="p in store.providers"
                  :key="p.id"
                  :value="p.id"
                  class="bg-gray-800 text-white"
                >{{ p.label || p.model || '(未命名)' }}</option>
              </select>
              <div class="pointer-events-none absolute inset-y-0 right-0 flex items-center pr-2.5">
                <svg class="w-4 h-4 text-white/40" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/></svg>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Save message -->
      <p v-if="saveMessage" class="mt-3 text-xs" :class="saveError ? 'text-red-400' : 'text-green-400'">
        {{ saveMessage }}
      </p>
    </template>

    <!-- ========== VIEW: Edit ========== -->
    <template v-if="view === 'edit'">
      <div class="flex items-center gap-3 mb-4">
        <button
          class="text-white/60 hover:text-white transition-colors text-sm flex items-center gap-1"
          @click="backToList"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/></svg>
          返回
        </button>
        <h3 class="text-white text-base font-semibold">{{ isEditing ? '编辑模型' : '添加模型' }}</h3>
      </div>

      <form @submit.prevent="saveCurrent" class="flex flex-col gap-4 overflow-y-auto flex-1 pr-1">
        <!-- Label -->
        <div class="flex flex-col gap-1">
          <label class="text-xs font-medium text-white/60">名称</label>
          <input
            v-model="editing.label"
            type="text"
            placeholder="例如: DeepSeek V3"
            class="px-3 py-2 rounded-lg bg-white/10 border border-white/20 text-white text-sm outline-none focus:border-brand transition-colors placeholder:text-white/20"
          />
        </div>

        <!-- Provider type -->
        <div class="flex flex-col gap-1">
          <label class="text-xs font-medium text-white/60">提供商类型</label>
          <div class="relative">
            <select
              v-model="editing.provider"
              class="w-full appearance-none pl-3 pr-8 py-2 rounded-lg bg-white/10 border border-white/20 text-white text-sm outline-none focus:border-brand transition-colors cursor-pointer"
            >
              <option value="openai" class="bg-gray-800 text-white">OpenAI 兼容 (DeepSeek / 通义千问 / Ollama)</option>
              <option value="gemini" class="bg-gray-800 text-white">Gemini</option>
            </select>
            <div class="pointer-events-none absolute inset-y-0 right-0 flex items-center pr-2.5">
              <svg class="w-4 h-4 text-white/40" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/></svg>
            </div>
          </div>
        </div>

        <!-- Model -->
        <div class="flex flex-col gap-1">
          <label class="text-xs font-medium text-white/60">模型名称</label>
          <input
            v-model="editing.model"
            type="text"
            placeholder="例如: deepseek-chat"
            class="px-3 py-2 rounded-lg bg-white/10 border border-white/20 text-white text-sm outline-none focus:border-brand transition-colors placeholder:text-white/20"
          />
        </div>

        <!-- API Key -->
        <div class="flex flex-col gap-1">
          <label class="text-xs font-medium text-white/60">API 密钥</label>
          <input
            v-model="editing.api_key"
            type="password"
            placeholder="sk-..."
            class="px-3 py-2 rounded-lg bg-white/10 border border-white/20 text-white text-sm outline-none focus:border-brand transition-colors placeholder:text-white/20"
          />
        </div>

        <!-- Base URL -->
        <div class="flex flex-col gap-1">
          <label class="text-xs font-medium text-white/60">API 地址（留空使用默认）</label>
          <input
            v-model="editing.base_url"
            type="text"
            placeholder="留空使用默认地址"
            class="px-3 py-2 rounded-lg bg-white/10 border border-white/20 text-white text-sm outline-none focus:border-brand transition-colors placeholder:text-white/20"
          />
        </div>

        <!-- Temperature -->
        <div class="flex flex-col gap-1">
          <label class="text-xs font-medium text-white/60">Temperature（留空使用默认）</label>
          <input
            v-model.number="editing.temperature"
            type="number"
            step="0.1"
            min="0"
            max="2"
            class="px-3 py-2 rounded-lg bg-white/10 border border-white/20 text-white text-sm outline-none focus:border-brand transition-colors"
          />
        </div>

        <!-- Top P -->
        <div class="flex flex-col gap-1">
          <label class="text-xs font-medium text-white/60">Top P（留空使用默认）</label>
          <input
            v-model.number="editing.top_p"
            type="number"
            step="0.05"
            min="0"
            max="1"
            class="px-3 py-2 rounded-lg bg-white/10 border border-white/20 text-white text-sm outline-none focus:border-brand transition-colors"
          />
        </div>

        <!-- Enable thinking -->
        <label class="flex items-center gap-3 cursor-pointer">
          <span class="text-xs font-medium text-white/60">启用思考链（部分模型支持）</span>
          <div class="relative">
            <input
              v-model="editing.enable_thinking"
              type="checkbox"
              class="sr-only peer"
            />
            <div class="w-9 h-5 bg-white/10 rounded-full peer-checked:bg-brand transition-colors border border-white/20 peer-checked:border-brand"></div>
            <div class="absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full peer-checked:translate-x-4 transition-transform"></div>
          </div>
        </label>

        <!-- Action buttons -->
        <div class="flex gap-3 pt-2">
          <button
            type="submit"
            class="px-5 py-2 bg-brand text-white rounded-lg text-sm font-medium hover:bg-brand/80 transition-colors"
          >保存</button>
          <button
            type="button"
            class="px-5 py-2 bg-white/10 text-white/70 rounded-lg text-sm hover:bg-white/20 transition-colors"
            @click="backToList"
          >取消</button>
        </div>

        <p v-if="saveMessage" class="text-xs" :class="saveError ? 'text-red-400' : 'text-green-400'">
          {{ saveMessage }}
        </p>
      </form>
    </template>

    <!-- ========== VIEW: Test ========== -->
    <template v-if="view === 'test'">
      <div class="flex items-center gap-3 mb-4">
        <button
          class="text-white/60 hover:text-white transition-colors text-sm flex items-center gap-1"
          @click="backToList"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/></svg>
          返回
        </button>
        <h3 class="text-white text-base font-semibold">
          测试 {{ testProvider?.label || testProvider?.model || '' }}
        </h3>
      </div>

      <div class="flex flex-col gap-4 flex-1 min-h-0">
        <!-- Test input -->
        <div class="flex gap-2">
          <input
            v-model="testMessage"
            type="text"
            placeholder="输入测试消息..."
            class="flex-1 px-3 py-2 rounded-lg bg-white/10 border border-white/20 text-white text-sm outline-none focus:border-brand transition-colors placeholder:text-white/20"
            @keydown.enter="doTest"
          />
          <button
            class="px-4 py-2 bg-brand text-white rounded-lg text-sm font-medium hover:bg-brand/80 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            :disabled="testing || !testMessage.trim()"
            @click="doTest"
          >
            {{ testing ? '测试中...' : '发送' }}
          </button>
        </div>

        <!-- Response area -->
        <div class="flex-1 min-h-0 rounded-lg bg-white/5 border border-white/10 p-4 overflow-y-auto">
          <div v-if="testing" class="flex items-center gap-2 text-white/40 text-sm">
            <div class="w-4 h-4 border-2 border-white/20 border-t-brand rounded-full animate-spin"></div>
            等待响应...
          </div>
          <div v-else-if="testError" class="text-red-400 text-sm whitespace-pre-wrap">{{ testError }}</div>
          <div v-else-if="testResponse" class="text-white/80 text-sm whitespace-pre-wrap leading-relaxed">{{ testResponse }}</div>
          <div v-else class="text-white/30 text-sm">输入消息并点击发送，测试模型响应</div>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive } from 'vue'
import { useLlmProvidersStore } from '@/stores/modules/llm-providers'
import { invoke } from '@tauri-apps/api/core'
import type { LlmProviderConfig } from '@/api/services/llm-providers'

const store = useLlmProvidersStore()

const view = ref<'list' | 'edit' | 'test'>('list')
const isEditing = ref(false)
const editing = reactive<LlmProviderConfig>(emptyProvider())
const saveMessage = ref('')
const saveError = ref(false)

// Test state
const testProvider = ref<LlmProviderConfig | null>(null)
const testMessage = ref('')
const testResponse = ref('')
const testError = ref('')
const testing = ref(false)

function emptyProvider(): LlmProviderConfig {
  return {
    id: '',
    label: '',
    provider: 'openai',
    model: '',
    api_key: '',
    base_url: '',
    temperature: null,
    top_p: null,
    enable_thinking: false,
  }
}

function backToList() {
  view.value = 'list'
  saveMessage.value = ''
}

function startAdd() {
  isEditing.value = false
  Object.assign(editing, emptyProvider())
  view.value = 'edit'
  saveMessage.value = ''
}

function startEdit(p: LlmProviderConfig) {
  isEditing.value = true
  Object.assign(editing, { ...p })
  view.value = 'edit'
  saveMessage.value = ''
}

function confirmDelete(p: LlmProviderConfig) {
  if (!confirm(`确定删除模型 "${p.label || p.model || '(未命名)'}"？`)) return
  deleteProvider(p.id)
}

async function deleteProvider(id: string) {
  try {
    await store.deleteProvider(id)
    saveMessage.value = '已删除'
    saveError.value = false
  } catch (e: any) {
    saveMessage.value = `删除失败: ${e}`
    saveError.value = true
  }
}

async function saveCurrent() {
  saveMessage.value = ''
  saveError.value = false
  try {
    await store.saveProvider({ ...editing })
    saveMessage.value = '保存成功！重启软件后生效。'
    // update the editing state with the new ID assigned by backend
    const saved = store.providers.find(
      (p) => p.label === editing.label && p.model === editing.model,
    )
    if (saved && !editing.id) {
      editing.id = saved.id
      isEditing.value = true
    }
  } catch (e: any) {
    saveMessage.value = `保存失败: ${e}`
    saveError.value = true
  }
}

// ---- Role assignment ----

async function onChatRoleChange(value: string) {
  try {
    await store.assignRole('chat', value || null)
  } catch (e: any) {
    console.error('Failed to set chat role:', e)
  }
}

async function onTranslateRoleChange(value: string) {
  try {
    await store.assignRole('translate', value === '__follow__' ? null : value)
  } catch (e: any) {
    console.error('Failed to set translate role:', e)
  }
}

// ---- Test ----

function startTest(p: LlmProviderConfig) {
  testProvider.value = p
  testMessage.value = ''
  testResponse.value = ''
  testError.value = ''
  view.value = 'test'
}

async function doTest() {
  if (!testProvider.value || !testMessage.value.trim()) return
  testing.value = true
  testResponse.value = ''
  testError.value = ''
  try {
    const res = await invoke<string>('test_llm_provider', {
      provider: testProvider.value,
      message: testMessage.value,
    })
    testResponse.value = res
  } catch (e: any) {
    testError.value = typeof e === 'string' ? e : (e.message || JSON.stringify(e))
  } finally {
    testing.value = false
  }
}

onMounted(async () => {
  await store.load()
})
</script>
