<template>
  <!-- 根节点：填充满屏、锁定溢出、背景黑、支持文字字体 -->
  <div
    class="relative w-full h-full overflow-hidden bg-[#0a0a0c] text-white font-['Noto_Sans_SC',_sans-serif] flex justify-center items-center"
  >
    <!-- 音频播放器，监听 ended 事件进行路由跳转 -->
    <audio ref="bgm" @ended="onAudioEnded"></audio>

    <!-- 开始界面 -->
    <div
      v-if="!isStarted"
      class="absolute inset-0 bg-[#0a0a0c] flex justify-center items-center cursor-pointer z-10"
      @click="startCredits"
    >
      <div class="text-center text-white animate-[pulse_3s_infinite]">
        <p class="text-[2.2em] font-light tracking-[2px] mb-2">来自灵灵感谢の书❤</p>
        <span class="text-[1.1em] opacity-70 tracking-[4px] uppercase"
          >A Letter For LingChat❤</span
        >
      </div>
    </div>

    <!-- 滚动致谢名单容器 -->
    <div
      v-show="isStarted"
      class="w-full h-full relative"
      style="
        -webkit-mask-image: linear-gradient(
          to bottom,
          transparent 0%,
          black 20%,
          black 80%,
          transparent 100%
        );
        mask-image: linear-gradient(
          to bottom,
          transparent 0%,
          black 20%,
          black 80%,
          transparent 100%
        );
      "
    >
      <div
        class="credits-scroll absolute top-0 left-0 w-full text-center flex flex-col items-center"
        :class="[!isStarted ? 'translate-y-[100vh]' : '']"
      >
        <StarField ref="starfieldRef" />

        <!-- Logo 与 标题 -->
        <div class="mb-[80px] w-full flex flex-col items-center">
          <img
            src="@/assets/images/LingChatLogo.png"
            class="w-1/2 max-w-[400px] object-contain mx-auto mb-6"
            alt="Logo"
          />
          <h1 class="text-[3.5em] text-[#00e5ff] font-normal tracking-[5px]">致谢</h1>
          <p class="text-[1.2em] text-[#ebfafb] opacity-70 tracking-[8px] uppercase mt-2">
            CREDITS
          </p>
        </div>

        <!-- 数据驱动渲染的致谢名录 -->
        <div
          v-for="(section, index) in creditsData"
          :key="index"
          class="w-full flex flex-col items-center"
        >
          <!-- 留白区块 -->
          <div v-if="section.layout === 'spacer'" :class="section.height"></div>

          <!-- 标准双语名单 (两列对齐) -->
          <div v-else-if="section.layout === 'normal'" class="flex flex-col items-center mb-[60px]">
            <h2 class="text-[2.2em] text-[#00e5ff] font-light mb-2">{{ section.title }}</h2>
            <p class="text-[1em] text-white opacity-60 mb-[25px]">{{ section.enTitle }}</p>
            <div
              v-for="(item, i) in section.items"
              :key="i"
              class="grid grid-cols-2 justify-items-center items-center w-[27%] min-w-[280px] mb-2"
            >
              <p class="text-[1.5em] leading-[1.8] font-light whitespace-nowrap">{{ item.name }}</p>
            </div>
          </div>

          <!-- 双排双语名单 (适用于特别鸣谢) -->
          <div
            v-else-if="section.layout === 'grid-2'"
            class="flex flex-col items-center mb-[60px] w-full"
          >
            <h2 class="text-[2.2em] text-[#00e5ff] font-light mb-2">{{ section.title }}</h2>
            <p class="text-[1em] text-white opacity-60 mb-[25px]">{{ section.enTitle }}</p>
            <div class="grid grid-cols-2 gap-y-4 gap-x-12 w-[60%] justify-items-center">
              <div
                v-for="(item, i) in section.items"
                :key="i"
                class="grid grid-cols-2 w-[220px] items-center"
              >
                <p class="text-[1.5em] leading-[1.8] font-light text-right pr-4 whitespace-nowrap">
                  {{ item.name }}
                </p>
                <p
                  class="text-[1em] opacity-80 leading-[1.8] font-light text-left pl-4 whitespace-nowrap"
                >
                  {{ item.enName }}
                </p>
              </div>
            </div>
          </div>

          <!-- 1排4人纯名字 (适用于新增的 反馈提供者) -->
          <div
            v-else-if="section.layout === 'grid-4'"
            class="flex flex-col items-center mb-[60px] w-full"
          >
            <h2 class="text-[2.2em] text-[#00e5ff] font-light mb-2">{{ section.title }}</h2>
            <p class="text-[1em] text-white opacity-60 mb-[25px]">{{ section.enTitle }}</p>
            <div
              class="grid grid-cols-4 gap-y-6 gap-x-8 w-[80%] max-w-[800px] justify-items-center"
            >
              <div v-for="(item, i) in section.items" :key="i">
                <p class="text-[1.5em] leading-[1.8] font-light whitespace-nowrap overflow-visible">
                  {{ item.name }}
                </p>
              </div>
            </div>
          </div>

          <!-- 孤立文本 (如赞助者们等结尾长串) -->
          <div v-else-if="section.layout === 'single'" class="flex flex-col items-center mb-4">
            <p class="text-[1.5em] leading-[1.8] font-light">{{ section.title }}</p>
            <p class="text-[1em] opacity-80 leading-[1.8] font-light">{{ section.enTitle }}</p>
          </div>

          <!-- 特殊结尾 (还有...你) -->
          <div
            v-else-if="section.layout === 'special'"
            class="flex flex-col items-center mb-[60px]"
          >
            <div class="h-[60vh]"></div>
            <h2 class="text-[2.2em] text-[#00e5ff] font-light mb-2">{{ section.title }}</h2>
            <p class="text-[1em] text-white opacity-60 mb-[80px]">{{ section.enTitle }}</p>
            <div class="h-[60vh]"></div>
            <p class="text-[1.5em] leading-[1.8] font-light">{{ section.items?.[0]?.name }}</p>
            <p class="text-[1em] opacity-80 leading-[1.8] font-light">
              {{ section.items?.[0]?.enName }}
            </p>

            <!-- 留出足够的滚动长尾 -->
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onBeforeUnmount } from 'vue'
import { useRouter } from 'vue-router'
import StarField from '../game/standard/particles/StarField.vue'

const router = useRouter()

const isStarted = ref(false)
const bgm = ref<HTMLAudioElement | null>(null)
let timer: ReturnType<typeof setTimeout> | null = null

// --- 【修改点】结构化数据层 ---
const creditsData = [
  {
    title: '策划 & 设计',
    enTitle: 'Project Lead & Designer',
    layout: 'normal',
    items: [
      { name: '钦灵', enName: 'NoiQing Ling' },
      { name: '风雪', enName: 'Snow Wind' },
    ],
  },
  {
    title: '程序开发',
    enTitle: 'Programming',
    layout: 'normal',
    items: [
      { name: '有梦当燃', enName: 'Cute RBQ' },
      { name: '元初', enName: 'Kawaii Femboy' },
      { name: '维克多多', enName: 'Vickko Sama' },
      { name: '喵达子大人', enName: 'Cat Sama' },
      { name: 'PL', enName: 'Magic Witch' },
      { name: '影空', enName: 'Shadow Sky' },
      { name: '随时跑路', enName: 'Fucked Anytime' },
      { name: '云小姐', enName: 'Cloud Sister' },
      { name: '远足', enName: 'Femboy Two' },
      { name: 'RatMan', enName: 'RatMan' },
      { name: '小苹果', enName: 'AppleChan' },
      { name: '总督', enName: 'Great Commander' },
    ],
  },
  {
    title: '视觉艺术',
    enTitle: 'Visual Arts',
    layout: 'normal',
    items: [
      { name: 'Yukito', enName: 'QAQ' },
      { name: '柏海', enName: 'Wood & Sea' },
      { name: '梦轩', enName: 'Dream Line' },
      { name: '晚安', enName: 'Nighty Femboy' },
    ],
  },
  {
    title: '人物设计',
    enTitle: 'Character Design',
    layout: 'normal',
    items: [
      { name: '徒花', enName: 'Flowing Flower' },
      { name: '卷', enName: 'Scroll' },
    ],
  },
  {
    title: '社区管理 & 维基搭建',
    enTitle: 'Community Management & Wiki Constructing',
    layout: 'normal',
    items: [
      { name: '七辰', enName: 'Horny Whenever' },
      { name: '雅诺狐', enName: 'Yano Fox' },
      { name: '琉璃子', enName: 'Ruriko' },
    ],
  },
  {
    title: '软件打包',
    enTitle: 'Software Packing',
    layout: 'normal',
    items: [{ name: 'uwa uwa', enName: 'Loli Loli' }],
  },
  {
    title: '特别鸣谢',
    enTitle: 'Special Thanks',
    layout: 'grid-2',
    items: [
      { name: 'Thz', enName: 'Sister M' },
      { name: '安静', enName: 'Quiet Kitty' },
      { name: '莱尔', enName: 'Layeray' },
      { name: '冰花', enName: 'Femboy Flower' },
      { name: 'DaDa', enName: 'DaDa' },
      { name: '波奶', enName: 'Super Kawaii' },
      { name: '插歪', enName: 'X Y' },
      { name: '爱灵tv', enName: 'Ling TV' },
      { name: '大饼', enName: 'Big Pie' },
      { name: '氧化性VC', enName: 'VC' },
    ],
  },
  {
    // --- 【修改点】这里是你要新增的 反馈提供者 ---
    title: '反馈提供者',
    enTitle: 'Feedback Providers',
    layout: 'grid-4',
    items: [
      { name: '钦灵的主人', enName: '' },
      { name: '晏酱不会hub', enName: '' },
      { name: '闫芫Yanzya', enName: '' },
      { name: 'DBJD-CR', enName: '' },
      { name: '超绝嘴可爱天使酱星野', enName: '' },
      { name: '忆乾', enName: '' },
      { name: 'ALiiio', enName: '' },
      { name: '不败', enName: '' },
      { name: 'VAIIYA', enName: '' },
      { name: 'AChang', enName: '' },
      { name: '团子丶', enName: '' },
      { name: '七辰喵', enName: '' },
      { name: '小透明H₂O', enName: '' },
      { name: '资费', enName: '' },
      { name: '总督', enName: '' },
      { name: '猫尾草fony璨星', enName: '' },
      { name: '阳光', enName: '' },
      { name: '我是好', enName: '' },
      { name: 'Ruriko', enName: '' },
      { name: '七毛钱的苹果', enName: '' },
      { name: '我没有名字', enName: '' },
      { name: 'GCSSZ', enName: '' },
      { name: '毛玉球', enName: '' },
      { name: '是鱼鱼哦', enName: '' },
      { name: 'NOGE404', enName: '' },
      { name: '莱尔Lain_kant', enName: '' },
      { name: 'slary', enName: '' },
      { name: '呜滋', enName: '' },
      { name: '神奇jf', enName: '' },
      { name: '灵灵的小穴', enName: '' },
      { name: '泡炮糖好甜', enName: '' },
      { name: '经常被钦灵凶的Lemaxw QwQ', enName: '' },
      { name: 'Thz922', enName: '' },
      { name: '小鱼丸子285', enName: '' },
      { name: '⑨⑨⑧⑩①', enName: '' },
      { name: 'summer day', enName: '' },
      { name: 'FlameTN7', enName: '' },
      { name: '132', enName: '' },
      { name: 'cafe_awa_', enName: '' },
      { name: 'Dream喵～', enName: '' },
      { name: '明月照清泉', enName: '' },
      { name: '追云暮雨', enName: '' },
      { name: '纯树', enName: '' },
      { name: '明谦', enName: '' },
      { name: 'going', enName: '' },
      { name: '繁星', enName: '' },
      { name: '至炎若水', enName: '' },
      { name: '景星', enName: '' },
      { name: '琉璃', enName: '' },
      { name: 'NotH2O', enName: '' },
      { name: '卖猹的鲁迅258', enName: '' },
      { name: 'moyang15731', enName: '' },
      { name: '游江魂', enName: '' },
      { name: '哈？', enName: '' },
      { name: 'weilaoer', enName: '' },
      { name: '睍梦', enName: '' },
      { name: '睍梦', enName: '' },
      { name: 'kasumi', enName: '' },
      { name: 'CNCCC', enName: '' },
      { name: '活性自由基', enName: '' },
      { name: 'VAIIYA', enName: '' },
      { name: '叙清风、', enName: '' },
      { name: '锦荣', enName: '' },
      { name: 'Vector', enName: '' },
      { name: '鲍比考迪克-Official', enName: '' },
      { name: '未来', enName: '' },
      { name: '氿菻', enName: '' },
      { name: 'ANND', enName: '' },
      { name: '白苏染', enName: '' },
      { name: '鱼仚', enName: '' },
      { name: '远辰', enName: '' },
      { name: '千矢', enName: '' },
      { name: 'Baimoer', enName: '' },
      { name: '泊羽xd', enName: '' },
      { name: 'PieteIna', enName: '' },
      { name: '放点粉丝', enName: '' },
      { name: '永之信', enName: '' },
      { name: 'Puesite', enName: '' },
      { name: 'chrock', enName: '' },
      { name: 'α粒子', enName: '' },
      { name: 'awa', enName: '' },
      { name: 'XZH', enName: '' },
      // 可以无限往下补充人名，将自动每排4个向下排布
    ],
  },
  // 底部长列
  { layout: 'spacer', height: 'h-16' },
  { title: 'Issue提供者', enTitle: 'Issue Providers', layout: 'single' },
  { layout: 'spacer', height: 'h-16' },
  { title: '创意工坊作者们', enTitle: 'Character Creators', layout: 'single' },
  { layout: 'spacer', height: 'h-16' },
  { title: 'B站粉丝们', enTitle: 'Bilibili Subscribers', layout: 'single' },
  { layout: 'spacer', height: 'h-16' },
  { title: '赞助者们', enTitle: 'Donators', layout: 'single' },
  { layout: 'spacer', height: 'h-32' },
  {
    title: '还有...',
    enTitle: 'Moreover...',
    layout: 'special',
    items: [{ name: '你', enName: 'You' }],
  },
]

const startCredits = () => {
  if (!bgm.value) return
  isStarted.value = true

  bgm.value.src = '/audio/credit.mp3'
  bgm.value.load()
  bgm.value.play().catch((error) => console.error('音频播放失败:', error))
}

// --- 【修改点】音乐播放完毕后的回调跳转 ---
const onAudioEnded = () => {
  router.push('/chat')
}

onBeforeUnmount(() => {
  if (timer) clearTimeout(timer)
  // 退出组件时，如果音乐还在播则停止它
  if (bgm.value) {
    bgm.value.pause()
  }
})
</script>

<style scoped>
@import url('https://fonts.googleapis.com/css2?family=Noto+Sans+SC:wght@300;400&display=swap');

/* 核心滚动逻辑：利用 100vh 和 -100% 自动适配任意长度的数据，而不会发生超距或不到底 */
.credits-scroll {
  animation: scrollAnimation 120s linear forwards;
}

@keyframes scrollAnimation {
  from {
    transform: translateY(100vh);
  }
  to {
    transform: translateY(-100%);
  }
}

@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

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
