<template>
  <div class="flow">
    <template
      v-for="(band, bi) in bands"
      :key="bi"
    >
      <!-- 子目录分组带：Chapters/<子目录>/ -->
      <div
        v-if="band.group"
        class="gband"
      >
        <span class="glabel">Chapters / {{ band.group }}</span>
        <ChapterLayer
          v-for="(layer, li) in band.layers"
          :key="li"
          :layer="layer"
          :last="li === band.layers.length - 1 && bi === bands.length - 1"
        />
      </div>

      <ChapterLayer
        v-else
        v-for="(layer, li) in band.layers"
        :key="'p' + li"
        :layer="layer"
        :last="li === band.layers.length - 1 && bi === bands.length - 1"
      />
    </template>

    <!-- 剧本结束 -->
    <div class="connector"></div>
    <div class="endcap">剧本结束</div>

    <p class="hint">
      从每章最后一条「章节结束」反推出连线 —— 数据即视图，没有额外的布局文件要同步。
      黄色虚线框表示没有任何章节指向它，玩家走不到。
    </p>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import type { ChapterSummary } from '@/api/services/script-editor'
import ChapterLayer from './ChapterLayer.vue'

const store = useScriptEditorStore()

/** 从 chapter_end 反推出的出边。summary 里没有事件内容，所以用诊断兜底判断断链。 */
interface Node extends ChapterSummary {
  isIntro: boolean
  isOrphan: boolean
  errorCount: number
}

const introId = computed(() => {
  const raw = store.detail?.storyConfig?.intro_chapter
  return typeof raw === 'string' ? raw.replace(/\.yaml$/, '') : 'main'
})

/** 校验器已经算过可达性，这里直接用它的结论，避免前后端两套图算法 */
const orphanIds = computed(() => {
  const set = new Set<string>()
  for (const d of store.report?.diagnostics ?? []) {
    if (d.code === 'graph.unreachable' && d.chapter) set.add(d.chapter)
  }
  return set
})

const errorCountOf = (id: string) =>
  (store.report?.diagnostics ?? []).filter((d) => d.chapter === id && d.severity === 'error').length

const nodes = computed<Node[]>(() =>
  store.chapters.map((c) => ({
    ...c,
    isIntro: c.id === introId.value,
    isOrphan: orphanIds.value.has(c.id),
    errorCount: errorCountOf(c.id),
  })),
)

/**
 * 纵向布局：按子目录切成「带」，带内每章一层。
 *
 * 官方语料里只有 Intro/ 一种子目录用法，而且同一目录下的章节是连续的，
 * 所以按「连续同目录」切带就够了，不需要真正的图分层。
 */
interface Band {
  group?: string
  layers: Node[][]
}

const bands = computed<Band[]>(() => {
  const out: Band[] = []
  let current: Band | null = null

  for (const n of nodes.value) {
    const group = n.group
    if (!current || current.group !== group) {
      current = { group, layers: [] }
      out.push(current)
    }
    current.layers.push([n])
  }
  return out
})
</script>

<style scoped>
@reference "tailwindcss";

.flow {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 18px 8px 24px;
}

/* 层与层之间的竖直连线 + 箭头 */
.connector {
  position: relative;
  width: 1px;
  height: 34px;
  background: rgba(121, 217, 255, 0.55);
}
.connector::after {
  content: '';
  position: absolute;
  left: -4px;
  bottom: 0;
  border-left: 4.5px solid transparent;
  border-right: 4.5px solid transparent;
  border-top: 8px solid rgba(121, 217, 255, 0.6);
}

.gband {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  width: 100%;
  max-width: 540px;
  margin: 8px auto;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 14px;
  padding: 26px 14px 14px;
  background: rgba(0, 0, 0, 0.16);
}
.glabel {
  position: absolute;
  top: -9px;
  left: 14px;
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 5px;
  padding: 2px 9px;
  font-size: 0.68rem;
  color: rgba(255, 255, 255, 0.7);
  background: #16202c;
}

.endcap {
  border: 1px dashed rgba(255, 255, 255, 0.25);
  border-radius: 99px;
  padding: 5px 14px;
  font-size: 0.72rem;
  white-space: nowrap;
  color: rgba(255, 255, 255, 0.5);
}

.hint {
  max-width: 540px;
  margin-top: 26px;
  font-size: 0.75rem;
  line-height: 1.8;
  color: rgba(255, 255, 255, 0.4);
}
</style>
