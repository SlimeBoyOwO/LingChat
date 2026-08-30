<script setup lang="ts">
// 插件压缩包进度条的统一挂载点（与 RoleArchiveProgress.vue 同构）。
// ImportProgressBar 内部通过 usePluginImport() 注册全局事件监听，
// 本组件仅作为插件功能区域内的渲染壳。
import ImportProgressBar from './ImportProgressBar.vue'
</script>

<template>
  <ImportProgressBar source="plugin" />
</template>
