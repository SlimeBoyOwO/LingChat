import { createPinia } from 'pinia'
import { persist } from './plugins/persist'

// 初始化 Pinia
const pinia = createPinia()

// 注册持久化插件
pinia.use(persist)

export default pinia
