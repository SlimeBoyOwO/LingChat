<template>
  <div class="layer">
    <div
      v-for="node in layer"
      :key="node.id"
      class="cnode"
      :class="{ entry: node.isIntro, orphan: node.isOrphan }"
      @click="store.openChapter(node.id)"
    >
      <span class="cid">
        {{ leaf(node.id) }}.yaml{{ node.isIntro ? ' · 开场' : '' }}{{
          node.isOrphan ? ' · 无人进入' : ''
        }}
      </span>

      <div class="crow">
        <span class="ct">{{ node.name || node.id }}</span>
        <span class="cm">{{ node.eventCount }} 个事件</span>
      </div>

      <div
        v-if="node.errorCount"
        class="cbad"
      >
        ● {{ node.errorCount }} 个错误
      </div>
      <div
        v-if="node.isOrphan"
        class="cwarn"
      >
        ▲ 没有章节指向它，玩家走不到
      </div>

      <button
        class="del"
        title="删除章节"
        @click.stop="store.deleteChapter(node.id)"
      >
        ✕
      </button>
    </div>
  </div>

  <div
    v-if="!last"
    class="connector"
  ></div>
</template>

<script setup lang="ts">
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import type { ChapterSummary } from '@/api/services/script-editor'

interface Node extends ChapterSummary {
  isIntro: boolean
  isOrphan: boolean
  errorCount: number
}

defineProps<{ layer: Node[]; last: boolean }>()

const store = useScriptEditorStore()

const leaf = (id: string) => (id.includes('/') ? id.slice(id.lastIndexOf('/') + 1) : id)
</script>

<style scoped>
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
.cbad {
  margin-top: 7px;
  font-size: 0.68rem;
  color: #fca5a5;
}
.cwarn {
  margin-top: 7px;
  font-size: 0.68rem;
  color: #fcd34d;
}

.del {
  position: absolute;
  top: 8px;
  right: 8px;
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

.connector {
  position: relative;
  width: 1px;
  height: 34px;
  margin: 0 auto;
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
</style>
