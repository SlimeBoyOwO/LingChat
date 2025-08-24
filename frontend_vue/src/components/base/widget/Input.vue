<template>
  <div class="form-group">
    <!-- 
      This is the text input element.
      - :value="modelValue" binds the input's value to the modelValue prop.
      - @input event emits the 'update:modelValue' event, which allows using v-model.
      - v-bind="$attrs" passes down any extra HTML attributes (like placeholder, id, etc.) to the input.
    -->
    <input
      type="text"
      class="form-control"
      :value="modelValue"
      @input="$emit('update:modelValue', $event.target.value)"
      v-bind="$attrs"
    />
  </div>
</template>

<script>
export default {
  name: "GlassInput",

  inheritAttrs: false,

  props: {
    modelValue: {
      type: String,
      default: "",
    },
  },

  emits: ["update:modelValue"],
};
</script>

<style scoped>
.form-group {
  width: 100%;
}

.form-group .form-control {
  width: 100%;
  padding: 10px 12px;
  resize: vertical;

  color: #fff;
  font-size: 15px;
  font-family: inherit;

  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);

  border: 1px solid rgba(255, 255, 255, 0.125);
  border-radius: 8px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.1);

  transition: border-color 0.2s, box-shadow 0.2s;
}

.form-group .form-control:focus {
  outline: none;
  border-color: var(--accent-color);
  box-shadow: 0 0 0 3px rgba(0, 123, 255, 0.2);
}
</style>
