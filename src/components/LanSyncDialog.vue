<template>
  <Transition name="modal">
    <div
      v-if="visible"
      class="fixed inset-0 z-50 flex items-center justify-center p-4 backdrop-blur-md bg-black/40"
      @click="emit('close')"
    >
      <div
        class="relative w-full max-w-lg max-h-[80vh] overflow-hidden rounded-3xl border border-white/20 bg-slate-900/40 backdrop-blur-2xl shadow-2xl flex flex-col"
        @click.stop
      >
        <!-- ─── 头部 ─── -->
        <div class="p-5 flex items-center gap-3 bg-white/10 border-b border-white/10">
          <div
            class="w-10 h-10 rounded-xl flex items-center justify-center"
            :class="headerIconBg"
          >
            <Wifi :size="20" class="text-white" />
          </div>
          <div class="flex-1">
            <h2 class="text-lg font-bold text-white leading-none">{{ dialogTitle }}</h2>
            <p class="text-white/50 text-xs mt-1">{{ dialogSubtitle }}</p>
          </div>
          <button
            @click="emit('close')"
            class="p-2 hover:bg-red-500/20 text-white/50 hover:text-white rounded-full transition-colors"
          >
            <Icon icon="close" class="w-5 h-5" />
          </button>
        </div>

        <!-- ─── 内容区 ─── -->
        <div class="flex-1 overflow-y-auto p-5 space-y-4">
          <!-- 设备列表：扫描中 -->
          <div v-if="view === 'device-list' && phase === 'scanning'" class="text-center py-8">
            <div class="animate-spin w-8 h-8 border-3 border-cyan-200/20 border-t-cyan-400 rounded-full mx-auto mb-3"></div>
            <p class="text-white/80 text-sm">{{ $t('ui.lanSync.scanning') }}</p>
            <p class="text-white/40 text-xs mt-1">{{ $t('ui.lanSync.serverPort', { port: serverPort }) }}</p>
          </div>

          <!-- 设备列表：获取对端清单中 -->
          <div v-if="view === 'device-list' && phase === 'fetching'" class="text-center py-8">
            <div class="animate-spin w-8 h-8 border-3 border-indigo-200/20 border-t-indigo-400 rounded-full mx-auto mb-3"></div>
            <p class="text-white/80 text-sm">{{ $t('ui.lanSync.fetching') }}</p>
            <p class="text-white/40 text-xs mt-1">{{ $t('ui.lanSync.fetchingHint') }}</p>
          </div>

          <!-- 设备列表：结果 -->
          <div v-if="view === 'device-list' && phase !== 'scanning'" class="space-y-3">
            <div class="flex items-center justify-between">
              <span class="text-white/50 text-xs">{{ $t('ui.lanSync.peersFound', { count: peers.length }) }}</span>
              <button
                @click="emit('rescan')"
                class="text-xs text-cyan-400 hover:text-cyan-300 transition-colors"
              >
                {{ $t('ui.lanSync.rescan') }}
              </button>
            </div>

            <div v-if="peers.length === 0" class="text-center py-6 text-white/40 text-sm">
              <p>{{ $t('ui.lanSync.noPeers') }}</p>
              <p class="text-xs mt-1">{{ $t('ui.lanSync.noPeersHint') }}</p>
            </div>

            <div
              v-for="peer in peers"
              :key="peer.deviceId"
              class="bg-white/5 rounded-2xl p-4 border border-white/10 hover:border-white/20 transition-all"
            >
              <div class="flex items-center justify-between">
                <div class="space-y-0.5">
                  <p class="font-bold text-white text-sm">{{ peer.deviceName }}</p>
                  <p class="text-white/40 text-xs">{{ peer.host }}:{{ peer.port }}</p>
                  <p class="text-white/30 text-xs">
                    {{ $t('ui.lanSync.peerInfo', { version: peer.dataVersion, count: peer.fileCount }) }}
                  </p>
                </div>
                <div class="flex gap-2">
                  <button
                    @click="emit('pull', peer)"
                    class="px-4 py-1.5 rounded-full bg-cyan-500/80 hover:bg-cyan-500 text-white text-xs font-semibold border border-cyan-400/50 shadow-lg shadow-cyan-500/20 transition-all active:scale-95"
                  >
                    {{ $t('ui.lanSync.pull') }}
                  </button>
                  <button
                    @click="emit('push', peer)"
                    class="px-4 py-1.5 rounded-full bg-amber-500/80 hover:bg-amber-500 text-white text-xs font-semibold border border-amber-400/50 shadow-lg shadow-amber-500/20 transition-all active:scale-95"
                  >
                    {{ $t('ui.lanSync.push') }}
                  </button>
                </div>
              </div>
            </div>
          </div>

          <!-- 同步计划 -->
          <div v-if="view === 'sync-plan' && syncPlan" class="space-y-4">
            <section>
              <h3 class="text-white font-bold text-sm mb-3 flex items-center gap-2">
                <span class="w-1 h-4 bg-indigo-500 rounded-full"></span> {{ $t('ui.lanSync.overview') }}
              </h3>
              <div class="bg-white/5 rounded-xl p-3 text-xs space-y-1.5 text-white/70">
                <p>
                  <span class="text-white/50">{{ $t('ui.lanSync.directionLabel') }}</span>
                  {{ syncPlan.direction === 'pull' ? $t('ui.lanSync.directionPull') : $t('ui.lanSync.directionPush') }}
                </p>
                <p>
                  <span class="text-white/50">{{ $t('ui.lanSync.deviceLabel') }}</span>
                  {{ syncPlan.peer.deviceName }}
                </p>
                <p>
                  <span class="text-white/50">{{ $t('ui.lanSync.transferLabel') }}</span>
                  {{ $t('ui.lanSync.transferSummary', { count: syncPlan.filesToTransfer.length, size: formatBytes(syncPlan.totalBytes) }) }}
                </p>
                <p v-if="syncPlan.filesToDelete.length" class="text-red-400">
                  <span class="text-white/50">{{ $t('ui.lanSync.deleteLabel') }}</span>
                  {{ $t('ui.lanSync.fileCount', { count: syncPlan.filesToDelete.length }) }}
                </p>
              </div>
            </section>

            <section v-if="syncPlan.filesToTransfer.length > 0">
              <h3 class="text-white font-bold text-sm mb-3 flex items-center gap-2">
                <span class="w-1 h-4 bg-orange-500 rounded-full"></span> {{ $t('ui.lanSync.fileList') }}
              </h3>
              <div class="max-h-48 overflow-y-auto bg-white/5 rounded-xl p-3 text-xs space-y-1">
                <p
                  v-for="file in syncPlan.filesToTransfer.slice(0, 50)"
                  :key="file.path"
                  class="truncate text-white/60"
                >
                  <span
                    :class="{
                      'text-emerald-400': file.reason === 'new',
                      'text-amber-400': file.reason === 'modified',
                      'text-cyan-400': file.reason === 'newer',
                    }"
                  >[{{ reasonLabel(file.reason) }}]</span>
                  {{ file.path }}
                  <span class="text-white/30">({{ formatBytes(file.size) }})</span>
                </p>
                <p v-if="syncPlan.filesToTransfer.length > 50" class="text-white/30 text-center">
                  {{ $t('ui.lanSync.moreFiles', { count: syncPlan.filesToTransfer.length - 50 }) }}
                </p>
              </div>
            </section>
          </div>

          <!-- 传输进度 -->
          <div v-if="view === 'progress'" class="text-center py-4 space-y-4">
            <div class="w-full bg-white/10 rounded-full h-2.5 overflow-hidden">
              <div
                class="h-full bg-gradient-to-r from-cyan-500 to-indigo-500 rounded-full transition-all duration-500 ease-out"
                :style="{ width: progress.progress + '%' }"
              ></div>
            </div>
            <p class="text-white/80 text-sm">{{ progress.message || $t('ui.lanSync.transferring') }}</p>
            <p v-if="progress.currentFile" class="text-white/40 text-xs truncate px-4">
              {{ progress.currentFile }}
            </p>
            <p class="text-white/40 text-xs">
              {{ formatBytes(progress.bytesTransferred) }}
              <template v-if="syncPlan"> / {{ formatBytes(syncPlan.totalBytes) }}</template>
            </p>
          </div>

          <!-- 结果：有 lastResult（来自 lan-sync-complete 事件） -->
          <div v-if="view === 'result' && lastResult" class="text-center py-4 space-y-3">
            <div
              class="w-14 h-14 rounded-full mx-auto flex items-center justify-center"
              :class="lastResult.success ? 'bg-emerald-500/20' : 'bg-red-500/20'"
            >
              <Check v-if="lastResult.success" class="w-7 h-7 text-emerald-400" />
              <X v-else class="w-7 h-7 text-red-400" />
            </div>
            <p v-if="lastResult.success" class="text-emerald-400 font-bold text-lg">{{ $t('ui.lanSync.syncComplete') }}</p>
            <p v-else class="text-red-400 font-bold text-lg">{{ $t('ui.lanSync.syncFailed') }}</p>
            <p class="text-white/50 text-xs">{{ lastResult.message }}</p>
            <div v-if="lastResult.success" class="text-white/40 text-xs space-y-0.5">
              <p>{{ $t('ui.lanSync.filesTransferred', { count: lastResult.filesDownloaded }) }}</p>
              <p>{{ $t('ui.lanSync.filesDeleted', { count: lastResult.filesDeleted }) }}</p>
              <p>{{ $t('ui.lanSync.bytesTransferred', { size: formatBytes(lastResult.bytesTransferred) }) }}</p>
            </div>
          </div>

          <!-- 结果：无 lastResult（计划阶段失败，如网络连接失败） -->
          <div v-if="view === 'result' && !lastResult" class="text-center py-4 space-y-3">
            <div class="w-14 h-14 rounded-full mx-auto flex items-center justify-center bg-red-500/20">
              <X class="w-7 h-7 text-red-400" />
            </div>
            <p class="text-red-400 font-bold text-lg">{{ $t('ui.lanSync.syncFailed') }}</p>
            <p class="text-white/50 text-xs">{{ errorMessage || $t('ui.lanSync.genericError') }}</p>
          </div>

          <!-- 通用错误（非 result 视图时显示） -->
          <div v-if="phase === 'error' && view !== 'result'" class="text-center py-6 space-y-2">
            <div class="w-14 h-14 rounded-full mx-auto flex items-center justify-center bg-red-500/20">
              <X class="w-7 h-7 text-red-400" />
            </div>
            <p class="text-red-400 font-bold">{{ $t('ui.lanSync.errorTitle') }}</p>
            <p class="text-white/50 text-xs">{{ errorMessage }}</p>
          </div>
        </div>

        <!-- ─── 按钮区 ─── -->
        <div class="p-5 pt-0 space-y-2 shrink-0">
          <button
            v-if="view === 'device-list'"
            @click="emit('close')"
            class="w-full py-3 rounded-full bg-white/10 hover:bg-white/15 text-white/60 hover:text-white/80 text-sm font-medium border border-white/10 transition-all"
          >
            {{ $t('ui.lanSync.close') }}
          </button>

          <template v-if="view === 'sync-plan'">
            <button
              @click="emit('confirm')"
              class="w-full py-3 rounded-full bg-cyan-500/80 hover:bg-cyan-500 text-white text-sm font-bold border border-cyan-400/50 shadow-lg shadow-cyan-500/20 transition-all active:scale-95"
            >
              {{ $t('ui.lanSync.confirmSync') }}
            </button>
            <button
              @click="emit('cancel')"
              class="w-full py-3 rounded-full text-white/40 hover:text-white/60 text-xs transition-colors"
            >
              {{ $t('ui.lanSync.cancel') }}
            </button>
          </template>

          <p v-if="view === 'progress'" class="text-center text-white/30 text-xs">
            {{ $t('ui.lanSync.syncingWait') }}
          </p>

          <!-- 结果：有暂存文件 → 重启按钮 -->
          <button
            v-if="view === 'result' && lastResult?.success && lastResult.filesStaged > 0"
            @click="emit('restart')"
            class="w-full py-3 rounded-full bg-emerald-500/80 hover:bg-emerald-500 text-white text-sm font-bold border border-emerald-400/50 shadow-lg shadow-emerald-500/20 transition-all active:scale-95"
          >
            {{ $t('ui.lanSync.restartToApply', { count: lastResult.filesStaged }) }}
          </button>

          <button
            v-if="view === 'result'"
            @click="emit('close')"
            class="w-full py-3 rounded-full bg-white/10 hover:bg-white/15 text-white/60 hover:text-white/80 text-sm font-medium border border-white/10 transition-all"
          >
            {{ $t('ui.lanSync.close') }}
          </button>

          <button
            v-if="phase === 'error' && view !== 'result'"
            @click="emit('close')"
            class="w-full py-3 rounded-full bg-red-500/20 hover:bg-red-500/30 text-red-400 text-sm font-medium border border-red-500/20 transition-all"
          >
            {{ $t('ui.lanSync.close') }}
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Icon } from './base'
import { Wifi, Check, X } from 'lucide-vue-next'
import type { PeerInfo, SyncPlan, SyncProgressEvent, SyncResult, SyncPhase, DialogView } from '../types/lanSync'

const props = defineProps<{
  visible: boolean
  view: DialogView
  phase: SyncPhase
  serverPort: number
  peers: PeerInfo[]
  syncPlan: SyncPlan | null
  progress: SyncProgressEvent
  lastResult: SyncResult | null
  errorMessage: string
}>()

const emit = defineEmits<{
  rescan: []
  pull: [peer: PeerInfo]
  push: [peer: PeerInfo]
  confirm: []
  cancel: []
  close: []
  restart: []
}>()

const { t } = useI18n()

const dialogTitle = computed(() => {
  switch (props.view) {
    case 'device-list': return t('ui.lanSync.titleDeviceList')
    case 'sync-plan': return t('ui.lanSync.titleSyncPlan')
    case 'progress': return t('ui.lanSync.titleProgress')
    case 'result': return props.lastResult?.success ? t('ui.lanSync.titleResultSuccess') : t('ui.lanSync.titleResultFailed')
    default: return t('ui.lanSync.titleDeviceList')
  }
})

const dialogSubtitle = computed(() => {
  switch (props.view) {
    case 'device-list': return t('ui.lanSync.subtitleDeviceList')
    case 'sync-plan': return t('ui.lanSync.subtitleSyncPlan')
    case 'progress': return t('ui.lanSync.subtitleProgress')
    case 'result': return props.lastResult?.success ? t('ui.lanSync.subtitleResultSuccess') : t('ui.lanSync.subtitleResultFailed')
    default: return ''
  }
})

const headerIconBg = computed(() => {
  switch (props.view) {
    case 'progress': return 'bg-gradient-to-br from-cyan-500 to-indigo-500'
    case 'result': return props.lastResult?.success
      ? 'bg-gradient-to-br from-emerald-500 to-teal-500'
      : 'bg-gradient-to-br from-red-500 to-rose-500'
    default: return 'bg-gradient-to-br from-indigo-500 to-purple-500'
  }
})

function formatBytes(bytes: number): string {
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB'
  return (bytes / (1024 * 1024 * 1024)).toFixed(2) + ' GB'
}

function reasonLabel(reason: string): string {
  switch (reason) {
    case 'new': return t('ui.lanSync.reasonNew')
    case 'modified': return t('ui.lanSync.reasonModified')
    case 'newer': return t('ui.lanSync.reasonNewer')
    default: return reason
  }
}
</script>

<style scoped>
.modal-enter-active,
.modal-leave-active {
  transition: all 0.4s cubic-bezier(0.16, 1, 0.3, 1);
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
  transform: scale(0.95) translateY(10px);
}

.overflow-y-auto::-webkit-scrollbar {
  display: none;
}
</style>
