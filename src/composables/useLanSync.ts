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
import { i18n } from '@/locales'

// ─── 共享状态 ────────────────────────────────────────────────

const serverRunning = ref(false)
const serverPort = ref(0)
const peers = ref<PeerInfo[]>([])
const selectedPeer = ref<PeerInfo | null>(null)
/** 本机配对令牌（另一台设备输入此令牌完成配对）。 */
const ownToken = ref('')
/** 已配对的设备 ID 集合。 */
const pairedDeviceIds = ref<Set<string>>(new Set())
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
      // 刷新配对状态（每台设备查询一次已保存的令牌）
      const paired = new Set<string>()
      for (const peer of result) {
        try {
          const token = await invoke<string | null>('lan_sync_get_peer_token', {
            deviceId: peer.deviceId,
          })
          if (token) paired.add(peer.deviceId)
        } catch {
          // 单台设备查询失败不影响整体扫描
        }
      }
      pairedDeviceIds.value = paired
      phase.value = result.length > 0 ? 'idle' : 'idle'
      return result
    } catch (e) {
      phase.value = 'error'
      errorMessage.value = String(e)
      throw e
    }
  }

  /** 查看本机配对令牌 */
  async function loadOwnToken(): Promise<string> {
    try {
      ownToken.value = await invoke<string>('lan_sync_get_token')
      return ownToken.value
    } catch (e) {
      errorMessage.value = String(e)
      throw e
    }
  }

  /** 与对端设备配对（保存其配对令牌）。 */
  async function pairPeer(deviceId: string, token: string): Promise<void> {
    try {
      await invoke('lan_sync_set_peer_token', { deviceId, token })
      pairedDeviceIds.value = new Set([...pairedDeviceIds.value, deviceId])
    } catch (e) {
      errorMessage.value = String(e)
      throw e
    }
  }

  /** 解除与某台设备的配对。 */
  async function unpairPeer(deviceId: string): Promise<void> {
    try {
      await invoke('lan_sync_remove_peer_token', { deviceId })
      const next = new Set(pairedDeviceIds.value)
      next.delete(deviceId)
      pairedDeviceIds.value = next
    } catch (e) {
      errorMessage.value = String(e)
      throw e
    }
  }

  function isPeerPaired(deviceId: string): boolean {
    return pairedDeviceIds.value.has(deviceId)
  }

  /** 选择对等设备 */
  function selectPeer(peer: PeerInfo) {
    selectedPeer.value = peer
  }

  /** 计划拉取 */
  async function planPull(): Promise<void> {
    if (!selectedPeer.value) throw new Error(i18n.global.t('stores.lanSync.noPeerSelected'))
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
    if (!selectedPeer.value) throw new Error(i18n.global.t('stores.lanSync.noPeerSelected'))
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
      errorMessage.value = i18n.global.t('stores.lanSync.manualRestart')
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

  /** 文件操作原因的界面显示标签 */
  function reasonLabel(reason: string): string {
    switch (reason) {
      case 'new':
        return i18n.global.t('stores.lanSync.reason.new')
      case 'modified':
        return i18n.global.t('stores.lanSync.reason.modified')
      case 'newer':
        return i18n.global.t('stores.lanSync.reason.newer')
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
    ownToken,
    pairedDeviceIds,
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
    loadOwnToken,
    pairPeer,
    unpairPeer,
    isPeerPaired,
    formatBytes,
    reasonLabel,
  }
}
