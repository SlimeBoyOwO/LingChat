<template>
  <!-- 云端创意工坊：独立全屏页，主菜单「创意工坊」二级菜单进入（原设置页 workshop 标签迁移） -->
  <div class="workshop-page">
    <!-- 背景层（与主菜单同一张背景图 + 暗色遮罩保证对比度） -->
    <div class="workshop-page__bg"></div>

    <div class="relative z-10 flex h-full w-full flex-col gap-4 p-4 md:p-8">
      <!-- 顶部：返回 + 标题 -->
      <div class="flex shrink-0 items-center justify-between gap-4">
        <button
          class="inline-flex shrink-0 items-center gap-2 rounded-xl border border-white/15 bg-white/8 px-4 py-2 text-[0.9rem] text-white/80 backdrop-blur-xl transition-all duration-200 hover:bg-white/15 hover:text-white"
          @click="goBack"
        >
          ← {{ t('views.menu.back') }}
        </button>
        <h1
          class="flex-1 truncate text-center text-2xl font-bold text-white drop-shadow-[0_2px_8px_rgba(0,0,0,0.6)] md:text-3xl"
        >
          {{ t('views.menu.cloudWorkshop') }}
        </h1>
        <div class="w-16 shrink-0 md:w-20"></div>
      </div>

      <!-- 内容容器（毛玻璃卡片流，原 SettingsWorkshop 内容） -->
      <div
        class="min-h-0 flex-1 overflow-y-auto custom-scrollbar rounded-2xl border border-white/10 bg-white/8 p-4 backdrop-blur-2xl md:p-6"
      >
        <!-- Toolbar: category filter + sort toggle -->
        <div class="mb-5 flex shrink-0 flex-wrap items-center justify-between gap-2">
          <div class="flex flex-wrap items-center gap-1.5">
            <button
              class="filter-btn"
              :class="{ active: selectedCategory === null }"
              @click="selectCategory(null)"
            >
              {{ $t('settings.workshop.all') }}
            </button>
            <button
              v-for="cat in categories"
              :key="cat.name"
              class="filter-btn"
              :class="{ active: selectedCategory === cat.name }"
              :style="{
                '--cat-color': cat.color,
                '--cat-bg': cat.color + '22',
              }"
              @click="selectCategory(cat.name)"
            >
              {{ cat.name }}
            </button>
            <span class="text-sm text-white/40 ml-2"
              >{{ filteredDiscussions.length }} / {{ discussions.length }}</span
            >
          </div>

          <!-- Sort toggle -->
          <div class="flex items-center gap-1 rounded-lg bg-white/5 p-0.5">
            <button
              class="sort-btn"
              :class="{ active: sortMode === 'hot' }"
              @click="sortMode = 'hot'"
            >
              {{ $t('settings.workshop.hot') }}
            </button>
            <button
              class="sort-btn"
              :class="{ active: sortMode === 'newest' }"
              @click="sortMode = 'newest'"
            >
              {{ $t('settings.workshop.newest') }}
            </button>
          </div>
        </div>

        <!-- Loading -->
        <div v-if="loading" class="flex items-center justify-center py-12">
          <p class="text-white/60">{{ $t('settings.workshop.loadingList') }}</p>
        </div>

        <!-- Error -->
        <div v-else-if="error" class="flex flex-col items-center justify-center gap-3 py-12">
          <p class="text-red-400">{{ error }}</p>
          <button
            class="rounded-lg border border-white/10 bg-white/10 px-5 py-2 text-white transition-colors hover:bg-white/20"
            @click="load"
          >
            {{ $t('settings.workshop.retry') }}
          </button>
        </div>

        <!-- Empty -->
        <div v-else-if="discussions.length === 0" class="flex items-center justify-center py-12">
          <p class="text-white/50">{{ $t('settings.workshop.empty') }}</p>
        </div>

        <!-- Filtered empty -->
        <div
          v-else-if="filteredDiscussions.length === 0"
          class="flex items-center justify-center py-12"
        >
          <p class="text-white/50">{{ $t('settings.workshop.emptyCategory') }}</p>
        </div>

        <!-- Discussion cards section -->
        <template v-else>
          <!-- Token hint: no real upvote data -->
          <div
            v-if="!hasAnyUpvoteData"
            class="mb-5 flex items-center gap-3 rounded-xl border border-yellow-500/25 bg-yellow-500/10 px-5 py-3 text-sm text-yellow-200/80"
          >
            <span class="text-base">💡</span>
            <span>
              {{ $t('settings.workshop.upvoteHint1') }}<strong
                >{{ $t('settings.workshop.upvoteHintLink') }}</strong
              >{{ $t('settings.workshop.upvoteHint2') }}
            </span>
          </div>

          <div class="grid w-full gap-5 grid-cols-1 xl:grid-cols-2">
            <div
              v-for="discussion in pagedDiscussions"
              :key="discussion.number"
              class="group relative flex items-start rounded-2xl border border-white/10 bg-white/10 p-5 backdrop-blur-xl transition-all duration-300 hover:-translate-y-0.5 hover:border-white/20 hover:shadow-xl hover:shadow-white/5 cursor-pointer"
              @click="openDiscussion(discussion.html_url)"
            >
              <!-- Top-left: category icon -->
              <div
                v-if="getCornerIcon(discussion.category.name)"
                class="absolute -top-2 -left-2 z-10 flex w-6 h-6 -rotate-18 items-center justify-center rounded-full text-brand shadow-md"
              >
                <component :is="getCornerIcon(discussion.category.name)" :size="20" />
              </div>

              <!-- Top-right: external link -->
              <button
                class="absolute top-3 right-3 z-10 rounded-full bg-white/5 p-1.5 text-white/40 transition-all hover:bg-white/10 hover:text-white"
                @click.stop="openDiscussion(discussion.html_url)"
              >
                <ExternalLink :size="14" />
              </button>

              <!-- Left: Avatar section -->
              <div
                class="flex w-32 shrink-0 flex-col items-center gap-3 border-r border-white/10 pr-5"
              >
                <div
                  class="h-28 w-28 shrink-0 overflow-hidden rounded-full border-2 border-white/20 shadow-lg"
                >
                  <img
                    v-if="discussion.avatar_url"
                    :src="discussion.avatar_url"
                    :alt="discussion.title"
                    class="h-full w-full object-cover transition-transform duration-500 group-hover:scale-110"
                  />
                  <div v-else class="flex h-full w-full items-center justify-center bg-white/5">
                    <img
                      src="@/assets/images/LingChatLogo.png"
                      alt="Logo"
                      class="h-full w-full -rotate-20 scale-130 object-contain opacity-100"
                    />
                  </div>
                </div>
                <!-- Category badge -->
                <span
                  class="rounded-full border px-3 py-0.5 text-center text-sm font-medium leading-5"
                  :style="{
                    backgroundColor: getCategoryColor(discussion.category.name) + '22',
                    borderColor: getCategoryColor(discussion.category.name) + '4D',
                    color: getCategoryColor(discussion.category.name),
                  }"
                >
                  {{ discussion.category.name }}
                </span>
              </div>

              <!-- Right: Content -->
              <div class="flex h-full min-w-0 flex-1 flex-col py-0.5 pl-4">
                <!-- Title -->
                <h3 class="mb-2 line-clamp-2 text-xl font-bold leading-7 text-white">
                  {{ discussion.title }}
                </h3>

                <!-- Description -->
                <p class="mb-3 line-clamp-4 flex-1 text-base leading-5 text-white/60">
                  {{ getDisplayDescription(discussion) }}
                </p>

                <!-- Footer: tags -->
                <div
                  v-if="discussion.tags.length > 0"
                  class="mb-2 flex min-h-5 flex-wrap items-center gap-1.5"
                >
                  <span
                    v-for="(tag, i) in discussion.tags"
                    :key="tag"
                    class="rounded-full border px-2 py-0.5 text-xs font-medium"
                    :style="{
                      backgroundColor: getTagColor(i) + '22',
                      borderColor: getTagColor(i) + '4D',
                      color: getTagColor(i),
                    }"
                  >
                    {{ tag }}
                  </span>
                </div>

                <!-- Footer: meta info -->
                <div
                  class="flex items-center gap-4 border-t border-white/5 pt-2.5 text-xs text-white/35"
                >
                  <!-- Upvotes -->
                  <span
                    class="flex items-center gap-1"
                    :title="discussion.has_upvotes ? $t('settings.workshop.upvoteTitle') : $t('settings.workshop.reactionTitle')"
                  >
                    <ThumbsUp :size="12" />
                    {{ discussion.has_upvotes ? discussion.upvotes : discussion.reactions_upvotes }}
                  </span>
                  <!-- Author -->
                  <span class="flex items-center gap-1">
                    <User :size="12" />
                    {{ discussion.author?.login ?? $t('settings.workshop.unknownAuthor') }}
                  </span>
                  <!-- Time -->
                  <span class="ml-auto flex items-center gap-1">
                    <Clock :size="12" />
                    {{ formatTime(discussion.created_at) }}
                  </span>
                </div>
              </div>
            </div>
          </div>
        </template>

        <!-- Pagination -->
        <div v-if="totalPages > 1" class="mt-2 flex w-full items-center justify-between px-3 py-2">
          <button
            class="rounded-lg border-none bg-white/8 px-5 py-2 text-base font-medium text-white/60 transition-all duration-200 hover:bg-white/15 hover:text-white disabled:cursor-not-allowed disabled:opacity-30"
            :disabled="currentPage <= 1"
            @click="currentPage--"
          >
            {{ $t('settings.shared.prevPage') }}
          </button>
          <span class="text-base font-medium text-white/60">
            {{ $t('settings.shared.pageOf', { current: currentPage, total: totalPages }) }}
          </span>
          <button
            class="rounded-lg border-none bg-white/8 px-5 py-2 text-base font-medium text-white/60 transition-all duration-200 hover:bg-white/15 hover:text-white disabled:cursor-not-allowed disabled:opacity-30"
            :disabled="currentPage >= totalPages"
            @click="currentPage++"
          >
            {{ $t('settings.shared.nextPage') }}
          </button>
        </div>

        <!-- Refresh button -->
        <div v-if="!loading && !error" class="mt-6 flex justify-center">
          <button
            class="rounded-lg border border-white/5 bg-white/5 px-5 py-1.5 text-sm text-white/40 transition-all hover:border-white/15 hover:bg-white/10 hover:text-white/70"
            @click="load"
          >
            {{ $t('settings.workshop.refreshList') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { fetchDiscussions, type Discussion } from '@/api/services/workshop'
import { openUrl } from '@tauri-apps/plugin-opener'
import { Cat, Clover, ExternalLink, ThumbsUp, User, Clock } from 'lucide-vue-next'
import type { Component } from 'vue'

// ── Data ──────────────────────────────────────────────────────

const discussions = ref<Discussion[]>([])
const loading = ref(true)
const error = ref<string | null>(null)
const selectedCategory = ref<string | null>(null)
const currentPage = ref(1)
const sortMode = ref<'hot' | 'newest'>('hot')
const { t } = useI18n()
const router = useRouter()
const ITEMS_PER_PAGE = 10

const goBack = () => {
  // 从主菜单（或设置）进入后返回上一页；直接访问时兜底回主菜单
  if (window.history.length > 1) router.back()
  else router.push('/')
}

// ── Category colors ───────────────────────────────────────────

function getCategoryColor(name: string): string {
  const n = name.toLowerCase()
  if (/人物|角色|character/i.test(n)) return '#79d9ff'
  if (/剧本|故事|script|story/i.test(n)) return '#a855f7'
  if (/资源|工具|素材|模组|asset|tool|plugin|mod/i.test(n)) return '#4ade80'
  if (/背景|background/i.test(n)) return '#3b82f6'
  if (/音乐|music|bgm/i.test(n)) return '#ec4899'
  if (/语音|voice|tts/i.test(n)) return '#eab308'
  return '#6b7280'
}

const TAG_RAINBOW = [
  '#fca5a5', // 红
  '#fdba74', // 橙
  '#fde047', // 黄
  '#86efac', // 绿
  '#93c5fd', // 蓝
  '#a5b4fc', // 靛
  '#d8b4fe', // 紫
]

function getTagColor(index: number): string {
  return TAG_RAINBOW[index % TAG_RAINBOW.length]
}

function getCornerIcon(name: string): Component | null {
  const n = name.toLowerCase()
  if (/人物|角色|character/i.test(n)) return Cat
  if (/资源|工具|素材|模组|asset|tool|plugin|mod/i.test(n)) return Clover
  return null
}

// ── Categories ────────────────────────────────────────────────

const categories = computed(() => {
  const seen = new Set<string>()
  const result: { name: string; color: string }[] = []
  for (const d of discussions.value) {
    const name = d.category.name
    if (!seen.has(name)) {
      seen.add(name)
      result.push({ name, color: getCategoryColor(name) })
    }
  }
  return result
})

// ── Sort → Filter → Pagination ────────────────────────────────

const hasAnyUpvoteData = computed(() => discussions.value.some((d) => d.has_upvotes))

const sortedDiscussions = computed(() => {
  const arr = [...discussions.value]
  if (sortMode.value === 'hot') {
    // 优先用真实 upvotes，没有则用 👍 表情数
    arr.sort((a, b) => {
      const aScore = a.has_upvotes ? a.upvotes : a.reactions_upvotes
      const bScore = b.has_upvotes ? b.upvotes : b.reactions_upvotes
      return bScore - aScore
    })
  } else {
    arr.sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime())
  }
  return arr
})

const filteredDiscussions = computed(() => {
  if (!selectedCategory.value) return sortedDiscussions.value
  return sortedDiscussions.value.filter((d) => d.category.name === selectedCategory.value)
})

const totalPages = computed(() =>
  Math.max(1, Math.ceil(filteredDiscussions.value.length / ITEMS_PER_PAGE)),
)

const pagedDiscussions = computed(() => {
  const start = (currentPage.value - 1) * ITEMS_PER_PAGE
  return filteredDiscussions.value.slice(start, start + ITEMS_PER_PAGE)
})

function selectCategory(name: string | null) {
  selectedCategory.value = selectedCategory.value === name ? null : name
}

watch(selectedCategory, () => {
  currentPage.value = 1
})
watch(sortMode, () => {
  currentPage.value = 1
})

// ── Display helpers ───────────────────────────────────────────

function getDisplayDescription(d: Discussion): string {
  if (d.description) return d.description
  if (!d.body) return t('settings.workshop.noDesc')
  const plain = d.body
    .replace(/[#*`>\[\]()!|\\]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
  const max = 200
  return plain.length <= max ? plain : plain.slice(0, max) + '...'
}

function formatTime(iso: string): string {
  const now = Date.now()
  const then = new Date(iso).getTime()
  const diff = now - then
  const mins = Math.floor(diff / 60000)
  if (mins < 1) return t('settings.workshop.time.justNow')
  if (mins < 60) return t('settings.workshop.time.minutesAgo', { n: mins })
  const hours = Math.floor(mins / 60)
  if (hours < 24) return t('settings.workshop.time.hoursAgo', { n: hours })
  const days = Math.floor(hours / 24)
  if (days < 30) return t('settings.workshop.time.daysAgo', { n: days })
  const months = Math.floor(days / 30)
  if (months < 12) return t('settings.workshop.time.monthsAgo', { n: months })
  return t('settings.workshop.time.yearsAgo', { n: Math.floor(months / 12) })
}

function openDiscussion(url: string) {
  openUrl(url)
}

// ── Load ──────────────────────────────────────────────────────

async function load() {
  loading.value = true
  error.value = null
  try {
    discussions.value = await fetchDiscussions()
    currentPage.value = 1
  } catch (e: unknown) {
    const err = e as { message?: string }
    error.value = typeof e === 'string' ? e : err?.message || t('settings.workshop.loadFailed')
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  load()
})
</script>

<style scoped>
.workshop-page {
  width: 100%;
  height: 100%;
  position: relative;
  overflow: hidden;
}

/* 背景层：与主菜单同一张背景图，加暗色渐变遮罩保证卡片文字对比度 */
.workshop-page__bg {
  position: absolute;
  inset: -10% -10% 0;
  background-image: url('@/assets/images/background2.png');
  background-size: cover;
  background-position: center;
}

.workshop-page__bg::after {
  content: '';
  position: absolute;
  inset: 0;
  background: linear-gradient(180deg, rgba(0, 0, 0, 0.45), rgba(0, 0, 0, 0.3) 40%, rgba(0, 0, 0, 0.55));
}

.filter-btn {
  font-size: 13px;
  font-weight: 600;
  padding: 4px 12px;
  border-radius: 6px;
  border: 1px solid transparent;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.5);
  cursor: pointer;
  transition: all 0.2s ease;
  letter-spacing: 0.3px;
}
.filter-btn:hover {
  background: rgba(255, 255, 255, 0.12);
  color: rgba(255, 255, 255, 0.8);
}
.filter-btn.active {
  background: var(--cat-bg, rgba(121, 217, 255, 0.15));
  border-color: var(--cat-color, #79d9ff);
  color: var(--cat-color, #79d9ff);
}
.filter-btn.active:hover {
  background: var(--cat-color, #79d9ff);
  color: #fff;
}

.sort-btn {
  font-size: 12px;
  font-weight: 600;
  padding: 3px 10px;
  border-radius: 6px;
  border: none;
  background: transparent;
  color: rgba(255, 255, 255, 0.4);
  cursor: pointer;
  transition: all 0.2s ease;
}
.sort-btn:hover {
  color: rgba(255, 255, 255, 0.7);
}
.sort-btn.active {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
}
</style>
