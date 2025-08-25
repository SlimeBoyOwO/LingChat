<template>
  <Loader />
  <FastLoader />
  <MainPanel />
</template>

<script setup lang="ts">
import MainPanel from "./components/views/MainPanel.vue";
import { Loader, FastLoader } from "./components/loaders";
</script>

<style>
:root {
  --accent-color: #79d9ff;
  --menu-max-width: 1100px;
}

body,
html {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  font-family:
    -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue",
    Arial, sans-serif;
  overflow: hidden;
  background: transparent;
}

.glass-effect {
  background-color: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  border: 1px solid rgba(255, 255, 255, 0.125);
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.1);
  transition: border-color 0.2s, box-shadow 0.2s;
}
</style>
