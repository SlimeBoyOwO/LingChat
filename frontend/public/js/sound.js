function initSound() {
    // 获取必要的DOM元素
    const menuContent = document.getElementById("menu-content");
    const menuSound = document.getElementById("menu-sound");
    const soundPage = document.getElementById("sound-page");
    const menuImage = document.getElementById("menu-image");
    const imagePage = document.getElementById("background-page");
    const historyToggle = document.getElementById("menu-history");
    const historyContent = document.getElementById("history-page");
    const clearHistoryBtn = document.getElementById("clear-history");
    const menuText = document.getElementById("menu-text");
    const textPage = document.getElementById("text-page");
    
    // 切换显示/隐藏
    menuSound.addEventListener("click", () => {
        // 防止快速连续点击
        if (menuSound.dataset.processing === "true") return;
        menuSound.dataset.processing = "true";
        
        // 使用requestAnimationFrame优化DOM操作
        requestAnimationFrame(() => {
            // 声音页面不使用动画
            menuContent.classList.add("show");
            menuSound.classList.add("show");
            soundPage.classList.add("show");
            menuImage.classList.remove("show");
            imagePage.classList.remove("show");
            historyToggle.classList.remove("show");
            historyContent.classList.remove("show");
            clearHistoryBtn.classList.remove("show");
            menuText.classList.remove("show");
            textPage.classList.remove("show");
            if (typeof renderHistory === 'function') {
                renderHistory();
            }
            
            // 重置处理状态
            setTimeout(() => {
                menuSound.dataset.processing = "false";
            }, 300);
        });
    });
}

// 在页面加载时初始化
initSound();