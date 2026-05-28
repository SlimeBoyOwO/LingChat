<template>
  <div>
    <MenuPage>
      <MenuItem title="大模型管理">
        <template #header>
          <Icon icon="bot" :size="20" />
        </template>
        <div class="flex flex-col items-center gap-3 w-full">
          <p class="text-white/60 text-sm">管理 LLM 模型配置</p>
          <button
            class="w-full px-8 py-4 bg-white/10 backdrop-blur-xl border border-white/10 rounded-xl text-white text-base font-bold cursor-pointer hover:bg-white/20 hover:border-brand/50 active:scale-95 transition-all duration-200 inline-flex items-center gap-3 justify-center"
            @click="showLlmPanel = true"
          >
            <Icon icon="bot" :size="20" />
            打开大模型管理
          </button>
        </div>
      </MenuItem>

      <MenuItem title="高级设置">
        <template #header>
          <Icon icon="advance" :size="20" />
        </template>
        <div class="flex flex-col items-center gap-3 w-full">
          <p class="text-white/60 text-sm">点击下方按钮进入高级设置面板</p>
          <button
            class="w-full px-8 py-4 bg-white/10 backdrop-blur-xl border border-white/10 rounded-xl text-white text-base font-bold cursor-pointer hover:bg-white/20 hover:border-brand/50 active:scale-95 transition-all duration-200 inline-flex items-center gap-3 justify-center"
            @click="showAdvancePanel = true"
          >
            <Icon icon="advance" :size="20" />
            打开高级设置
          </button>
        </div>
      </MenuItem>
    </MenuPage>

  <!-- 滑入面板 -->
  <Teleport to="body">
    <Transition name="advance-slide">
      <div v-if="showAdvancePanel" class="advance-slide-wrapper">
        <!-- 模糊遮罩 — 使用与 SettingsPanel 一致的 blur 效果 -->
        <div class="advance-slide-overlay" @click="closePanel"></div>

        <!-- 面板容器：透明底，内容自带玻璃效果 -->
        <div class="advance-slide-panel">
          <div class="advance-slide-header">
            <button class="advance-back-btn" @click="closePanel">
              <Icon icon="close" :size="28" />
            </button>
            <div class="advance-header-title">
              <Icon icon="advance" :size="20" />
              <span>高级设置</span>
            </div>
          </div>

          <div class="advance-slide-content">
            <SettingsAdvance
              ref="settingsAdvanceRef"
              @remove-more-menu-from-b="onRemoveFromB"
            />
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>

  <!-- LLM 配置面板 -->
  <SettingsLlmConfig v-if="showLlmPanel" @close="showLlmPanel = false" />
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { MenuPage, MenuItem } from '../../ui'
import Icon from '../../base/widget/Icon.vue'
import SettingsAdvance from './SettingsAdvance.vue'
import SettingsLlmConfig from './SettingsLlmConfig.vue'

const showAdvancePanel = ref(false)
const showLlmPanel = ref(false)
const settingsAdvanceRef = ref<InstanceType<typeof SettingsAdvance> | null>(null)

const emit = defineEmits(['remove-more-menu-from-b'])

const closePanel = () => {
  showAdvancePanel.value = false
}

const onRemoveFromB = () => {
  emit('remove-more-menu-from-b')
}

const addMoreMenu = () => {
  settingsAdvanceRef.value?.addMoreMenu()
}

defineExpose({ addMoreMenu })
</script>

<style>
.advance-slide-wrapper {
  position: fixed;
  inset: 0;
  z-index: 1100;
}

/* 模糊遮罩 — 与 SettingsPanel 的 .blur-overlay 一致 */
.advance-slide-overlay {
  position: absolute;
  inset: 0;
  backdrop-filter: blur(5px);
  background-color: rgba(0, 0, 0, 0.45);
}

/* 面板本身透明，让遮罩的模糊效果透过来 */
.advance-slide-panel {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: transparent;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  transition: transform 0.3s cubic-bezier(0.18, 0.89, 0.32, 1);
}

.advance-slide-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 20px;
  flex-shrink: 0;
}

.advance-back-btn {
  background: transparent;
  border: none;
  color: white;
  cursor: pointer;
  padding: 6px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.3s ease;
}

.advance-back-btn:hover {
  background: rgba(255, 255, 255, 0.1);
  color: var(--accent-color);
  transform: rotate(90deg);
}

.advance-header-title {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--accent-color);
  font-weight: bold;
  font-size: 16px;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
}

.advance-slide-content {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  display: flex;
  min-height: 0;
}

/* 从左侧滑入 + 遮罩淡入 */
.advance-slide-enter-active,
.advance-slide-leave-active {
  transition: opacity 0.3s ease;
}

.advance-slide-enter-from,
.advance-slide-leave-to {
  opacity: 0;
}

.advance-slide-enter-from .advance-slide-panel {
  transform: translateX(-100%);
}

.advance-slide-leave-to .advance-slide-panel {
  transform: translateX(-100%);
}
</style>
