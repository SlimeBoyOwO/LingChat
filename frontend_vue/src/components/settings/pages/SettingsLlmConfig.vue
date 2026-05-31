<template>
  <Teleport to="body">
    <Transition name="llm-slide">
      <div class="fixed inset-0 z-[1200]">
        <!-- 模糊遮罩 -->
        <div class="absolute inset-0 backdrop-blur-sm bg-black/45" @click="emit('close')"></div>

        <!-- 面板 -->
        <div class="absolute inset-0 flex flex-col overflow-hidden bg-transparent">
          <!-- 头部：标题靠左，操作按钮统一在右上角 -->
          <div class="flex items-center justify-between shrink-0 px-5 py-3">
            <div class="flex items-center gap-2 text-[var(--accent-color)] font-bold text-base drop-shadow-md">
              <Icon icon="bot" :size="20" />
              <span>{{ headerTitle }}</span>
            </div>
            <div class="flex items-center gap-2">
              <button
                v-if="view !== 'list'"
                class="p-1.5 rounded-full text-white hover:bg-white/10 hover:text-[var(--accent-color)] hover:rotate-90 transition-all duration-300"
                @click="onBack"
              >
                <Icon icon="chevron-left" :size="28" />
              </button>
              <button
                v-if="view === 'list'"
                class="p-1.5 rounded-full text-white hover:bg-white/10 hover:text-[var(--accent-color)] hover:rotate-90 transition-all duration-300"
                @click="emit('close')"
              >
                <Icon icon="close" :size="28" />
              </button>
            </div>
          </div>

          <!-- 内容 -->
          <div class="flex-1 overflow-y-auto px-6 pb-8">
            <!-- 加载 -->
            <div v-if="store.loading" class="flex flex-col items-center justify-center gap-3 h-48 text-white/60 text-sm">
              <div class="w-8 h-8 border-3 border-white/20 border-t-[var(--accent-color)] rounded-full animate-spin"></div>
              <span>加载中...</span>
            </div>

            <!-- ===== 列表视图 ===== -->
            <template v-else-if="view === 'list'">
              <div class="flex flex-col gap-3">
                <div class="text-[var(--accent-color)] font-bold text-sm pb-1 border-b border-white/10">
                  已配置的方案
                </div>

                <div v-if="store.configs.length === 0" class="text-white/50 text-center py-8 text-sm">
                  暂无配置方案，点击下方按钮新建
                </div>

                <div
                  v-for="cfg in store.configs"
                  :key="cfg.name"
                  class="bg-white/8 backdrop-blur-xl border border-white/10 rounded-xl p-4 flex flex-col gap-2 transition-all duration-200 hover:bg-white/12 hover:border-[var(--accent-color)]"
                >
                  <div class="flex items-center justify-between">
                    <span class="text-white font-semibold text-sm">{{ cfg.display_name || cfg.name }}</span>
                    <span v-if="cfg.is_active" class="text-xs px-2 py-0.5 rounded-full bg-green-500/20 text-green-400 font-semibold">当前</span>
                  </div>
                  <p class="text-white/50 text-xs m-0">{{ cfg.description || '无描述' }}</p>
                  <div class="flex gap-1.5 flex-wrap">
                    <span class="text-[10px] px-2 py-0.5 rounded-full bg-[rgba(121,217,255,0.2)] text-[#79d9ff]">
                      {{ cfg.main_provider || '?' }}
                    </span>
                  </div>
                  <div class="flex gap-1.5 flex-wrap pt-1">
                    <button
                      v-if="!cfg.is_active"
                      class="text-xs px-3 py-1 rounded-lg bg-[var(--accent-color)] text-white hover:opacity-85 transition-all"
                      @click="activateConfig(cfg.name)"
                    >激活</button>
                    <button
                      class="text-xs px-3 py-1 rounded-lg bg-white/10 text-white border border-white/15 hover:bg-white/20 transition-all"
                      @click="startEdit(cfg.name)"
                    >编辑</button>
                    <button
                      class="text-xs px-3 py-1 rounded-lg bg-white/10 text-white border border-white/15 hover:bg-white/20 transition-all"
                      @click="startTest(cfg.name)"
                    >测试</button>
                    <button
                      v-if="cfg.name !== 'default'"
                      class="text-xs px-3 py-1 rounded-lg bg-red-500/20 text-red-400 border border-red-500/30 hover:bg-red-500/35 transition-all"
                      @click="confirmDelete(cfg.name)"
                    >删除</button>
                  </div>
                </div>

                <button
                  class="w-full py-2.5 rounded-lg bg-[var(--accent-color)] text-white text-sm font-medium flex items-center justify-center gap-1.5 hover:opacity-85 transition-all"
                  @click="startAdd"
                >
                  <Icon icon="plus" :size="16" />
                  新建配置
                </button>

                <div class="text-white/50 text-xs text-center pt-2">
                  当前激活：<strong>{{ store.activeSummary?.display_name || store.activeName || '无' }}</strong>
                </div>
              </div>
            </template>

            <!-- ===== 编辑视图 ===== -->
            <template v-else-if="view === 'edit'">
              <div class="flex flex-col gap-2.5">
                <div class="flex flex-col gap-1">
                  <label class="text-white/70 text-xs font-medium">配置名称</label>
                  <input v-model="form.config_name" type="text" class="w-full px-3 py-2 rounded-lg border border-white/12 bg-white/8 text-white text-sm outline-none focus:border-[var(--accent-color)] placeholder:text-white/25" placeholder="例如: 我的配置" />
                </div>
                <div class="flex flex-col gap-1">
                  <label class="text-white/70 text-xs font-medium">配置描述</label>
                  <input v-model="form.config_description" type="text" class="w-full px-3 py-2 rounded-lg border border-white/12 bg-white/8 text-white text-sm outline-none focus:border-[var(--accent-color)] placeholder:text-white/25" placeholder="可选的描述" />
                </div>

                <div class="text-[var(--accent-color)] font-bold text-sm pb-1 border-b border-white/10 mt-1">主对话模型</div>

                <!-- 动态渲染 main 配置字段：特殊字段自定义，其余从 schema 读取 -->
                <template v-for="setting in mainSettings" :key="setting.key">
                  <!-- provider → 特殊下拉框 -->
                  <FormField v-if="setting.key === 'provider'" label="提供商类型">
                    <select v-model="form.main.provider" class="w-full px-3 py-2 rounded-lg border border-white/12 bg-white/8 text-white text-sm outline-none focus:border-[var(--accent-color)]">
                      <option value="webllm" class="bg-[#222]">OpenAI 兼容 (DeepSeek / 通义千问)</option>
                      <option value="gemini" class="bg-[#222]">Gemini</option>
                      <option value="kimi-code" class="bg-[#222]">Kimi Code</option>
                      <option value="ollama" class="bg-[#222]">Ollama</option>
                      <option value="lmstudio" class="bg-[#222]">LM Studio</option>
                    </select>
                  </FormField>

                  <!-- enable_thinking → 特殊下拉框 -->
                  <FormField v-else-if="setting.key === 'enable_thinking'" label="思考链">
                    <select v-model="form.main.enable_thinking" class="w-full px-3 py-2 rounded-lg border border-white/12 bg-white/8 text-white text-sm outline-none focus:border-[var(--accent-color)]">
                      <option value="none" class="bg-[#222]">不启用</option>
                      <option value="true" class="bg-[#222]">启用</option>
                      <option value="false" class="bg-[#222]">禁用</option>
                    </select>
                  </FormField>

                  <!-- 其余字段 → 根据 schema type 动态渲染 -->
                  <FormField v-else :label="setting.description || setting.key">
                    <SettingItem :setting="setting" @update:value="(v) => { (form.main as any)[setting.key] = v }" />
                  </FormField>
                </template>

                <div class="text-[var(--accent-color)] font-bold text-sm pb-1 border-b border-white/10 mt-1">翻译模型</div>
                <FormField label="提供商类型">
                  <select v-model="form.translator.provider" class="w-full px-3 py-2 rounded-lg border border-white/12 bg-white/8 text-white text-sm outline-none focus:border-[var(--accent-color)]">
                    <option value="none" class="bg-[#222]">跟随主对话模型</option>
                    <option value="webllm" class="bg-[#222]">OpenAI 兼容</option>
                    <option value="gemini" class="bg-[#222]">Gemini</option>
                    <option value="ollama" class="bg-[#222]">Ollama</option>
                    <option value="lmstudio" class="bg-[#222]">LM Studio</option>
                    <option value="qwen-translate" class="bg-[#222]">Qwen 翻译</option>
                  </select>
                </FormField>
                <template v-if="form.translator.provider !== 'none'">
                  <FormField label="模型名称">
                    <input v-model="form.translator.model" type="text" class="w-full px-3 py-2 rounded-lg border border-white/12 bg-white/8 text-white text-sm outline-none focus:border-[var(--accent-color)]" />
                  </FormField>
                  <FormField label="API 密钥">
                    <input v-model="form.translator.api_key" type="password" class="w-full px-3 py-2 rounded-lg border border-white/12 bg-white/8 text-white text-sm outline-none focus:border-[var(--accent-color)]" />
                  </FormField>
                  <FormField label="API 地址">
                    <input v-model="form.translator.base_url" type="text" class="w-full px-3 py-2 rounded-lg border border-white/12 bg-white/8 text-white text-sm outline-none focus:border-[var(--accent-color)]" />
                  </FormField>
                </template>

                <div class="flex gap-2.5 pt-2">
                  <button class="px-5 py-2 rounded-lg bg-[var(--accent-color)] text-white text-sm font-medium hover:opacity-85 disabled:opacity-50 transition-all" @click="doSave" :disabled="saving">
                    {{ saving ? '保存中...' : '保存' }}
                  </button>
                  <button class="px-5 py-2 rounded-lg bg-transparent text-white/60 text-sm hover:text-white transition-all" @click="view = 'list'">取消</button>
                </div>
                <p v-if="saveMsg" :class="saveError ? 'text-red-400' : 'text-green-400'" class="text-xs mt-1">{{ saveMsg }}</p>
              </div>
            </template>

            <!-- ===== 测试视图 ===== -->
            <template v-else-if="view === 'test'">
              <div class="flex flex-col gap-3">
                <div class="text-white/70 text-sm">测试方案：<strong>{{ testSchemeName }}</strong></div>
                <FormField label="测试对象">
                  <select v-model="testSection" class="w-full px-3 py-2 rounded-lg border border-white/12 bg-white/8 text-white text-sm outline-none focus:border-[var(--accent-color)]">
                    <option value="main" class="bg-[#222]">主对话模型</option>
                    <option value="translator" class="bg-[#222]">翻译模型</option>
                  </select>
                </FormField>
                <FormField label="测试消息">
                  <input v-model="testMessage" type="text" class="w-full px-3 py-2 rounded-lg border border-white/12 bg-white/8 text-white text-sm outline-none focus:border-[var(--accent-color)]" placeholder="输入测试消息..." @keyup.enter="doTest" />
                </FormField>
                <button class="self-start px-4 py-2 rounded-lg bg-[var(--accent-color)] text-white text-sm font-medium hover:opacity-85 disabled:opacity-50 transition-all" :disabled="testing || !testMessage.trim()" @click="doTest">
                  {{ testing ? '测试中...' : '发送' }}
                </button>

                <div class="min-h-[120px] rounded-xl border border-white/10 bg-black/20 p-4 flex items-center justify-center">
                  <div v-if="testing" class="flex flex-col items-center gap-2 text-white/50 text-sm">
                    <div class="w-6 h-6 border-3 border-white/20 border-t-[var(--accent-color)] rounded-full animate-spin"></div>
                    <span>等待响应...</span>
                  </div>
                  <div v-else-if="testError" class="text-red-400 text-sm whitespace-pre-wrap break-words w-full">{{ testError }}</div>
                  <div v-else-if="testResult" class="text-white text-sm whitespace-pre-wrap break-words w-full">{{ testResult }}</div>
                  <div v-else class="text-white/30 text-sm">输入消息并点击发送，测试模型响应</div>
                </div>
              </div>
            </template>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted, watch } from 'vue'
import Icon from '../../base/widget/Icon.vue'
import FormField from '../../base/items/FormField.vue'
import SettingItem from '../../base/items/SettingItem.vue'
import { useLlmConfigStore } from '../../../stores/modules/llm-config'
import { testProvider, getLlmTomlSettings, getConfigTemplate } from '../../../api/services/llm-config'
import type { LlmConfigScheme } from '../../../api/services/llm-config'

/** 从 /v1/llm-config/settings 提取当前配置方案的 [main] 段 schema */
async function fetchMainSettings(): Promise<any[]> {
  try {
    const data = await getLlmTomlSettings()
    // data 形如 { "LLM 配置方案: 默认配置": { subcategories: { main: { settings: [...] } } } }
    for (const key of Object.keys(data || {})) {
      const scheme = data[key]
      const main = scheme?.subcategories?.main
      if (main?.settings) return main.settings
    }
  } catch (e) {
    console.warn('获取 LLM 配置 schema 失败', e)
  }
  return []
}

const store = useLlmConfigStore()

const view = ref<'list' | 'edit' | 'test'>('list')

onMounted(() => { store.load() })

const editName = ref('')
const form = reactive<LlmConfigScheme>({
  config_name: '',
  config_description: '',
  main: { provider: 'webllm', model: '', api_key: '', base_url: '' },
  translator: { provider: 'none', model: '', api_key: '', base_url: '' },
})
const saving = ref(false)
const saveMsg = ref('')
const saveError = ref(false)

/** 动态 [main] 段配置项列表（排除特殊处理的字段） */
const mainSettings = ref<any[]>([])
watch(view, async (v) => {
  if (v === 'edit') {
    const all = await fetchMainSettings()
    mainSettings.value = all
    // 将 schema 中的默认值注入 form.main，确保动态字段可被保存
    for (const s of mainSettings.value) {
      if (!(s.key in form.main)) {
        (form.main as any)[s.key] = s.value
      }
    }
  }
})

const testSchemeName = ref('')
const testSection = ref<'main' | 'translator'>('main')
const testMessage = ref('只需回复两个字：你好')
const testing = ref(false)
const testResult = ref('')
const testError = ref('')

const headerTitle = computed(() => {
  if (view.value === 'edit') return editName.value ? `编辑: ${editName.value}` : '新建配置'
  if (view.value === 'test') return `测试: ${testSchemeName.value}`
  return '大模型管理'
})

const emit = defineEmits<{ close: [] }>()

function onBack() {
  if (view.value !== 'list') {
    view.value = 'list'
    saveMsg.value = ''
  }
}

// ——— 列表操作 ———
async function activateConfig(name: string) {
  try { await store.switchTo(name) }
  catch (e: any) { console.error('激活失败', e) }
}

async function startAdd() {
  editName.value = ''
  try {
    const tpl = await getConfigTemplate()
    Object.assign(form, tpl)
  } catch {
    // fallback: 用最小空模板
    Object.assign(form, {
      config_name: '', config_description: '',
      main: { provider: 'webllm', model: '', api_key: '', base_url: '' },
      translator: { provider: 'none', model: '', api_key: '', base_url: '' },
    })
  }
  view.value = 'edit'
}

async function startEdit(name: string) {
  editName.value = name
  try {
    const cfg = await store.getConfig(name)
    Object.assign(form, {
      config_name: cfg.config_name || name,
      config_description: cfg.config_description || '',
      main: { ...cfg.main },
      translator: { ...cfg.translator },
    })
    view.value = 'edit'
  } catch (e) {
    console.error('加载配置失败', e)
  }
}

function confirmDelete(name: string) {
  if (window.confirm(`确定删除配置方案「${name}」吗？`)) store.remove(name)
}

// ——— 编辑 ———
async function doSave() {
  saving.value = true
  saveMsg.value = ''
  saveError.value = false
  try {
    const name = editName.value || form.config_name || 'default'
    await store.save(name, { ...form })
    saveMsg.value = '保存成功'
    view.value = 'list'
  } catch (e: any) {
    saveMsg.value = `保存失败: ${e.message}`
    saveError.value = true
  } finally { saving.value = false }
}

// ——— 测试 ———
async function doTest() {
  const cfg = store.activeConfig
  if (!cfg) return
  const section = cfg[testSection.value]
  if (!section) return

  testing.value = true
  testResult.value = ''
  testError.value = ''

  try {
    const res = await testProvider({
      provider: section.provider,
      model: section.model,
      api_key: section.api_key,
      base_url: section.base_url,
      proxy: (section as any).proxy || '',
      temperature: (section as any).temperature ?? null,
      top_p: (section as any).top_p ?? null,
      enable_thinking: (section as any).enable_thinking || 'none',
      message: testMessage.value,
    })
    if (res.status === 'success') testResult.value = res.response
    else testError.value = res.response || '测试失败'
  } catch (e: any) {
    testError.value = `请求错误: ${e.message}`
  } finally { testing.value = false }
}

function startTest(name: string) {
  testSchemeName.value = name
  testSection.value = 'main'
  testMessage.value = '只需回复两个字：你好'
  testResult.value = ''
  testError.value = ''
  view.value = 'test'
}

</script>

<style>
/* 滑块过渡动画 */
.llm-slide-enter-active,
.llm-slide-leave-active {
  transition: opacity 0.3s ease;
}
.llm-slide-enter-from,
.llm-slide-leave-to {
  opacity: 0;
}
.llm-slide-enter-from > div:last-child,
.llm-slide-leave-to > div:last-child {
  transform: translateX(-100%);
}
</style>
