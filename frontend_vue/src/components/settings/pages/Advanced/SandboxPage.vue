<template>
  <section v-if="view === 'sandbox'" class="space-y-4">
    <div class="flex items-center justify-between gap-3">
      <div>
        <h3 class="text-xl font-bold text-brand flex items-center gap-2">
          <Shield :size="20" />
          代码沙盒
        </h3>
        <p class="text-xs text-white/70 mt-1">AI 可安全读写文件和执行白名单命令的工作空间</p>
      </div>
      <div class="flex items-center gap-2">
        <button class="action-btn" @click="loadFiles">
          <RefreshCw :size="16" />
          刷新
        </button>
        <button class="action-btn bg-emerald-500/30 hover:bg-emerald-500/45" @click="createFile">
          <FilePlus2 :size="16" />
          新建文件
        </button>
      </div>
    </div>

    <div class="rounded-lg bg-white/10 px-3 py-2 text-sm text-white/80 flex items-center gap-2">
      <Folder :size="16" />
      <span>sandbox</span>
    </div>

    <div
      v-if="error"
      class="rounded-lg border border-rose-400/40 bg-rose-500/15 p-3 text-sm text-rose-100"
    >
      {{ error }}
    </div>

    <div class="rounded-lg border border-cyan-300/20 bg-slate-900/20 overflow-hidden">
      <div v-if="loading" class="p-4 text-white/60">正在读取...</div>
      <div
        v-else-if="files.length === 0"
        class="min-h-44 flex flex-col items-center justify-center text-white/55"
      >
        <FolderOpen :size="34" class="mb-3" />
        <p>沙盒是空的</p>
        <p class="text-xs mt-2">AI 可以在这里创建和编辑文件</p>
      </div>
      <div v-else class="divide-y divide-white/5">
        <div
          v-for="item in files"
          :key="item.name"
          class="px-4 py-3 text-sm text-white hover:bg-white/10 flex items-center gap-3"
          :class="{ 'bg-cyan-400/15': editorVisible && selectedPath === item.name }"
        >
          <button class="flex flex-1 items-center gap-3 min-w-0 text-left" @click="openItem(item)">
            <Folder v-if="item.type === 'directory'" :size="18" class="text-cyan-200 shrink-0" />
            <FileCode2 v-else :size="18" class="text-emerald-200 shrink-0" />
            <span class="font-semibold truncate">{{ item.name }}</span>
          </button>
          <span class="text-xs text-white/55 w-16 text-right">{{ formatSize(item.size) }}</span>
          <span class="text-xs text-white/55 w-36 text-right">{{ formatTime(item.modified) }}</span>
          <button
            v-if="item.type === 'file'"
            class="p-1 rounded hover:bg-rose-500/25 text-white/65 hover:text-rose-100"
            title="删除"
            @click="deleteFile(item.name)"
          >
            <Trash2 :size="16" />
          </button>
        </div>
      </div>
    </div>

    <div
      v-if="editorVisible"
      class="rounded-lg border border-cyan-300/20 bg-slate-900/25 p-4 space-y-3"
    >
      <div class="flex items-center justify-between gap-3">
        <input
          v-model="selectedPath"
          class="flex-1 rounded-lg bg-black/25 border border-white/10 px-3 py-2 text-white outline-none focus:border-cyan-300"
          placeholder="例如 hello.py"
        />
        <button class="action-btn" :disabled="!selectedPath" @click="saveFile">保存</button>
      </div>
      <textarea
        v-model="content"
        class="w-full min-h-72 rounded-lg bg-black/25 border border-white/10 p-3 text-sm text-white font-mono outline-none focus:border-cyan-300 resize-y"
        spellcheck="false"
        placeholder="选择或新建一个文件后在这里编辑"
      ></textarea>
    </div>

    <div class="rounded-lg border border-cyan-300/20 bg-slate-900/25 p-4 space-y-3">
      <div class="flex items-center gap-2 text-brand font-bold">
        <Terminal :size="18" />
        <span>执行命令</span>
      </div>
      <div class="flex gap-2">
        <input
          v-model="packageName"
          class="flex-1 rounded-lg bg-black/25 border border-white/10 px-3 py-2 text-white outline-none focus:border-cyan-300"
          placeholder="输入库名，例如 pygame"
          @keydown.enter.prevent="installPackage"
        />
        <button
          class="action-btn bg-emerald-500/30 hover:bg-emerald-500/45"
          :disabled="!packageName || executing"
          @click="installPackage"
        >
          <PackagePlus :size="16" />
          安装库
        </button>
      </div>
      <div class="flex gap-2">
        <input
          v-model="command"
          class="flex-1 rounded-lg bg-black/25 border border-white/10 px-3 py-2 text-white outline-none focus:border-cyan-300"
          placeholder="例如 python gomoku.py"
          @keydown.enter.prevent="executeCommand"
        />
        <button
          class="action-btn bg-cyan-500/30 hover:bg-cyan-500/45"
          :disabled="!command || executing"
          @click="executeCommand"
        >
          <Play :size="16" />
          执行
        </button>
      </div>
      <pre
        v-if="commandOutput"
        class="rounded-lg bg-black/30 border border-white/10 p-3 text-xs text-white whitespace-pre-wrap break-words max-h-48 overflow-y-auto"
        >{{ commandOutput }}</pre
      >
    </div>
  </section>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import {
  FileCode2,
  FilePlus2,
  Folder,
  FolderOpen,
  PackagePlus,
  Play,
  RefreshCw,
  Shield,
  Terminal,
  Trash2,
} from 'lucide-vue-next'
import http from '@/api/http'

type SandboxItem = {
  name: string
  type: 'file' | 'directory'
  size?: number | null
  modified?: number
}

type ExecuteResult = {
  ok: boolean
  package?: string
  command?: string
  resolved_command?: string
  returncode?: number
  stdout?: string
  stderr?: string
  error?: string
  cwd?: string
}

const props = defineProps<{
  view: string
}>()

const selectedPath = ref('')
const content = ref('')
const files = ref<SandboxItem[]>([])
const loading = ref(false)
const error = ref('')
const editorVisible = ref(false)
const command = ref('')
const commandOutput = ref('')
const packageName = ref('')
const executing = ref(false)

const loadFiles = async () => {
  loading.value = true
  error.value = ''
  try {
    const data = await http.get<{ items: SandboxItem[] }>('/v1/chat/sandbox/list', {
      params: { path: '.' },
      silent: true,
    })
    files.value = data.items || []
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
    files.value = []
  } finally {
    loading.value = false
  }
}

const openItem = async (item: SandboxItem) => {
  if (item.type === 'directory') return
  if (editorVisible.value && selectedPath.value === item.name) {
    selectedPath.value = ''
    content.value = ''
    editorVisible.value = false
    return
  }
  selectedPath.value = item.name
  editorVisible.value = true
  error.value = ''
  try {
    const data = await http.get<{ content: string }>('/v1/chat/sandbox/read', {
      params: { path: item.name },
      silent: true,
    })
    content.value = data.content || ''
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  }
}

const createFile = () => {
  selectedPath.value = 'hello.py'
  content.value = 'print("hello from LingChat sandbox")\n'
  editorVisible.value = true
}

const saveFile = async () => {
  error.value = ''
  try {
    await http.post(
      '/v1/chat/sandbox/write',
      {
        path: selectedPath.value,
        content: content.value,
      },
      { silent: true },
    )
    await loadFiles()
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  }
}

const deleteFile = async (path = selectedPath.value) => {
  if (!path) return
  error.value = ''
  try {
    await http.post('/v1/chat/sandbox/delete', { path }, { silent: true })
    if (selectedPath.value === path) {
      selectedPath.value = ''
      content.value = ''
      editorVisible.value = false
    }
    await loadFiles()
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  }
}

const executeCommand = async () => {
  if (!command.value || executing.value) return
  executing.value = true
  error.value = ''
  commandOutput.value = ''
  try {
    const result = await http.post<ExecuteResult>(
      '/v1/chat/sandbox/execute',
      {
        command: command.value,
        timeout: 30,
      },
      { silent: true },
    )
    commandOutput.value = [
      `$ ${result.command || command.value}`,
      `exit code: ${result.returncode ?? (result.ok ? 0 : 1)}`,
      result.stdout ? `\nstdout:\n${result.stdout}` : '',
      result.stderr ? `\nstderr:\n${result.stderr}` : '',
      result.error ? `\nerror:\n${result.error}` : '',
    ]
      .filter(Boolean)
      .join('\n')
  } catch (err) {
    commandOutput.value = err instanceof Error ? err.message : String(err)
  } finally {
    executing.value = false
  }
}

const installPackage = async () => {
  const name = packageName.value.trim()
  if (!name || executing.value) return
  if (!/^[a-zA-Z0-9_.-]+$/.test(name)) {
    error.value = '库名只能包含字母、数字、下划线、点和短横线'
    return
  }

  command.value = `python -m pip install --target Python ${name}`
  executing.value = true
  error.value = ''
  commandOutput.value = ''
  try {
    const result = await http.post<ExecuteResult>(
      '/v1/chat/sandbox/install',
      {
        package: name,
      },
      { silent: true },
    )
    commandOutput.value = [
      `$ ${result.command || command.value}`,
      result.resolved_command ? `resolved: ${result.resolved_command}` : '',
      `exit code: ${result.returncode ?? (result.ok ? 0 : 1)}`,
      result.stdout ? `\nstdout:\n${result.stdout}` : '',
      result.stderr ? `\nstderr:\n${result.stderr}` : '',
      result.error ? `\nerror:\n${result.error}` : '',
      result.ok ? '\n已安装到沙盒 Python 目录，运行脚本时会自动加入 PYTHONPATH。' : '',
    ]
      .filter(Boolean)
      .join('\n')
    await loadFiles()
  } catch (err) {
    commandOutput.value = err instanceof Error ? err.message : String(err)
  } finally {
    executing.value = false
  }
}

const formatSize = (value?: number | null) => {
  if (!value) return ''
  if (value < 1024) return `${value} B`
  return `${(value / 1024).toFixed(1)} KB`
}

const formatTime = (value?: number) => {
  if (!value) return ''
  return new Date(value * 1000).toLocaleString()
}

onMounted(loadFiles)
</script>

<style scoped>
.action-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.4rem;
  border-radius: 0.5rem;
  background: rgba(255, 255, 255, 0.12);
  color: white;
  padding: 0.5rem 0.75rem;
  font-size: 0.875rem;
  transition: background 0.2s ease;
}

.action-btn:hover {
  background: rgba(255, 255, 255, 0.2);
}

.action-btn:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
</style>
