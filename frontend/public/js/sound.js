function initSound() {
    // 切换显示/隐藏
    menuSound.addEventListener("click", () => {
        menuSound.classList.add("show");
        soundPage.classList.add("show");
        menuImage.classList.remove("show");
        imagePage.classList.remove("show");
        historyToggle.classList.remove("show");
        historyContent.classList.remove("show");
        clearHistoryBtn.classList.remove("show");
        menuText.classList.remove("show");
        textPage.classList.remove("show");
        if (soundPage.classList.contains("show")) {
            renderHistory();
        }
    });
}

// 在页面加载时初始化
initSound();