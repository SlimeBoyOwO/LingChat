document.addEventListener('DOMContentLoaded', () => {
    const settingsButton = document.getElementById('settings-button');
    const mainContainer = document.querySelector('.main-container');
    let settingsPanel; // 延迟初始化

    // --- 旧的显示/隐藏逻辑 ---
    const showSettings = () => {
        settingsPanel = document.getElementById('settings-panel');
        if (settingsPanel) settingsPanel.classList.add('active');
        setupSettingsInteraction(); // 显示后初始化内部交互
    };
    
    const hideSettings = () => {
        settingsPanel = document.getElementById('settings-panel');
        if (settingsPanel) settingsPanel.classList.remove('active');
    };
    
    settingsButton.addEventListener('click', (event) => {
        // 阻止事件冒泡，防止立即触发下面的document监听器
        event.stopPropagation();
        showSettings();
    });
    
    // 全局点击事件监听器，处理"点击外部关闭"和"关闭按钮"
    document.addEventListener('click', (event) => {
        settingsPanel = document.getElementById('settings-panel');

        // 如果面板不存在或未激活，则不执行任何操作
        if (!settingsPanel || !settingsPanel.classList.contains('active')) {
            return;
        }

        // 检查点击是否发生在设置面板内部
        const clickInsidePanel = settingsPanel.contains(event.target);
        
        // 如果点击发生在"关闭"按钮上，或者点击发生在面板的外部
        if (event.target.closest('#close-settings-button') || !clickInsidePanel) {
            hideSettings();
        }
    });

    // --- 新的设置面板内部交互逻辑 ---
    function setupSettingsInteraction() {
        const settingsNav = document.querySelector('.settings-nav');
        if (!settingsNav) return; // 如果面板不存在则退出

        const indicator = settingsNav.querySelector('.nav-indicator');
        const navButtons = settingsNav.querySelectorAll('.nav-button');
        const pages = document.querySelectorAll('.settings-page');

        // --- 文本速度预览逻辑 ---
        const textSpeedSlider = document.getElementById('text-speed-slider');
        const textSampleDisplay = document.getElementById('typed-text-sample');
        const sampleText = "Ling Chat：测试文本显示速度";
        let typingInterval;

        function startTyping(speed) {
            clearInterval(typingInterval); // 清除上一个定时器
            if (!textSampleDisplay) return;

            textSampleDisplay.textContent = '';
            let i = 0;
            // 将滑块值 (1-100) 映射到打字延迟 (200ms - 10ms)
            const maxDelay = 200;
            const minDelay = 10;
            const delay = maxDelay - ((speed - 1) / 99) * (maxDelay - minDelay);

            typingInterval = setInterval(() => {
                if (i < sampleText.length) {
                    textSampleDisplay.textContent += sampleText.charAt(i);
                    i++;
                } else {
                    clearInterval(typingInterval);
                }
            }, delay);
        }

        if (textSpeedSlider) {
            textSpeedSlider.addEventListener('input', (e) => {
                startTyping(e.target.value);
            });
            // 初始触发一次
            startTyping(textSpeedSlider.value);
        }
        // --- 结束 ---

        function moveIndicator(target) {
            // 现在是水平移动
            indicator.style.left = `${target.offsetLeft}px`;
            indicator.style.width = `${target.offsetWidth}px`;
        }

        function switchPage(targetContentId) {
            pages.forEach(page => {
                if (page.id === targetContentId) {
                    page.classList.add('active');
                } else {
                    page.classList.remove('active');
                }
            });
        }

        navButtons.forEach(button => {
            button.addEventListener('click', (e) => {
                const currentActive = settingsNav.querySelector('.nav-button.active');
                if (currentActive) currentActive.classList.remove('active');
                
                const target = e.currentTarget;
                target.classList.add('active');
                
                moveIndicator(target);
                switchPage(target.dataset.content);
                
                // 如果切换到文本页面，重新触发打字效果
                if (target.dataset.content === 'text-settings' && textSpeedSlider) {
                    startTyping(textSpeedSlider.value);
                }
            });
        });

        // 初始化指示器位置
        const initialActiveButton = settingsNav.querySelector('.nav-button.active');
        if (initialActiveButton) {
            moveIndicator(initialActiveButton);
        }
    }
}); 