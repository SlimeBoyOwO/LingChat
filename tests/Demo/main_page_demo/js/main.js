document.addEventListener('DOMContentLoaded', () => {
    // --- DOM 元素获取 ---
    const settingsButton = document.getElementById('settings-button');
    const mainContainer = document.querySelector('.main-container');
    let settingsPanel; // 设置面板元素，延迟初始化

    // --- 设置面板的显示与隐藏 ---
    const showSettings = () => {
        settingsPanel = document.getElementById('settings-panel');
        if (settingsPanel) {
            settingsPanel.classList.add('active');
            // 首次显示时，初始化面板内部的交互逻辑
            if (!settingsPanel.dataset.initialized) {
                setupSettingsInteraction();
                settingsPanel.dataset.initialized = 'true';
            }
        }
    };
    
    const hideSettings = () => {
        settingsPanel = document.getElementById('settings-panel');
        if (settingsPanel) settingsPanel.classList.remove('active');
    };
    
    // “设置”按钮点击事件：显示面板
    settingsButton.addEventListener('click', (event) => {
        event.stopPropagation(); // 阻止事件冒泡，防止触发全局点击事件
        showSettings();
    });
    
    // 全局点击事件：处理“点击外部”或“关闭按钮”来隐藏面板
    document.addEventListener('click', (event) => {
        settingsPanel = document.getElementById('settings-panel');
        if (!settingsPanel || !settingsPanel.classList.contains('active')) {
            return; // 如果面板未激活，则不执行任何操作
        }

        const clickInsidePanel = settingsPanel.contains(event.target);
        const isCloseButton = event.target.closest('#close-settings-button');
        
        if (isCloseButton || !clickInsidePanel) {
            hideSettings();
        }
    });

    /**
     * @description 初始化设置面板内的所有交互，包括主导航、文本速度预览和高级设置的二级导航。
     * 该函数只应在面板首次打开时执行一次。
     */
    function setupSettingsInteraction() {
        // --- 元素获取 ---
        const settingsNav = document.querySelector('.settings-nav');
        if (!settingsNav) return; 

        const indicator = settingsNav.querySelector('.nav-indicator');
        const navButtons = settingsNav.querySelectorAll('.nav-button');
        const pages = document.querySelectorAll('.settings-page');
        const textSpeedSlider = document.getElementById('text-speed-slider');
        const textSampleDisplay = document.getElementById('typed-text-sample');

        // --- 文本速度预览逻辑 ---
        const sampleText = "Ling Chat：测试文本显示速度";
        let typingInterval;

        /**
         * @description 根据滑块速度值，模拟打字机效果。
         * @param {number} speed - 速度值 (1-100)，值越大速度越快。
         */
        function startTyping(speed) {
            clearInterval(typingInterval);
            if (!textSampleDisplay) return;

            textSampleDisplay.textContent = '';
            let i = 0;
            // 将滑块值 (1-100) 线性映射到打字延迟 (200ms - 10ms)。
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
            textSpeedSlider.addEventListener('input', (e) => startTyping(e.target.value));
            startTyping(textSpeedSlider.value); // 初始触发一次
        }

        // --- 主导航栏交互 (顶部) ---
        
        /**
         * @description 将顶部导航的指示器移动到目标按钮。
         * @param {HTMLElement} target - 目标按钮元素。
         */
        function moveIndicator(target) {
            if (!indicator || !target) return;
            indicator.style.left = `${target.offsetLeft}px`;
            indicator.style.width = `${target.offsetWidth}px`;
        }
        
        /**
         * @description 根据 data-content 属性切换显示的页面。
         * @param {string} targetContentId - 目标页面的ID。
         */
        function switchPage(targetContentId) {
            pages.forEach(page => {
                page.classList.toggle('active', page.id === targetContentId);
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
                            moveAdvIndicator(initialActiveAdvLink);
                        }
                    }
                }
            });
        });

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

            /**
             * @description 将高级设置的二级导航指示器移动到目标链接。
             * @param {HTMLElement} target - 目标链接元素。
             */
            function moveAdvIndicator(target) {
                if (!advIndicator || !target) return;
                advIndicator.style.top = `${target.offsetTop}px`;
                advIndicator.style.height = `${target.offsetHeight}px`;
            }

            advNavLinks.forEach(link => {
                link.addEventListener('click', (e) => {
                    e.preventDefault();

                    const currentActiveLink = advancedNav.querySelector('.adv-nav-link.active');
                    if (currentActiveLink) currentActiveLink.classList.remove('active');
                    
                    const targetLink = e.currentTarget;
                    targetLink.classList.add('active');
                    moveAdvIndicator(targetLink);

                    const targetContentId = targetLink.dataset.content;
                    advContentPages.forEach(page => {
                        page.classList.toggle('active', page.id === targetContentId);
                    });
                });
            });

            // 初始化二级导航指示器位置
            const initialActiveAdvLink = advancedNav.querySelector('.adv-nav-link.active');
            if (initialActiveAdvLink && advIndicator) {
                advIndicator.classList.add('no-transition');
                moveAdvIndicator(initialActiveAdvLink);
                setTimeout(() => {
                    advIndicator.classList.remove('no-transition');
                }, 0);
            }
        }
    }
}); 