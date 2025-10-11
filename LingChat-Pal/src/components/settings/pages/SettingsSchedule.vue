<template>
  <MenuPage>
    <MenuItem title="主动提醒" size="small">
      <Toggle>是否开启日程主动提醒功能</Toggle>
    </MenuItem>

    <MenuItem title="随机聊天" size="small">
      <Toggle>是否开启随机间隔主动聊天功能</Toggle>
    </MenuItem>

    <MenuItem title="窥屏权限" size="small">
      <Toggle>是否开启主动窥屏聊天功能</Toggle>
    </MenuItem>

    <MenuItem title="日程列表" size="large"> 日程组件 </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { MenuPage, MenuItem } from "../../ui";
import { Toggle } from "../../base";
</script>
