<template>
  <div class="flow">
    <p
      v-if="!store.report"
      class="hint"
    >
      正在读取章节跳转关系…
    </p>

    <template v-else>
      <div
        v-for="(row, ri) in rows"
        :key="row.key"
      >
        <!-- 层与层之间的连线。分支层上方画一个分叉提示 -->
        <div
          v-if="ri > 0"
          class="conn"
          :class="{ fork: row.nodes.length > 1 }"
        >
          <span
            v-if="row.inboundLabels.length"
            class="conn-label"
            >{{ row.inboundLabels.join(' / ') }}</span
          >
        </div>

        <div class="layer">
          <div
            v-for="node in row.nodes"
            :key="node.id"
            class="cnode"
            :class="{ entry: node.isIntro, orphan: node.isOrphan }"
            @click="open(node.id)"
          >
            <span class="cid">
              {{ leaf(node.id) }}.yaml{{ node.isIntro ? ' · 开场' : ''
              }}{{ node.isOrphan ? ' · 无人进入' : '' }}
            </span>

            <div class="crow">
              <span class="ct">{{ node.name || node.id }}</span>
              <span class="cm">{{ node.eventCount }} 个事件</span>
            </div>

            <div class="cfoot">
              <span
                v-if="node.errors"
                class="tag tag-err"
                >{{ node.errors }} 个错误</span
              >
              <span
                v-else-if="node.warns"
                class="tag tag-warn"
                >{{ node.warns }} 个提醒</span
              >
              <span
                v-if="node.endType && node.endType !== 'linear'"
                class="tag tag-branch"
                >{{ node.endType === 'branching' ? '条件分支' : 'AI 判定分支' }}</span
              >
              <span
                v-if="node.isOrphan"
                class="tag tag-warn"
                >玩家走不到</span
              >
              <button
                class="del"
                title="删除章节"
                @click.stop="store.deleteChapter(node.id)"
              >
                ✕
              </button>
            </div>
          </div>
        </div>
      </div>

      <div class="conn"></div>
      <div class="endcap">剧本结束</div>

      <p class="hint">
        这张图是<b>读出来的，不是排出来的</b> —— 连线来自每章最后那条「章节结束」，
        章节的先后也由它决定。要改走向，进对应章节改那条事件的
        <code>下一章</code>；改完这里会跟着变。
      </p>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useScriptEditorStore } from '@/stores/modules/script-editor'

interface FlowNode {
  id: string
  name?: string
  eventCount: number
  isIntro: boolean
  isOrphan: boolean
  errors: number
  warns: number
  /** 该章 chapter_end 的 end_type，用来标注分支类型 */
  endType: string
}

interface FlowRow {
  key: string
  nodes: FlowNode[]
  /** 从上一层进来的分支标签 */
  inboundLabels: string[]
}

const store = useScriptEditorStore()

const leaf = (id: string) => (id.includes('/') ? id.slice(id.lastIndexOf('/') + 1) : id)

const open = (id: string) => void store.openChapter(id)

/** 校验器已经算过可达性，直接用它的结论，避免前后端两套图算法 */
const orphanIds = computed(() => {
  const set = new Set<string>()
  for (const d of store.report?.diagnostics ?? []) {
    if (d.code === 'graph.unreachable' && d.chapter) set.add(d.chapter)
  }
  return set
})

/**
 * 按真实跳转关系分层：从开场章节沿 edges 广度优先。
 *
 * 早先这里是把章节按文件名字典序排一列，箭头表达的是「id 的字母顺序」而不是
 * 真实跳转 —— 看起来对但完全不对，是最容易骗到下一个维护者的地方。
 */
const rows = computed<FlowRow[]>(() => {
  const summaries = new Map(store.chapters.map((c) => [c.id, c]))
  const diag = store.diagnosticsByChapter
  const edges = store.edges

  const outgoing = new Map<string, { to: string; label: string; endType: string }[]>()
  for (const e of edges) {
    if (e.isEnd) continue
    const list = outgoing.get(e.from) ?? []
    list.push({ to: e.to, label: e.label ?? '', endType: e.endType })
    outgoing.set(e.from, list)
  }
  const endTypeOf = new Map<string, string>()
  for (const e of edges) endTypeOf.set(e.from, e.endType)

  const mk = (id: string): FlowNode => {
    const s = summaries.get(id)
    const d = diag[id] ?? { errors: 0, warns: 0, infos: 0 }
    const endType = endTypeOf.get(id) ?? ''
    return {
      id,
      name: s?.name,
      eventCount: s?.eventCount ?? 0,
      isIntro: id === store.introChapter,
      isOrphan: orphanIds.value.has(id),
      errors: d.errors,
      warns: d.warns,
      endType,
    }
  }

  const out: FlowRow[] = []
  const seen = new Set<string>()
  let frontier = summaries.has(store.introChapter) ? [store.introChapter] : []
  let inboundLabels: string[] = []

  while (frontier.length) {
    const layer = frontier.filter((id) => !seen.has(id) && summaries.has(id))
    if (!layer.length) break
    layer.forEach((id) => seen.add(id))
    out.push({
      key: layer.join('|'),
      nodes: layer.map(mk),
      inboundLabels: inboundLabels.filter(Boolean),
    })

    const nextIds: string[] = []
    const nextLabels: string[] = []
    for (const id of layer) {
      for (const e of outgoing.get(id) ?? []) {
        if (!nextIds.includes(e.to)) {
          nextIds.push(e.to)
          nextLabels.push(e.label)
        }
      }
    }
    frontier = nextIds
    inboundLabels = nextLabels
  }

  // 走不到的章节单独挂在末尾，不参与主链布局
  const unreached = store.chapters.filter((c) => !seen.has(c.id))
  if (unreached.length) {
    out.push({
      key: '__unreached',
      nodes: unreached.map((c) => mk(c.id)),
      inboundLabels: [],
    })
  }

  return out
})

</script>

<style scoped>
.flow {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 18px 8px 24px;
}

/* 层与层之间的竖直连线 + 箭头 */
.conn {
  position: relative;
  width: 1px;
  height: 34px;
  background: rgba(121, 217, 255, 0.55);
}
.conn::after {
  content: '';
  position: absolute;
  left: -4px;
  bottom: 0;
  border-left: 4.5px solid transparent;
  border-right: 4.5px solid transparent;
  border-top: 8px solid rgba(121, 217, 255, 0.6);
}
/* 分叉：横一道短线示意「这里分开了」 */
.conn.fork::before {
  content: '';
  position: absolute;
  top: 50%;
  left: -60px;
  width: 120px;
  height: 1px;
  background: rgba(121, 217, 255, 0.35);
}
.conn-label {
  position: absolute;
  left: 10px;
  top: 50%;
  transform: translateY(-50%);
  font-family: ui-monospace, Menlo, monospace;
  font-size: 9.5px;
  white-space: nowrap;
  color: rgba(255, 255, 255, 0.5);
}

.layer {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 18px;
  width: 100%;
}

.cnode {
  position: relative;
  flex: 1 1 300px;
  max-width: 460px;
  border: 1px solid rgba(255, 255, 255, 0.125);
  border-radius: 12px;
  padding: 12px 14px;
  cursor: pointer;
  background: rgba(255, 255, 255, 0.1);
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.1);
  transition: all 0.2s ease-in-out;
}
.cnode:hover {
  border-color: var(--accent-color);
  transform: translateY(-2px);
  box-shadow: 0 6px 18px rgba(121, 217, 255, 0.22);
}
.cnode.entry {
  border-color: rgba(74, 222, 128, 0.5);
}
.cnode.entry .cid {
  color: #4ade80;
  border-color: rgba(74, 222, 128, 0.4);
}
.cnode.orphan {
  border-style: dashed;
  border-color: rgba(251, 191, 36, 0.5);
}
.cnode.orphan .cid {
  color: #fcd34d;
  border-color: rgba(251, 191, 36, 0.4);
}

.cid {
  position: absolute;
  top: -8px;
  left: 12px;
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 4px;
  padding: 1px 6px;
  font-family: ui-monospace, Menlo, monospace;
  font-size: 9.5px;
  color: rgba(255, 255, 255, 0.5);
  background: #16202c;
}

.crow {
  display: flex;
  align-items: baseline;
  gap: 8px;
}
.ct {
  font-size: 0.88rem;
  font-weight: 600;
  color: #fff;
}
.cm {
  margin-left: auto;
  font-size: 0.7rem;
  white-space: nowrap;
  color: rgba(255, 255, 255, 0.45);
}

.cfoot {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 8px;
  min-height: 1rem;
}
.tag {
  border-radius: 4px;
  padding: 1px 6px;
  font-size: 0.64rem;
  white-space: nowrap;
}
.tag-err {
  color: #fca5a5;
  border: 1px solid rgba(248, 113, 113, 0.35);
  background: rgba(248, 113, 113, 0.15);
}
.tag-warn {
  color: #fcd34d;
  border: 1px solid rgba(251, 191, 36, 0.3);
  background: rgba(251, 191, 36, 0.15);
}
.tag-branch {
  color: #c4b5fd;
  border: 1px solid rgba(167, 139, 250, 0.35);
  background: rgba(167, 139, 250, 0.15);
}
.del {
  margin-left: auto;
  border-radius: 4px;
  padding: 1px 5px;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.25);
  opacity: 0;
  transition: all 0.15s;
}
.cnode:hover .del {
  opacity: 1;
}
.del:hover {
  color: #fca5a5;
  background: rgba(248, 113, 113, 0.15);
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
  max-width: 560px;
  margin-top: 26px;
  font-size: 0.75rem;
  line-height: 1.9;
  color: rgba(255, 255, 255, 0.4);
}
.hint b {
  color: rgba(255, 255, 255, 0.65);
}
.hint code {
  font-family: ui-monospace, Menlo, monospace;
  color: var(--accent-color);
}
</style>
