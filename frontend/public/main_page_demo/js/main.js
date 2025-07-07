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

                // 如果切换到高级设置页面，确保二级导航的指示器位置正确
                if (target.dataset.content === 'advanced-settings') {
                    const advNav = document.querySelector('.advanced-nav');
                    if (advNav) {
                        const initialActiveAdvLink = advNav.querySelector('.adv-nav-link.active');
                        if (initialActiveAdvLink) {
                           // 延迟一小段时间执行，确保CSS渲染完成
                           setTimeout(() => {
                                moveAdvIndicator(initialActiveAdvLink);
                           }, 50);
                        }
                    }
                }
            });
        });

        // 初始化指示器位置
        const initialActiveButton = settingsNav.querySelector('.nav-button.active');
        if (initialActiveButton) {
            moveIndicator(initialActiveButton);
        }

        // --- 高级设置页面内部的二级导航逻辑 ---
        const advancedNav = document.querySelector('.advanced-nav');
        if (advancedNav) {
            const advIndicator = advancedNav.querySelector('.adv-nav-indicator');
            const advNavLinks = advancedNav.querySelectorAll('.adv-nav-link');
            const advContentPages = document.querySelectorAll('.adv-content-page');

            function moveAdvIndicator(target) {
                if (!advIndicator || !target) return;
                // target.offsetTop是相对于父元素.advanced-nav的偏移量
                // .advanced-nav本身有padding，所以offsetTop已经是我们需要的正确值
                advIndicator.style.top = `${target.offsetTop}px`;
                advIndicator.style.height = `${target.offsetHeight}px`;
            }

            advNavLinks.forEach(link => {
                link.addEventListener('click', (e) => {
                    e.preventDefault(); // 阻止 a 标签的默认跳转行为

                    const currentActiveLink = advancedNav.querySelector('.adv-nav-link.active');
                    if (currentActiveLink) {
                        currentActiveLink.classList.remove('active');
                    }
                    const targetLink = e.currentTarget;
                    targetLink.classList.add('active');
                    moveAdvIndicator(targetLink); // 移动指示器

                    const targetContentId = targetLink.dataset.content;
                    advContentPages.forEach(page => {
                        if (page.id === targetContentId) {
                            page.classList.add('active');
                        } else {
                            page.classList.remove('active');
                        }
                    });
                });
            });

            // 初始化时移动到激活的链接
            const initialActiveAdvLink = advancedNav.querySelector('.adv-nav-link.active');
            if (initialActiveAdvLink) {
                // 同样，延迟执行以确保渲染完成
                setTimeout(() => moveAdvIndicator(initialActiveAdvLink), 50);
            }
        }
    }
}); 