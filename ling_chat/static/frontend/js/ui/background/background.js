class Background {
  constructor() {
    this.container = document.createElement('div')
    this.container.className = 'background-container'
    this.container.innerHTML = `
      <img class="background">
    `
  }
  get element() {
    return this.container
  }
  changeBackgroundUrl(url) {
    this.container.querySelector('.background').src = url
  }
}
const background = new Background()
export default background