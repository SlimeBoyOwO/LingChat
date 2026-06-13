<template>
  <MenuPage>
    <MenuItem title="创意工坊">
      <template #header>
        <Icon icon="package" :size="20" />
      </template>

      <div class="flex flex-col h-full min-h-0">
        <!-- Toolbar: category filter + sort toggle -->
        <div class="flex items-center justify-between mb-5 shrink-0 flex-wrap gap-2">
          <div class="flex items-center gap-1.5 flex-wrap">
            <button
              class="filter-btn"
              :class="{ active: selectedCategory === null }"
              @click="selectCategory(null)"
            >
              全部
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
              热度
            </button>
            <button
              class="sort-btn"
              :class="{ active: sortMode === 'newest' }"
              @click="sortMode = 'newest'"
            >
              最新
            </button>
          </div>
        </div>

        <!-- Loading -->
        <div v-if="loading" class="flex items-center justify-center py-12">
          <p class="text-white/60">正在加载讨论列表...</p>
        </div>

        <!-- Error -->
        <div v-else-if="error" class="flex flex-col items-center justify-center py-12 gap-3">
          <p class="text-red-400">{{ error }}</p>
          <button
            class="px-5 py-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors border border-white/10"
            @click="load"
          >
            重试
          </button>
        </div>

        <!-- Empty -->
        <div v-else-if="discussions.length === 0" class="flex items-center justify-center py-12">
          <p class="text-white/50">暂无讨论内容</p>
        </div>

        <!-- Filtered empty -->
        <div
          v-else-if="filteredDiscussions.length === 0"
          class="flex items-center justify-center py-12"
        >
          <p class="text-white/50">该分类下暂无内容</p>
        </div>

        <!-- Discussion cards section -->
        <template v-else>
          <!-- Token hint: no real upvote data -->
          <div
            v-if="!hasAnyUpvoteData"
            class="mb-5 px-5 py-3 rounded-xl bg-yellow-500/10 border border-yellow-500/25 text-yellow-200/80 text-sm flex items-center gap-3"
          >
            <span class="text-base">💡</span>
            <span>
              当前无法获取 Discussion 热度（upvote）数据，列表按 👍 表情数排序。 在<strong
                >高级设置 → 创意工坊</strong
              >中填入 GitHub Token 即可获取精确的 upvote 热度。
            </span>
          </div>

          <div class="grid gap-5 w-full grid-cols-1 xl:grid-cols-2">
            <div
              v-for="discussion in pagedDiscussions"
              :key="discussion.number"
              class="relative flex items-start p-5 rounded-2xl transition-all duration-300 group cursor-pointer bg-white/10 backdrop-blur-xl border border-white/10 hover:border-white/20 hover:-translate-y-0.5 hover:shadow-xl hover:shadow-white/5"
              @click="openDiscussion(discussion.html_url)"
            >
              <!-- Top-left: category icon -->
              <div
                v-if="getCornerIcon(discussion.category.name)"
                class="absolute -top-2 -left-2 w-6 h-6 rounded-full flex items-center justify-center text-brand shadow-md transform -rotate-18 z-10"
              >
                <component :is="getCornerIcon(discussion.category.name)" :size="20" />
              </div>

              <!-- Top-right: external link -->
              <button
                class="absolute top-3 right-3 p-1.5 z-10 rounded-full bg-white/5 text-white/40 hover:text-white hover:bg-white/10 transition-all"
                @click.stop="openDiscussion(discussion.html_url)"
              >
                <ExternalLink :size="14" />
              </button>

              <!-- Left: Avatar section -->
              <div
                class="flex flex-col items-center shrink-0 gap-3 w-32 border-r border-white/10 pr-5"
              >
                <div
                  class="w-28 h-28 rounded-full overflow-hidden border-2 border-white/20 shadow-lg shrink-0"
                >
                  <img
                    v-if="discussion.avatar_url"
                    :src="discussion.avatar_url"
                    :alt="discussion.title"
                    class="w-full h-full object-cover group-hover:scale-110 transition-transform duration-500"
                  />
                  <div v-else class="w-full h-full flex items-center justify-center bg-white/5">
                    <img
                      src="@/assets/images/LingChatLogo.png"
                      alt="Logo"
                      class="w-full h-full object-contain opacity-100 -rotate-20 scale-130"
                    />
                  </div>
                </div>
                <!-- Category badge -->
                <span
                  class="text-sm px-3 py-0.5 rounded-full border text-center leading-5 font-medium"
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
              <div class="flex-1 min-w-0 flex flex-col pl-4 py-0.5 h-full">
                <!-- Title -->
                <h3 class="text-xl font-bold text-white mb-2 line-clamp-2 leading-7">
                  {{ discussion.title }}
                </h3>

                <!-- Description -->
                <p class="text-base text-white/60 line-clamp-4 leading-5 mb-3 flex-1">
                  {{ getDisplayDescription(discussion) }}
                </p>

                <!-- Footer: tags -->
                <div v-if="discussion.tags.length > 0" class="flex items-center gap-1.5 flex-wrap min-h-5 mb-2">
                  <span
                    v-for="(tag, i) in discussion.tags"
                    :key="tag"
                    class="text-xs px-2 py-0.5 rounded-full border font-medium"
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
                  class="flex items-center gap-4 text-xs text-white/35 border-t border-white/5 pt-2.5"
                >
                  <!-- Upvotes -->
                  <span
                    class="flex items-center gap-1"
                    :title="discussion.has_upvotes ? 'upvote 热度' : '👍 表情数'"
                  >
                    <ThumbsUp :size="12" />
                    {{ discussion.has_upvotes ? discussion.upvotes : discussion.reactions_upvotes }}
                  </span>
                  <!-- Author -->
                  <span class="flex items-center gap-1">
                    <User :size="12" />
                    {{ discussion.author?.login ?? '未知' }}
                  </span>
                  <!-- Time -->
                  <span class="flex items-center gap-1 ml-auto">
                    <Clock :size="12" />
                    {{ formatTime(discussion.created_at) }}
                  </span>
                </div>
              </div>
            </div>
          </div>
        </template>

        <!-- Pagination -->
        <div v-if="totalPages > 1" class="flex items-center justify-between px-3 py-2 w-full mt-2">
          <button
            class="px-5 py-2 text-base font-medium border-none rounded-lg cursor-pointer bg-white/8 text-white/60 transition-all duration-200 hover:bg-white/15 hover:text-white disabled:opacity-30 disabled:cursor-not-allowed"
            :disabled="currentPage <= 1"
            @click="currentPage--"
          >
            上一页
          </button>
          <span class="text-base font-medium text-white/60">
            第 {{ currentPage }} / {{ totalPages }} 页
          </span>
          <button
            class="px-5 py-2 text-base font-medium border-none rounded-lg cursor-pointer bg-white/8 text-white/60 transition-all duration-200 hover:bg-white/15 hover:text-white disabled:opacity-30 disabled:cursor-not-allowed"
            :disabled="currentPage >= totalPages"
            @click="currentPage++"
          >
            下一页
          </button>
        </div>

        <!-- Refresh button -->
        <div v-if="!loading && !error" class="flex justify-center mt-6">
          <button
            class="px-5 py-1.5 rounded-lg bg-white/5 hover:bg-white/10 text-white/40 hover:text-white/70 text-sm transition-all border border-white/5 hover:border-white/15"
            @click="load"
          >
            刷新列表
          </button>
        </div>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { MenuPage, MenuItem } from '../../ui'
import Icon from '@/components/base/widget/Icon.vue'
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
const ITEMS_PER_PAGE = 10

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
  if (!d.body) return '暂无描述'
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
  if (mins < 1) return '刚刚'
  if (mins < 60) return `${mins} 分钟前`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours} 小时前`
  const days = Math.floor(hours / 24)
  if (days < 30) return `${days} 天前`
  const months = Math.floor(days / 30)
  if (months < 12) return `${months} 个月前`
  return `${Math.floor(months / 12)} 年前`
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
    error.value = typeof e === 'string' ? e : err?.message || '加载失败'
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  load()
})
</script>

<style scoped>
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
