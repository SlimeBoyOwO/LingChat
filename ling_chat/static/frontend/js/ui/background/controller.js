import eventListener from '../../core/event-bus.js'
import background from './background.js'
document.querySelector('body').append(background.element)

// 添加监听事件
eventListener.on('background:change', (url) => {
  background.changeBackgroundUrl(url)
})

// 添加元素样式
const url = import.meta.url.replace('controller.js', 'background.css')
const html = `<link rel="stylesheet" href="${url}">`
document.querySelector('head').insertAdjacentHTML('beforeend', html)


