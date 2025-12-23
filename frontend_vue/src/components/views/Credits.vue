<template>
  <div class="credits-page-wrapper">
    <audio ref="bgm"></audio>
    <div v-if="!isStarted" class="start-overlay" @click="startCredits">
      <div class="start-prompt">
        <p>来自灵灵感谢の书❤</p>
        <span>A Letter For LingChat❤</span>
      </div>
    </div>

    <div v-if="isStarted" class="credits-container" ref="creditsContainer">
      <div class="credits-scroll">
        <StarField ref="starfieldRef" />
        <div class="title">
          <img src="@/assets/images/LingChatLogo.png" class="logo" />
          <h1>致谢</h1>
          <p>CREDITS</p>
        </div>

        <div class="section">
          <h2>策划 & 设计</h2>
          <p class="role">Project Lead & Designer</p>
          <div class="character">
            <p class="name">钦灵</p>
            <p class="en-name">NoiQing Ling</p>
          </div>

          <div class="character">
            <p class="name">风雪</p>
            <p class="en-name">Snow Wind</p>
          </div>
        </div>

        <div class="section">
          <h2>程序开发</h2>
          <p class="role">Programming</p>
          <div class="character">
            <p class="name">有梦当燃</p>
            <p class="en-name">Cute RBQ</p>
          </div>
          <div class="character">
            <p class="name">元初</p>
            <p class="en-name">Kawaii Femboy</p>
          </div>
          <div class="character">
            <p class="name">维克多多</p>
            <p class="en-name">Vickko Sama</p>
          </div>
          <div class="character">
            <p class="name">喵达子大人</p>
            <p class="en-name">Cat Sama</p>
          </div>
          <div class="character">
            <p class="name">PL</p>
            <p class="en-name">Magic Witch</p>
          </div>
          <div class="character">
            <p class="name">影空</p>
            <p class="en-name">Shadow Sky</p>
          </div>
          <div class="character">
            <p class="name">随时跑路</p>
            <p class="en-name">Fucked Anytime</p>
          </div>
          <div class="character">
            <p class="name">云小姐</p>
            <p class="en-name">Cloud Sister</p>
          </div>
          <div class="character">
            <p class="name">远足</p>
            <p class="en-name">Femboy Two</p>
          </div>
        </div>

        <div class="section">
          <h2>视觉艺术</h2>
          <p class="role">Visual Arts</p>
          <div class="character">
            <p class="name">Yukito</p>
            <p class="en-name">QAQ</p>
          </div>
          <div class="character">
            <p class="name">柏海</p>
            <p class="en-name">Wood & Sea</p>
          </div>
          <div class="character">
            <p class="name">梦轩</p>
            <p class="en-name">Dream Line</p>
          </div>
        </div>

        <div class="section">
          <h2>人物设计</h2>
          <p class="role">Character Desgin</p>
          <div class="character">
            <p class="name">徒花</p>
            <p class="en-name">Flowing Flower</p>
          </div>
          <div class="character">
            <p class="name">总督</p>
            <p class="en-name">Great Commander</p>
          </div>
        </div>

        <div class="section">
          <h2>社区管理 & 维基搭建</h2>
          <p class="role">Community Management & Wiki Constructing</p>
          <div class="character">
            <p class="name">七辰</p>
            <p class="en-name">Horny Whenever</p>
          </div>
          <div class="character">
            <p class="name">雅诺狐</p>
            <p class="en-name">Yano Fox</p>
          </div>
        </div>

        <div class="section">
          <h2>软件打包</h2>
          <p class="role">Software Packing</p>
          <div class="character">
            <p class="name">uwa uwa</p>
            <p class="en-name">Loli Loli</p>
          </div>
        </div>

        <div class="section">
          <h2>特别鸣谢</h2>
          <p class="role">Special Thanks</p>

          <div class="character-list">
            <div class="character">
              <p class="name">Thz</p>
              <p class="en-name">Sister M</p>
            </div>
            <div class="character">
              <p class="name">琉璃子</p>
              <p class="en-name">Ruriko</p>
            </div>
            <div class="character">
              <p class="name">安静</p>
              <p class="en-name">Quiet Kitty</p>
            </div>
            <div class="character">
              <p class="name">莱尔</p>
              <p class="en-name">Layeray</p>
            </div>
            <div class="character">
              <p class="name">冰花</p>
              <p class="en-name">Femboy Flower</p>
            </div>
            <div class="character">
              <p class="name">DaDa</p>
              <p class="en-name">DaDa</p>
            </div>
            <div class="character">
              <p class="name">波奶</p>
              <p class="en-name">Super Kawaii</p>
            </div>
            <div class="character">
              <p class="name">插歪</p>
              <p class="en-name">X Y</p>
            </div>
            <div class="character">
              <p class="name">これも無い</p>
              <p class="en-name">Empty</p>
            </div>
            <div class="character">
              <p class="name">阳光</p>
              <p class="en-name">Sunshine</p>
            </div>
            <div class="character">
              <p class="name">40冷却器</p>
              <p class="en-name">40 Freezer</p>
            </div>
            <div class="character">
              <p class="name">影</p>
              <p class="en-name">Darkness</p>
            </div>
          </div>
          <div class="section"></div>
          <div class="section"></div>
          <p class="name">Issue提供者</p>
          <p class="en-name">Issue Providers</p>
          <div class="section"></div>
          <p class="name">创意工坊作者们</p>
          <p class="en-name">Character Creators</p>
          <div class="section"></div>
          <p class="name">B站粉丝们</p>
          <p class="en-name">Bilibili Subscribers</p>
          <div class="section"></div>
          <p class="name">赞助者们</p>
          <p class="en-name">Donators</p>
          <div class="section"></div>
        </div>

        <div class="section"></div>
        <div class="section"></div>
        <div class="section"></div>
        <div class="section">
          <h2>还有...</h2>
          <p class="role">Moreover...</p>
          <div class="section"></div>
          <div class="section"></div>
          <div class="section"></div>
          <div class="section"></div>
          <div class="section"></div>
          <p class="name">你</p>
          <p class="en-name">You</p>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import StarField from '../game/standard/particles/StarField.vue'

// 新增状态：控制是否已开始播放致谢名单
const isStarted = ref(false)

const isFinalMessageVisible = ref(false) // 这个变量在你的原代码中存在，但模板中未使用，我保留了它
const bgm = ref<HTMLAudioElement | null>(null) // 明确 audio 元素的类型

let timer: NodeJS.Timeout | null = null

// 点击开始界面的处理函数
const startCredits = () => {
  if (!bgm.value) return // 防御性编程，确保 bgm 存在

  // 1. 设置状态为“已开始”，这将触发 v-if 切换，显示致谢名单
  isStarted.value = true

  // 2. 播放音频
  // 因为这是在用户点击事件后触发的，所以浏览器不会阻止
  bgm.value.src = '/audio/credit.mp3'
  bgm.value.load()
  bgm.value.play().catch((error) => {
    console.error('音频播放失败:', error)
  })

  // 3. 将设置最终信息的定时器也移到这里
  // 这样可以确保动画从用户点击后才开始计时
  const animationDuration = 41000 // 41秒
  timer = setTimeout(() => {
    isFinalMessageVisible.value = true
  }, animationDuration)
}

// onMounted 现在可以留空，因为所有逻辑都移到了 startCredits 函数中
onMounted(() => {
  // 可以在这里做一些与DOM相关的初始化，如果需要的话
})

// onBeforeUnmount 保持不变，用于清除定时器
onBeforeUnmount(() => {
  if (timer) {
    clearTimeout(timer)
  }
})
</script>

<style scoped>
/* 引入外部字体 */
@import url('https://fonts.googleapis.com/css2?family=Noto+Sans+SC:wght@300;400&display=swap');

/* 组件根元素，确保其撑满父容器并作为定位上下文 */
.credits-page-wrapper {
  position: relative;
  width: 100%;
  height: 100%; /* 默认占满整个视口高度 */
  overflow: hidden;
  background-color: #0a0a0c;
  color: #ffffff;
  font-family: 'Noto Sans SC', sans-serif;
  display: flex;
  justify-content: center;
  align-items: center;
}

/* 致谢名单容器 */
.credits-container {
  width: 100%;
  height: 100%;
  position: relative;
  animation: fadeIn 2s ease-out;
  /* 使用 mask 实现上下淡入淡出效果 */
  -webkit-mask-image: linear-gradient(
    to bottom,
    transparent 0%,
    black 20%,
    black 80%,
    transparent 100%
  );
  mask-image: linear-gradient(to bottom, transparent 0%, black 20%, black 80%, transparent 100%);
}

/* 滚动内容的核心 */
.credits-scroll {
  position: absolute;
  top: 100%;
  width: 100%;
  text-align: center;
  /* 动画：名称、时长、速度曲线、延迟、循环次数、方向 */
  animation: scrollAnimation 120s linear 2s forwards;
}

/* 标题样式 */
.title {
  margin-bottom: 80px;
}

.title h1 {
  font-size: 3.5em;
  color: #00e5ff;
  font-weight: 400;
  letter-spacing: 5px;
}

.title p {
  font-size: 1.2em;
  color: #ebfafb;
  opacity: 0.7;
  letter-spacing: 8px;
  text-transform: uppercase;
}

.logo {
  width: 50%;
  height: 50%;
}

/* 部门/分组样式 */
.section {
  flex-direction: column;
  margin-bottom: 60px;
  display: flex;
  align-items: center;
}

.section h2 {
  font-size: 2.2em;
  color: #00e5ff;
  font-weight: 300;
  margin-bottom: 8px;
}

.section .role {
  font-size: 1em;
  color: #ffffff;
  opacity: 0.6;
  margin-bottom: 25px;
}

.section .character {
  display: grid;
  grid-template-columns: 1fr 1fr; /* 两列等宽 */
  justify-items: center;
  align-items: center;
  width: 27%;
  min-width: 280px;
}

.section .character-list {
  display: grid;
  grid-template-columns: 1fr 1fr; /* 两列等宽 */
  justify-items: center;
  align-items: center;
  width: 60%;
}

.section .name {
  font-size: 1.5em;
  color: #ffffff;
  line-height: 1.8;
  font-weight: 300;
}

.section .en-name {
  font-size: 1em;
  color: #ffffff;
  opacity: 0.8;
  line-height: 1.8;
  font-weight: 300;
}

.section .job {
  font-size: 1.2em;
  color: var(--accent-color);
  opacity: 0.8;
  line-height: 1.8;
  font-weight: 300;
}

/* 定义滚动的动画 */
@keyframes scrollAnimation {
  from {
    top: 100%;
  }
  to {
    top: -536%;
  }
}

.start-overlay {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background-color: #0a0a0c; /* 与背景色一致 */
  display: flex;
  justify-content: center;
  align-items: center;
  cursor: pointer;
  z-index: 10; /* 确保在顶层 */
}

.start-prompt {
  text-align: center;
  color: #fff;
  /* 添加一个呼吸动画，吸引用户点击 */
  animation: pulse 3s infinite;
}

.start-prompt p {
  font-size: 2.2em;
  font-family: 'Noto Sans SC', sans-serif;
  font-weight: 300;
  letter-spacing: 2px;
}

.start-prompt span {
  font-size: 1.1em;
  opacity: 0.7;
  letter-spacing: 4px;
  text-transform: uppercase;
}

/* 定义淡入动画 */
@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

/* 定义呼吸动画 */
@keyframes pulse {
  0% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.05);
  }
  100% {
    transform: scale(1);
  }
}
</style>
