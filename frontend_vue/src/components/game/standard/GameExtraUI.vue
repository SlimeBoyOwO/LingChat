<template>
  <!-- 整个 UI 层的根容器 -->
  <div class="fixed top-0 left-0 w-full h-full pointer-events-none z-[999]">
    <!-- 1. 章节名称显示区域 (原有功能) -->
    <div class="w-full pt-8">
      <transition
        @before-enter="beforeEnter"
        @enter="enter"
        @after-enter="afterEnter"
        @before-leave="beforeLeave"
        @leave="leave"
        @after-leave="afterLeave"
      >
        <div
          v-if="shouldShowChapterName"
          class="relative drop-shadow-[0_8px_12px_rgba(0,0,0,0.25)] w-[20%]"
        >
          <div
            class="relative flex items-center pl-8 pr-14 py-3 bg-slate-900/40 backdrop-blur-md overflow-hidden"
            style="
              clip-path: polygon(
                0 0,
                calc(100% - 24px) 0,
                100% 50%,
                calc(100% - 24px) 100%,
                0 100%
              );
            "
          >
            <div class="absolute left-0 top-0 bottom-0 w-1.5 bg-brand rounded-2xl"></div>
            <Columns2 class="text-white mr-4" />
            <div
              class="text-xl font-bold text-gray-300 tracking-widest drop-shadow-[0_2px_4px_rgba(0,0,0,0.8)]"
            >
              {{ displayChapterName }}
            </div>
          </div>
        </div>
      </transition>
    </div>

    <!-- 2. Galgame 选项显示区域 (已修复动画) -->
    <!-- 移除 v-if，父容器常驻 DOM，加上 pointer-events-none 防止空状态阻挡底层点击 -->
    <div
      class="fixed inset-0 flex flex-col items-center justify-center -mt-[15vh] pointer-events-none"
    >
      <!-- 添加 appear 开启首屏动画，开启 :css="false" 彻底接管 JS 动画 -->
      <transition-group
        appear
        :css="false"
        tag="div"
        class="flex flex-col gap-10 w-full max-w-2xl px-4 pointer-events-auto"
        @before-enter="choiceBeforeEnter"
        @enter="choiceEnter"
        @leave="choiceLeave"
      >
        <!-- 数据源改为 displayChoices，key 改为 choice 内容 -->
        <button
          v-for="(choice, index) in displayChoices"
          :key="choice"
          :data-index="index"
          @click="selectChoice(choice)"
          class="group relative w-full py-4 px-8 border rounded-full text-sm text-white bg-slate-900/40 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass hover:outline-none hover:border-brand hover:ring-2 hover:ring-brand/20 shadow-[0_4px_12px_rgba(0,0,0,0.3)] hover:shadow-[0_0_15px_rgba(0,0,0,0.5)] transform hover:-translate-y-1 transition-all duration-200"
        >
          <!-- 粒子效果 - 静态粒子 (小圆点) -->
          <div
            class="absolute inset-0 opacity-30 group-hover:opacity-50 transition-opacity duration-700"
          >
            <!-- 左上区域粒子 -->
            <div class="absolute top-2 left-4 w-1 h-1 rounded-full bg-white/60"></div>
            <div class="absolute top-6 left-8 w-0.5 h-0.5 rounded-full bg-blue-300/50"></div>
            <div
              class="absolute top-4 left-16 w-1.5 h-1.5 rounded-full bg-brand/40 blur-[1px]"
            ></div>

            <!-- 右上区域粒子 -->
            <div class="absolute top-3 right-6 w-1 h-1 rounded-full bg-white/40"></div>
            <div class="absolute top-8 right-12 w-0.5 h-0.5 rounded-full bg-purple-300/50"></div>
            <div class="absolute top-5 right-20 w-1 h-1 rounded-full bg-brand/30 blur-[1px]"></div>

            <!-- 中部区域粒子 -->
            <div class="absolute top-1/2 left-10 w-0.5 h-0.5 rounded-full bg-cyan-300/40"></div>
            <div class="absolute top-1/2 right-12 w-1 h-1 rounded-full bg-white/30"></div>

            <!-- 底部区域粒子 -->
            <div class="absolute bottom-4 left-8 w-1 h-1 rounded-full bg-brand/30"></div>
            <div class="absolute bottom-8 right-10 w-0.5 h-0.5 rounded-full bg-blue-300/40"></div>
            <div
              class="absolute bottom-3 right-16 w-1.5 h-1.5 rounded-full bg-white/20 blur-[1px]"
            ></div>
          </div>

          <!-- 动态漂浮粒子 (悬停时动画) -->
          <div
            class="absolute inset-0 opacity-0 group-hover:opacity-40 transition-opacity duration-500"
          >
            <!-- 漂浮粒子1 -->
            <div
              class="absolute top-2 left-4 w-1 h-1 rounded-full bg-white/60 animate-float-slow"
            ></div>
            <!-- 漂浮粒子2 -->
            <div
              class="absolute bottom-6 right-8 w-0.5 h-0.5 rounded-full bg-brand/60 animate-float"
            ></div>
            <!-- 漂浮粒子3 -->
            <div
              class="absolute top-8 right-12 w-1 h-1 rounded-full bg-purple-400/50 animate-float-reverse"
            ></div>
            <!-- 漂浮粒子4 -->
            <div
              class="absolute bottom-10 left-12 w-1.5 h-1.5 rounded-full bg-cyan-300/40 blur-[1px] animate-float-slow"
            ></div>
            <!-- 漂浮粒子5 -->
            <div
              class="absolute top-1/3 right-20 w-1 h-1 rounded-full bg-white/40 animate-float"
            ></div>
          </div>

          <!-- 微光扫射效果 (悬停时) -->
          <div
            class="absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-700 overflow-hidden rounded-full"
          >
            <div
              class="absolute -inset-full top-0 h-full w-1/2 z-5 block transform -skew-x-12 bg-gradient-to-r from-transparent via-white/10 to-transparent animate-shine"
            ></div>
          </div>
          <span
            class="text-lg font-medium text-white group-hover:text-white tracking-widest text-center block drop-shadow-[0_2px_4px_rgba(0,0,0,0.8)]"
          >
            {{ choice }}
          </span>
        </button>
      </transition-group>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { Columns2 } from 'lucide-vue-next'
import { scriptHandler } from '../../../api/websocket/handlers/script-handler'

const gameStore = useGameStore()
const uiStore = useUIStore()

// --- 章节名称相关状态 ---
const displayChapterName = ref('')
const showChapterName = ref(false)
const isTransitioning = ref(false)

const shouldShowChapterName = computed(() => {
  return uiStore.showSettings ? false : showChapterName.value
})

// --- 选项相关状态 (已修复) ---
// 使用计算属性控制数组：如果打开设置，则喂给 transition-group 一个空数组以触发 leave 动画
const displayChoices = computed(() => {
  if (uiStore.showSettings) return []
  return gameStore.runningScript?.choices || []
})

// 处理玩家选择
function selectChoice(choice: string) {
  // 1. 触发游戏引擎选择分支的逻辑
  gameStore.appendGameMessage({
    type: 'message',
    displayName: gameStore.userName,
    content: choice,
  })
  scriptHandler.sendMessage(choice)

  // 2. 清空选项，displayChoices 响应式更新为空，完美触发交错渐隐动画
  if (gameStore.runningScript) {
    gameStore.runningScript.choices = []
  }
}

// --- 章节名称监听器 ---
watch(
  () => gameStore.runningScript?.currentChapterName,
  async (newName, oldName) => {
    if (isTransitioning.value) return
    if (newName === oldName) return

    if (!oldName && newName) {
      displayChapterName.value = newName
      showChapterName.value = true
    } else if (oldName && !newName) {
      showChapterName.value = false
    } else if (oldName && newName && oldName !== newName) {
      isTransitioning.value = true
      showChapterName.value = false

      setTimeout(() => {
        displayChapterName.value = newName
        showChapterName.value = true
        isTransitioning.value = false
      }, 500)
    }
  },
)

// --- 章节名称动画 ---
function beforeEnter(el: Element) {
  const element = el as HTMLElement
  element.style.opacity = '0'
  element.style.transform = 'translateX(-20px)'
  element.style.transition = 'all 0.5s cubic-bezier(0.4, 0, 0.2, 1)'
}

function enter(el: Element, done: () => void) {
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      const element = el as HTMLElement
      element.style.opacity = '1'
      element.style.transform = 'translateX(0)'
      setTimeout(done, 500)
    })
  })
}

function afterEnter(el: Element) {
  const element = el as HTMLElement
  element.style.opacity = '1'
  element.style.transform = 'translateX(0)'
}

function beforeLeave(el: Element) {
  const element = el as HTMLElement
  element.style.opacity = '1'
  element.style.transform = 'translateX(0)'
  element.style.transition = 'all 0.5s cubic-bezier(0.4, 0, 0.2, 1)'
}

function leave(el: Element, done: () => void) {
  const element = el as HTMLElement
  element.style.opacity = '0'
  element.style.transform = 'translateX(20px)'
  setTimeout(done, 500)
}

function afterLeave(el: Element) {
  const element = el as HTMLElement
  element.style.opacity = '0'
}

// --- 选项列表动画 (已修复) ---
// 选项进入前的状态：透明且靠下
function choiceBeforeEnter(el: Element) {
  const element = el as HTMLElement
  element.style.opacity = '0'
  element.style.transform = 'translateY(30px)'
  element.style.transition = 'all 0.5s cubic-bezier(0.34, 1.56, 0.64, 1)' // 使用回弹缓动曲线
}

// 选项进入动画：加入 requestAnimationFrame 强制渲染初始状态，随后通过 setTimeout 加入交错延迟
function choiceEnter(el: Element, done: () => void) {
  const element = el as HTMLElement
  const index = parseInt(element.dataset.index || '0')

  requestAnimationFrame(() => {
    setTimeout(() => {
      element.style.opacity = '1'
      element.style.transform = 'translateY(0)'
      setTimeout(done, 500) // 等待动画完成释放生命周期
    }, index * 100) // 每个选项延迟 100ms
  })
}

// 选项离开动画：点击后稍微缩小并渐隐消失
function choiceLeave(el: Element, done: () => void) {
  const element = el as HTMLElement
  element.style.transition = 'all 0.3s cubic-bezier(0.4, 0, 0.2, 1)'
  element.style.opacity = '0'
  element.style.transform = 'scale(0.95)'
  setTimeout(done, 300)
}
</script>

<style scoped>
@keyframes float {
  0%,
  100% {
    transform: translateY(0) translateX(0);
  }
  25% {
    transform: translateY(-4px) translateX(2px);
  }
  50% {
    transform: translateY(0) translateX(4px);
  }
  75% {
    transform: translateY(4px) translateX(0);
  }
}

@keyframes float-slow {
  0%,
  100% {
    transform: translateY(0) translateX(0);
  }
  33% {
    transform: translateY(-3px) translateX(-2px);
  }
  66% {
    transform: translateY(2px) translateX(3px);
  }
}

@keyframes float-reverse {
  0%,
  100% {
    transform: translateY(0) translateX(0);
  }
  33% {
    transform: translateY(3px) translateX(-3px);
  }
  66% {
    transform: translateY(-2px) translateX(2px);
  }
}

@keyframes shine {
  100% {
    left: 200%;
  }
}

.animate-float {
  animation: float 6s ease-in-out infinite;
}

.animate-float-slow {
  animation: float-slow 8s ease-in-out infinite;
}

.animate-float-reverse {
  animation: float-reverse 7s ease-in-out infinite;
}

.animate-shine {
  animation: shine 3s ease-in-out infinite;
}

/* 为粒子添加随机延迟 */
.animate-float:nth-child(1) {
  animation-delay: 0s;
}
.animate-float:nth-child(2) {
  animation-delay: 1.2s;
}
.animate-float:nth-child(3) {
  animation-delay: 2.4s;
}
.animate-float:nth-child(4) {
  animation-delay: 0.8s;
}
.animate-float:nth-child(5) {
  animation-delay: 1.8s;
}
</style>
