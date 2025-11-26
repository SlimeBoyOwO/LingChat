<template>
    <span class="name">{{ name }}</span>
    <span>: </span>
    <span class="content">{{ content }}</span>
</template >

<script setup lang="ts">

const props = defineProps({
  name: {
    type: String,
    require: true,
  },
  content: {
    type: String,
    require: true,
  },
});

</script>
<style scoped>

span  {
  vertical-align: middle;
}

.name {
  font-weight: bold;
}

.symbol {
  font-weight: bold;
}

</style>