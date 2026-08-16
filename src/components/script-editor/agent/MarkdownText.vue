<script setup lang="ts">
/**
 * 把 LLM 输出的 Markdown 安全渲染成 HTML。
 *
 * - GFM + 单换行断行：表格、任务列表、~~删除线~~ 都能显示，聊天单换行也会断行。
 * - XSS 安全：assistant 内容可能被文件内容诱导（prompt injection）带上 `<script>` /
 *   `<img onerror>`，因此原始 HTML 一律转义成可见文本，绝不原样输出。
 * - 链接点击交给 @tauri-apps/plugin-opener 在外部浏览器打开（WebView 内点 `<a>` 会
 *   直接导航，可能把整个应用页面跳走）。
 */
import { computed } from 'vue'
import { marked } from 'marked'
import { openUrl } from '@tauri-apps/plugin-opener'
import { escapeHtml } from '@/utils/escapeHtml'

// 单例配置：整应用只有一个 marked 实例，模块加载时配置一次即可。
marked.use({
  renderer: {
    // 原始 HTML 块/片段 → 转义为可见文本
    html({ text }) {
      return escapeHtml(text)
    },
  },
})

const props = defineProps<{ content: string }>()

const html = computed(() => {
  if (!props.content) return ''
  // async 关闭 → 同步返回 string
  return (marked.parse(props.content, { gfm: true, breaks: true }) as string) || ''
})

function onClick(e: MouseEvent) {
  const a = (e.target as HTMLElement).closest('a')
  if (!a) return
  const href = a.getAttribute('href')
  if (!href) return
  e.preventDefault()
  void openUrl(href)
}
</script>

<template>
  <div
    class="markdown-body"
    v-html="html"
    @click="onClick"
  ></div>
</template>
