<template>
  <div class="w-full h-full flex items-center justify-between relative px-5 py-2">
    <img src="@/assets/images/LingChatLogo.png" alt="Logo" class="w-20 ml-5 hidden xl:block" />
    <nav
      ref="navContainer"
      class="relative w-full flex felx-row items-center justify-around h-full ease-in-out duration-100 xl:justify-center overflow-x-auto overflow-y-hidden"
    >
      <div
        ref="indicator"
        class="absolute bottom-0 left-0 w-0 h-1 bg-brand rounded z-10 shadow-[0_0_10px_rgba(121,217,255,0.4)]"
      ></div>
      <Button
        ref="characterBtn"
        type="nav"
        icon="character"
        @click="() => switchTab('character', 'characterBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'character' }"
        ><p class="hidden xl:block">角色</p></Button
      >
      <Button
        ref="clothesBtn"
        type="nav"
        icon="clothes"
        @click="() => switchTab('clothes', 'clothesBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'clothes' }"
        ><p class="hidden xl:block">换衣</p></Button
      >
      <Button
        ref="textBtn"
        type="nav"
        icon="text"
        @click="() => switchTab('text', 'textBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'text' }"
        ><p class="hidden xl:block">通用</p></Button
      >
      <Button
        ref="backgroundBtn"
        type="nav"
        icon="background"
        @click="() => switchTab('background', 'backgroundBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'background' }"
        ><p class="hidden xl:block">背景</p></Button
      >
      <Button
        ref="soundBtn"
        type="nav"
        icon="sound"
        @click="() => switchTab('sound', 'soundBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'sound' }"
        ><p class="hidden xl:block">声音</p></Button
      >
      <Button
        ref="historyBtn"
        type="nav"
        icon="history"
        @click="() => switchTab('history', 'historyBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'history' }"
        ><p class="hidden xl:block">对话历史</p></Button
      >
      <Button
        ref="saveBtn"
        type="nav"
        icon="save"
        @click="() => switchTab('save', 'saveBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'save' }"
        ><p class="hidden xl:block">存档</p></Button
      >
      <Button
        ref="scheduleBtn"
        type="nav"
        icon="schedule"
        @click="() => switchTab('schedule', 'scheduleBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'schedule' }"
        ><p class="hidden xl:block">日程</p></Button
      >
      <Button
        ref="advanceBtn"
        type="nav"
        icon="advance"
        @click="
          () => {
            switchTab('advance', 'advanceBtn')
            removeMoreMenu()
          }
        "
        :class="{ active: uiStore.currentSettingsTab === 'advance' }"
        ><p class="hidden xl:block">高级设置</p></Button
      >
      <Button
        ref="updateBtn"
        type="nav"
        icon="update"
        @click="() => switchTab('update', 'updateBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'update' }"
        ><p class="hidden xl:block">检查更新</p></Button
      >
    </nav>
    <Icon
      icon="close"
      class="bg-transparent border-none p-1.5 rounded-full cursor-pointer flex items-center justify-center text-white transition-all duration-300 ease-in-out hover:text-accent hover:bg-white/10 hover:rotate-90"
      :size="40"
      @click="closeSettings"
    ></Icon>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { useUIStore } from '../../stores/modules/ui/ui'
import { Button } from '../base'
import Icon from '../base/widget/Icon.vue'

const props = defineProps<{}>()

const emit = defineEmits([
  'remove-more-menu-from-a', // A 组件触发 remove 时通知父组件
])

const uiStore = useUIStore()
const indicator = ref<HTMLElement | null>(null)
const navContainer = ref<HTMLElement | null>(null)

// 定义按钮ref的类型
type ButtonRef = InstanceType<typeof Button>

// 使用更宽松的类型定义
const characterBtn = ref<ButtonRef | null>(null)
const textBtn = ref<ButtonRef | null>(null)
const backgroundBtn = ref<ButtonRef | null>(null)
const soundBtn = ref<ButtonRef | null>(null)
const historyBtn = ref<ButtonRef | null>(null)
const saveBtn = ref<ButtonRef | null>(null)
const advanceBtn = ref<ButtonRef | null>(null)
const scheduleBtn = ref<ButtonRef | null>(null)
const updateBtn = ref<ButtonRef | null>(null)
const clothesBtn = ref<ButtonRef | null>(null)

// 设置可重设的值（使用 ref 存储，确保响应式或跨函数访问）
const oldRefName = ref('textBtn')

// 提取：根据 refName 获取按钮并移动指示器
const handleIndicatorMove = (currentRefName: string) => {
  const buttonRef = {
    characterBtn,
    textBtn,
    backgroundBtn,
    soundBtn,
    historyBtn,
    saveBtn,
    advanceBtn,
    scheduleBtn,
    updateBtn,
    clothesBtn,
  }[currentRefName]

  if (buttonRef?.value?.$el) {
    moveIndicator(buttonRef.value.$el)
  }
}

// 移动指示器
const moveIndicator = (target: HTMLElement) => {
  if (!indicator.value || !target) return

  indicator.value.style.left = `${target.offsetLeft}px`
  indicator.value.style.width = `${target.offsetWidth}px`
}

// 统一处理标签切换
const switchTab = (tabName: string, refName: string) => {
  // 记录当前 refName 到 oldRefName
  oldRefName.value = refName
  uiStore.setSettingsTab(tabName)

  if (!indicator.value) return

  // 1. 设置过渡动画
  indicator.value.style.transition =
    'left 0.3s cubic-bezier(0.18, 0.89, 0.32, 1), width 0.3s cubic-bezier(0.18, 0.89, 0.32, 1)'

  // 2. 触发动画（移动指示器）
  handleIndicatorMove(refName)

  // 3. 使用 setTimeout 延迟执行 unset
  //    延迟时间设置为 400ms，略长于动画时长 300ms，确保动画完全结束
  setTimeout(() => {
    if (indicator.value) {
      // 再次检查 indicator 是否存在，避免组件卸载后报错
      indicator.value.style.transition = 'unset'
    }
  }, 400) // 延迟 400 毫秒
}

// 屏幕宽度变化监测器
const setupResizeObserver = () => {
  if (!navContainer.value) {
    return
  }
  const resizeObserver = new ResizeObserver((entries) => {
    // 宽度变化时，从 oldRefName 提取 refName 并执行逻辑
    if (oldRefName.value) {
      handleIndicatorMove(oldRefName.value)
    }
  })

  // 监听nav的大小变化
  resizeObserver.observe(navContainer.value)
}

// 初始化指示器位置
const initIndicator = () => {
  const activeTab = uiStore.currentSettingsTab
  let activeButton = null

  switch (activeTab) {
    case 'character':
      activeButton = characterBtn.value
      break
    case 'text':
      activeButton = textBtn.value
      break
    case 'background':
      activeButton = backgroundBtn.value
      break
    case 'sound':
      activeButton = soundBtn.value
      break
    case 'history':
      activeButton = historyBtn.value
      break
    case 'save':
      activeButton = saveBtn.value
      break
    case 'advance':
      activeButton = advanceBtn.value
      break
    case 'schedule':
      activeButton = scheduleBtn.value
      break
    case 'update':
      activeButton = updateBtn.value
      break
    case 'clothes':
      activeButton = clothesBtn.value
      break
  }

  if (activeButton?.$el) {
    moveIndicator(activeButton.$el)
  }
}

// 组件挂载后初始化指示器
onMounted(() => {
  initIndicator()
  setupResizeObserver()
})

// 监听当前标签变化
watch(
  () => uiStore.currentSettingsTab,
  () => {
    initIndicator()
  },
)

const closeSettings = () => {
  uiStore.toggleSettings(false)
  // 将 refName 修改为默认的 "textBtn"
  oldRefName.value = 'textBtn'
}

const addMoreMenu = () => {
  const btnEl = advanceBtn.value?.$el as HTMLElement | null
  if (btnEl) {
    // console.log('A 组件内部执行 addMoreMenu');
    btnEl.classList.add('moreMenu')
  }
}

// 2. 定义 removeMoreMenu 方法
const removeMoreMenu = () => {
  const btnEl = advanceBtn.value?.$el as HTMLElement | null
  if (btnEl) {
    btnEl.classList.remove('moreMenu')
  }

  // 向父组件发送事件，告知“我这边已经执行了 remove”
  emit('remove-more-menu-from-a')
}

// 3. 关键：将 removeMoreMenu 方法暴露出去，这样父组件才能调用
defineExpose({
  addMoreMenu,
})

// 监听父组件转发的 B 组件事件（触发 A 组件自身逻辑）
// 监听 B 组件的 add 事件，触发 A 组件的 addMoreMenu
watch(
  () => {
    /* 可通过 props 传递状态，或直接监听 emit 事件 */
  },
  () => {},
  { immediate: true },
)
</script>
