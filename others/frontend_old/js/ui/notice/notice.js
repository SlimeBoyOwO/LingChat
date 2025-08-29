import EventBus from "../core/event-bus.js";
function praseType(type) {
  switch (type) {
    case "error": return "错误"
    case "alert": return "警告"
    case "loading": return "正在进行"
    case "success": return "完成"
    default: return "未知"
  }
}
function praseTypeIcon(type) {
  switch (type) {
    case "error": return ""
    case "alert": return ""
    case "loading": return ""
    case "finish": return ""
    default: return ""
  }
}

class Notice {
  constructor() {
    this.notice = {
      notice: document.createElement("div"),
      header: document.createElement("div"),
      icon: document.createElement("img"),
      type: document.createElement("div"),
      message: document.createElement("div"),
    }
    this.notice.notice.append(this.notice.header, this.notice.message)
    this.notice.header.append(this.notice.icon, this.notice.type)

    this.notice.notice.className = "notice"
    this.notice.header.className = "notice-header"
    this.notice.icon.className = "notice-icon"
    this.notice.type.className = "notice-type"
    this.notice.message.className = "notice-message"

    this.delete()
  }
  get element() {
    return this.notice.notice
  }
  set type(type) {
    this.notice.icon.src = praseTypeIcon(type)
    this.notice.type.textContent = praseType(type)
  }
  set message(message) {
    this.notice.message.textContent = message
  }
  delete() {
    this.notice.notice.className = "not-notice"
  }
  display(position) {
    this.delete()
    this.notice.notice.className = "notice"
    this.notice.notice.classList.add(position)
    setTimeout(() => {
      this.delete()
    }, 2000)
  }
}

const notice = new Notice()
document.querySelector('body').appendChild(notice.element)

EventBus.on("notice:error", (message) => {
  notice.type = "error"
  notice.message = message
  notice.display("RB")
})
EventBus.on("notice:alert", (message) => {
  notice.type = "alert"
  notice.message = message
  notice.display("RB")
})
EventBus.on("notice:loading", (message) => {
  notice.type = "loading"
  notice.message = message
  notice.display("RB")
})
EventBus.on("notice:success", (message) => {
  notice.type = "success"
  notice.message = message 
  notice.display("RB")
})