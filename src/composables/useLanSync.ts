import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type {
  PeerInfo,
  SyncPlan,
  SyncProgressEvent,
  SyncResult,
  SyncPhase,
} from '../types/lanSync'

// ─── 共享状态 ────────────────────────────────────────────────

const serverRunning = ref(false)
const serverPort = ref(0)
const peers = ref<PeerInfo[]>([])
const selectedPeer = ref<PeerInfo | null>(null)
const phase = ref<SyncPhase>('idle')
const syncPlan = ref<SyncPlan | null>(null)
const progress = ref<SyncProgressEvent>({
  phase: 'scanning',
  current: 0,
  total: 0,
  progress: 0,
  currentFile: null,
  bytesTransferred: 0,
  message: null,
})
const lastResult = ref<SyncResult | null>(null)
const errorMessage = ref('')
const dialogVisible = ref(false)

// ─── 内部监听 ────────────────────────────────────────────────

let unlistenProgress: (() => void) | null = null
let unlistenPlan: (() => void) | null = null
let unlistenComplete: (() => void) | null = null
let unlistenPeers: (() => void) | null = null
let initCount = 0

async function setupEventListeners() {
  if (unlistenProgress) return
  unlistenProgress = await listen<SyncProgressEvent>(
    'lan-sync-progress',
    (event) => {
      progress.value = event.payload
      phase.value = 'executing'
      if (event.payload.phase === 'complete') {
        phase.value = 'complete'
      }
    },
  )

  unlistenPlan = await listen<SyncPlan>('lan-sync-plan', (event) => {
    syncPlan.value = event.payload
    phase.value = 'planning'
  })

  unlistenComplete = await listen<SyncResult>(
    'lan-sync-complete',
    (event) => {
      lastResult.value = event.payload
      phase.value = event.payload.success ? 'complete' : 'error'
      errorMessage.value = event.payload.message
    },
  )

  unlistenPeers = await listen<PeerInfo[]>(
    'lan-sync-peers-updated',
    (event) => {
      peers.value = event.payload
    },
  )
}

function teardownEventListeners() {
  if (unlistenProgress) {
    unlistenProgress()
    unlistenProgress = null
  }
  if (unlistenPlan) {
    unlistenPlan()
    unlistenPlan = null
  }
  if (unlistenComplete) {
    unlistenComplete()
    unlistenComplete = null
  }
  if (unlistenPeers) {
    unlistenPeers()
    unlistenPeers = null
  }
}

// ─── 导出 composable ─────────────────────────────────────────

export function useLanSync() {
  function init() {
    if (initCount++ > 0) return
    setupEventListeners()
  }

  function destroy() {
    if (--initCount > 0) return
    teardownEventListeners()
    // 确保停止服务
    if (serverRunning.value) {
      invoke('lan_sync_stop_server').catch(() => {})
    }
  }

  /** 启动本地同步服务 */
  async function startServer(): Promise<number> {
    try {
      const port = await invoke<number>('lan_sync_start_server')
      serverRunning.value = true
      serverPort.value = port
      return port
    } catch (e) {
      errorMessage.value = String(e)
      throw e
    }
  }

  /** 停止本地同步服务 */
  async function stopServer(): Promise<void> {
    try {
      await invoke('lan_sync_stop_server')
      serverRunning.value = false
      serverPort.value = 0
    } catch (e) {
      errorMessage.value = String(e)
      throw e
    }
  }

  /** 扫描局域网设备 */
  async function scanPeers(): Promise<PeerInfo[]> {
    phase.value = 'scanning'
    errorMessage.value = ''
    try {
      const result = await invoke<PeerInfo[]>('lan_sync_scan_peers')
      peers.value = result
      phase.value = result.length > 0 ? 'idle' : 'idle'
      return result
    } catch (e) {
      phase.value = 'error'
      errorMessage.value = String(e)
      throw e
    }
  }

  /** 选择对等设备 */
  function selectPeer(peer: PeerInfo) {
    selectedPeer.value = peer
  }

  /** 计划拉取 */
  async function planPull(): Promise<void> {
    if (!selectedPeer.value) throw new Error('未选择对等设备')
    phase.value = 'fetching'
    errorMessage.value = ''
    try {
      await invoke('lan_sync_plan_pull', { peer: selectedPeer.value })
    } catch (e) {
      phase.value = 'error'
      errorMessage.value = String(e)
      throw e
    }
  }

  /** 计划推送 */
  async function planPush(): Promise<void> {
    if (!selectedPeer.value) throw new Error('未选择对等设备')
    phase.value = 'fetching'
    errorMessage.value = ''
    try {
      await invoke('lan_sync_plan_push', { peer: selectedPeer.value })
    } catch (e) {
      phase.value = 'error'
      errorMessage.value = String(e)
      throw e
    }
  }

  /** 执行拉取 */
  async function executePull(): Promise<SyncResult> {
    try {
      const result = await invoke<SyncResult>('lan_sync_execute_pull')
      return result
    } catch (e) {
      phase.value = 'error'
      errorMessage.value = String(e)
      throw e
    }
  }

  /** 执行推送 */
  async function executePush(): Promise<SyncResult> {
    try {
      const result = await invoke<SyncResult>('lan_sync_execute_push')
      return result
    } catch (e) {
      phase.value = 'error'
      errorMessage.value = String(e)
      throw e
    }
  }

  /** 打开对话框 */
  function openDialog() {
    dialogVisible.value = true
    reset()
    startServer().then(() => scanPeers())
  }

  /** 重启应用（桌面端），应用暂存的同步文件 */
  async function restart(): Promise<void> {
    try {
      await invoke('lan_sync_restart')
    } catch (e) {
      // 移动端或不支持时回退到手动重启提示
      errorMessage.value = '请手动重启应用以应用同步文件'
      phase.value = 'error'
    }
  }

  /** 关闭对话框 */
  function closeDialog() {
    dialogVisible.value = false
    stopServer().catch(() => {})
  }

  /** 重置状态 */
  function reset() {
    phase.value = 'idle'
    errorMessage.value = ''
    syncPlan.value = null
    lastResult.value = null
    progress.value = {
      phase: 'scanning',
      current: 0,
      total: 0,
      progress: 0,
      currentFile: null,
      bytesTransferred: 0,
      message: null,
    }
  }

  /** 格式化字节数 */
  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    if (bytes < 1024 * 1024 * 1024)
      return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`
  }

  /** 文件操作原因的中文标签 */
  function reasonLabel(reason: string): string {
    switch (reason) {
      case 'new':
        return '新增'
      case 'modified':
        return '修改'
      case 'newer':
        return '更新'
      default:
        return reason
    }
  }

  return {
    // 状态
    serverRunning,
    serverPort,
    peers,
    selectedPeer,
    phase,
    syncPlan,
    progress,
    lastResult,
    errorMessage,
    dialogVisible,
    // 方法
    init,
    destroy,
    startServer,
    stopServer,
    scanPeers,
    selectPeer,
    planPull,
    planPush,
    executePull,
    executePush,
    openDialog,
    closeDialog,
    restart,
    reset,
    formatBytes,
    reasonLabel,
  }
}
