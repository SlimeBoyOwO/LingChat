<script setup lang="ts">
// 角色压缩包进度条的统一挂载点。
// ImportProgressBar 内部已通过 useRoleImportExport() 注册全局事件监听，
// 本组件仅作为功能区域内的渲染壳，避免 App.vue 关注此业务细节。
import ImportProgressBar from './ImportProgressBar.vue'
</script>

<template>
  <ImportProgressBar />
</template>
