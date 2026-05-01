/**
 * LingChat 主界面交互脚本
 * 功能：
 *  - 面板管理系统（设置/开始游戏/存档面板）
 *  - 鼠标移动视差效果（菜单倾斜 + 多层背景平移）
 *  - 星星粒子动画（闪烁 + 平移）
 *  - 流星雨动画
 */

document.addEventListener('DOMContentLoaded', () => {
    // ==================== DOM 元素获取 ====================
    const settingsButton = document.getElementById('settings-button');
    const mainContainer = document.querySelector('.main-container');
    const menuButtons = document.querySelectorAll('.menu-button');
    const startGameButton = menuButtons[1];   // 第二个按钮：开始游戏
    const loadSaveButton = menuButtons[2];    // 第三个按钮：存档
    const logo = document.querySelector('.logo');

    let activePanelId = null; // 当前打开的面板ID

    // ==================== 面板管理系统 ====================
    /**
     * 打开指定ID的面板
     * @param {string} panelId - 面板ID (如 'settings-panel')
     */
    const openPanel = (panelId) => {
        // 清理之前激活的面板类
        if (activePanelId) {
            const oldPanelType = activePanelId.replace('-panel', '');
            document.body.classList.remove('panel-active', `show-${oldPanelType}`);
        }

        // 确定面板类型
        let panelType;
        if (panelId === 'settings-panel') panelType = 'settings';
        else if (panelId === 'game-screen-panel') panelType = 'game-screen';
        else if (panelId === 'load-save-panel') panelType = 'load-save';
        else panelType = panelId.replace('-panel', '');

        // 添加激活类
        document.body.classList.add('panel-active', `show-${panelType}`);
        activePanelId = panelId;

        // 首次打开设置面板时初始化内部交互
        if (panelId === 'settings-panel') {
            const settingsPanel = document.getElementById('settings-panel');
            if (settingsPanel && !settingsPanel.dataset.initialized) {
                setupSettingsInteraction();
                settingsPanel.dataset.initialized = 'true';
            }
        }
        // 首次打开游戏面板时初始化内部交互
        if (panelId === 'game-screen-panel') {
            const gamePanel = document.getElementById('game-screen-panel');
            if (gamePanel && !gamePanel.dataset.initialized) {
                setupGameSelectionInteraction();
                gamePanel.dataset.initialized = 'true';
            }
        }
    };

    /**
     * 关闭当前激活的面板
     */
    const closePanel = () => {
        if (activePanelId) {
            let panelType;
            if (activePanelId === 'settings-panel') panelType = 'settings';
            else if (activePanelId === 'game-screen-panel') panelType = 'game-screen';
            else if (activePanelId === 'load-save-panel') panelType = 'load-save';
            else panelType = activePanelId.replace('-panel', '');

            document.body.classList.remove('panel-active', `show-${panelType}`);
            activePanelId = null;
        }
    };

    // ==================== 菜单按钮事件绑定 ====================
    settingsButton.addEventListener('click', (e) => {
        e.stopPropagation();
        openPanel('settings-panel');
    });

    if (startGameButton) {
        startGameButton.addEventListener('click', (e) => {
            e.stopPropagation();
            openPanel('game-screen-panel');
        });
    }

    if (loadSaveButton) {
        loadSaveButton.addEventListener('click', (e) => {
            e.stopPropagation();
            openPanel('load-save-panel');
        });
    }

    // 全局点击：点击面板外部或关闭按钮时关闭面板
    document.addEventListener('click', (event) => {
        if (!document.body.classList.contains('panel-active') || !activePanelId) return;

        const currentActivePanel = document.getElementById(activePanelId);
        if (!currentActivePanel) return;

        const clickInsidePanel = currentActivePanel.contains(event.target);
        const clickedCloseButton = event.target.closest('[data-close-panel]');

        if (clickedCloseButton && clickedCloseButton.dataset.closePanel === activePanelId) {
            closePanel();
        } else if (!clickInsidePanel) {
            closePanel();
        }
    });

    // ==================== 设置面板内部交互 ====================
    /**
     * 初始化设置面板内的导航、文本速度预览、二级导航
     */
    function setupSettingsInteraction() {
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

        function startTyping(speed) {
            clearInterval(typingInterval);
            if (!textSampleDisplay) return;
            textSampleDisplay.textContent = '';
            let i = 0;
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
            startTyping(textSpeedSlider.value);
        }

        // --- 主导航栏交互（顶部）---
        function moveIndicator(target) {
            if (!indicator || !target) return;
            indicator.style.left = `${target.offsetLeft}px`;
            indicator.style.width = `${target.offsetWidth}px`;
        }

        function switchPage(targetContentId) {
            pages.forEach(page => page.classList.toggle('active', page.id === targetContentId));
        }

        navButtons.forEach(button => {
            button.addEventListener('click', (e) => {
                const currentActive = settingsNav.querySelector('.nav-button.active');
                if (currentActive) currentActive.classList.remove('active');

                const target = e.currentTarget;
                target.classList.add('active');

                moveIndicator(target);
                switchPage(target.dataset.content);

                // 切换到文本页面时重新触发打字效果
                if (target.dataset.content === 'text-settings' && textSpeedSlider) {
                    startTyping(textSpeedSlider.value);
                }

                // 切换到高级设置页面时确保二级导航指示器位置正确
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
        if (initialActiveButton) moveIndicator(initialActiveButton);

        // --- 高级设置页面二级导航 ---
        const advancedNav = document.querySelector('.advanced-nav');
        if (advancedNav) {
            const advIndicator = advancedNav.querySelector('.adv-nav-indicator');
            const advNavLinks = advancedNav.querySelectorAll('.adv-nav-link');
            const advContentPages = document.querySelectorAll('.adv-content-page');

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
                    advContentPages.forEach(page => page.classList.toggle('active', page.id === targetContentId));
                });
            });

            const initialActiveAdvLink = advancedNav.querySelector('.adv-nav-link.active');
            if (initialActiveAdvLink && advIndicator) {
                advIndicator.classList.add('no-transition');
                moveAdvIndicator(initialActiveAdvLink);
                setTimeout(() => advIndicator.classList.remove('no-transition'), 0);
            }
        }
    }

    // ==================== 游戏选择面板交互 ====================
    function setupGameSelectionInteraction() {
        const gameNavButtons = document.querySelectorAll('.game-nav-btn');
        const gameContents = document.querySelectorAll('.game-category-content');

        gameNavButtons.forEach(button => {
            button.addEventListener('click', (e) => {
                e.preventDefault();

                gameNavButtons.forEach(btn => btn.classList.remove('active'));
                gameContents.forEach(content => content.classList.remove('active'));

                button.classList.add('active');
                const category = button.dataset.category;
                const targetContent = document.getElementById(`${category}-content`);
                if (targetContent) targetContent.classList.add('active');
            });
        });

        // 游戏项目点击事件（可扩展）
        const gameItems = document.querySelectorAll('.game-item');
        gameItems.forEach(item => {
            item.addEventListener('click', () => {
                const gameName = item.querySelector('h4').textContent;
                console.log(`选择了游戏: ${gameName}`);
                // 可在此添加游戏启动逻辑
            });
        });
    }

    // ==================== 菜单倾斜 & 多层平移视差效果 ====================
    const startPage = document.getElementById('start-page');
    const bgDiv = document.querySelector('.video-background');
    const characterImg = document.querySelector('.character-image');

    // 视差参数（可调整以改变层次感）
    const MAX_ANGLE = 7;                // 菜单最大倾斜角度
    const VERTICAL_SCALE = 0.5;          // 垂直倾斜缩放系数
    const CHAR_MAX_SHIFT = 20;           // 人物最大平移（中等）
    const BG_MAX_SHIFT = 12;             // 背景最大平移（最慢）
    const STARS_MAX_SHIFT = 40;          // 星星最大平移（最快）

    function updateTilt(e) {
        if (!startPage || !bgDiv || !characterImg) return;

        // 面板打开时复位所有效果
        if (document.body.classList.contains('panel-active')) {
            startPage.style.transform = 'rotateX(0deg) rotateY(0deg)';
            bgDiv.style.transform = 'translateX(0)';
            characterImg.style.transform = 'translate(-50%, -50%)';
            if (starsLayer) starsLayer.style.transform = 'translateX(0)';
            return;
        }

        const centerX = window.innerWidth / 2;
        const centerY = window.innerHeight / 2;
        const mouseX = e.clientX;
        const mouseY = e.clientY;
        const offsetX = (mouseX - centerX) / centerX;
        const offsetY = (mouseY - centerY) / centerY;

        // 菜单倾斜（反向：鼠标右移，菜单右边翘起）
        const rotateY = -offsetX * MAX_ANGLE;
        const rotateX = -offsetY * MAX_ANGLE * VERTICAL_SCALE;
        startPage.style.transform = `rotateX(${rotateX}deg) rotateY(${rotateY}deg)`;

        // 背景/人物平移（反向：鼠标右移，图片左移）
        const charShift = -offsetX * CHAR_MAX_SHIFT;
        const bgShift = -offsetX * BG_MAX_SHIFT;
        characterImg.style.transform = `translate(-50%, -50%) translateX(${charShift}px)`;
        bgDiv.style.transform = `translateX(${bgShift}px)`;

        // 星星平移
        const starsShift = -offsetX * STARS_MAX_SHIFT;
        if (starsLayer) starsLayer.style.transform = `translateX(${starsShift}px)`;
    }

    document.addEventListener('mousemove', updateTilt);
    document.addEventListener('mouseleave', () => {
        if (startPage && bgDiv && characterImg) {
            startPage.style.transform = 'rotateX(0deg) rotateY(0deg)';
            bgDiv.style.transform = 'translateX(0)';
            characterImg.style.transform = 'translate(-50%, -50%)';
            if (starsLayer) starsLayer.style.transform = 'translateX(0)';
        }
    });

    // ==================== 星星粒子系统 ====================
    const starsLayer = document.querySelector('.stars-layer');
    const starsCanvas = document.getElementById('stars-canvas');
    let starsCtx = starsCanvas ? starsCanvas.getContext('2d') : null;
    let starsPositions = [];
    const STARS_COUNT = 80;               // 星星数量
    const FLICKER_SPEED = 0.003;           // 闪烁速度

    let animationFrameId = null;

    // 绘制一个四角星（带渐变和光晕）
    function drawStar(ctx, x, y, size, rotation) {
        ctx.save();
        ctx.translate(x, y);
        ctx.rotate(rotation);

        // 径向渐变：中心亮白 → 边缘透明暖黄
        const gradient = ctx.createRadialGradient(0, 0, 0, 0, 0, size * 1.5);
        gradient.addColorStop(0, '#ffffff');                 // 中心纯白
        gradient.addColorStop(0.6, 'rgba(255, 240, 200, 0.9)'); // 中间暖黄
        gradient.addColorStop(1, 'rgba(255, 200, 100, 0)');   // 边缘完全透明

        // 外发光阴影
        ctx.shadowColor = 'rgba(255, 255, 200, 0.8)';
        ctx.shadowBlur = 20;
        ctx.shadowOffsetX = 0;
        ctx.shadowOffsetY = 0;

        ctx.fillStyle = gradient;

        // 绘制四角星路径
        ctx.beginPath();
        for (let i = 0; i < 8; i++) {
            const angle = (i * Math.PI) / 4;
            const radius = i % 2 === 0 ? size : size * 0.4;
            const dx = Math.cos(angle) * radius;
            const dy = Math.sin(angle) * radius;
            if (i === 0) ctx.moveTo(dx, dy);
            else ctx.lineTo(dx, dy);
        }
        ctx.closePath();
        ctx.fill();

        ctx.restore();
    }

    // 绘制一个圆形（带渐变和光晕）
    function drawCircle(ctx, x, y, size) {
        ctx.save();

        // 径向渐变
        const gradient = ctx.createRadialGradient(x, y, 0, x, y, size * 1.5);
        gradient.addColorStop(0, '#ffffff');
        gradient.addColorStop(0.6, 'rgba(255, 240, 200, 0.9)');
        gradient.addColorStop(1, 'rgba(255, 200, 100, 0)');

        ctx.shadowColor = 'rgba(255, 255, 200, 0.8)';
        ctx.shadowBlur = 20;
        ctx.shadowOffsetX = 0;
        ctx.shadowOffsetY = 0;

        ctx.fillStyle = gradient;

        ctx.beginPath();
        ctx.arc(x, y, size, 0, Math.PI * 2);
        ctx.fill();

        ctx.restore();
    }

    // 生成星星位置（只在上半屏幕）
    function generateStars() {
        if (!starsCanvas || !starsCtx) return;
        const w = window.innerWidth;
        const h = window.innerHeight;
        starsCanvas.width = w;
        starsCanvas.height = h;

        starsPositions = [];
        for (let i = 0; i < STARS_COUNT; i++) {
            starsPositions.push({
                x: Math.random() * w,
                y: Math.random() * (h * 0.33),        // 上三分之一
                size: Math.random() * 5 + 2,           // 2~7px
                baseOpacity: Math.random() * 0.5 + 0.5, // 0.5~1.0
                rotation: Math.random() * Math.PI * 2,
                type: Math.random() > 0.2 ? 'star' : 'circle' // 80% 星形，20% 圆形
            });
        }
        drawStars();
    }

    // 绘制星星（带闪烁效果）
    function drawStars() {
        if (!starsCtx) return;
        const w = window.innerWidth;
        const h = window.innerHeight;
        starsCtx.clearRect(0, 0, w, h);

        const now = Date.now();

        starsPositions.forEach(pos => {
            // 闪烁计算
            const flicker = 0.6 + 0.4 * Math.sin(now * FLICKER_SPEED + pos.x);
            let opacity = pos.baseOpacity * flicker;
            opacity = Math.min(opacity, 1.0);

            // 应用整体透明度（影响渐变和阴影）
            starsCtx.globalAlpha = opacity;

            if (pos.type === 'star') {
                drawStar(starsCtx, pos.x, pos.y, pos.size, pos.rotation);
            } else {
                drawCircle(starsCtx, pos.x, pos.y, pos.size);
            }
        });
    }

// 动画循环
function flickerAnimation() {
    drawStars();
    animationFrameId = requestAnimationFrame(flickerAnimation);
}

// 窗口大小变化时重新生成星星
window.addEventListener('resize', () => {
    if (animationFrameId) {
        cancelAnimationFrame(animationFrameId);
        animationFrameId = null;
    }
    generateStars();
    flickerAnimation();
    if (starsLayer) starsLayer.style.transform = 'translateX(0)';
});

    // 动画循环
    function flickerAnimation() {
        drawStars();
        animationFrameId = requestAnimationFrame(flickerAnimation);
    }

    // 窗口大小变化时重新生成星星
    window.addEventListener('resize', () => {
        if (animationFrameId) {
            cancelAnimationFrame(animationFrameId);
            animationFrameId = null;
        }
        generateStars();
        flickerAnimation();
        if (starsLayer) starsLayer.style.transform = 'translateX(0)';
    });

    // ==================== 流星雨系统 ====================
    let activeMeteors = [];

    const METEOR_CONFIG = {
        MAX_COUNT: 3,
        GENERATE_INTERVAL: 1000,
        MIN_DISTANCE: 800,
        DURATION_MIN: 9,
        DURATION_MAX: 14,
        START_X_MIN: 800,
        START_X_MAX: 1200,
        START_Y_RANGE: 0.25,
        END_X_MIN: -200,
        END_X_MAX: -400,
        END_Y_OFFSET: 1,
        CONTROL_X_RATIO_MIN: 0.5,
        CONTROL_X_RATIO_MAX: 0.9,
        CONTROL_Y_OFFSET: -0.1
    };

    function getValidStartY() {
        const h = window.innerHeight;
        const maxAttempts = 120;
        let attempts = 0;
        while (attempts < maxAttempts) {
            const startY = -Math.random() * (h * METEOR_CONFIG.START_Y_RANGE);
            let tooClose = false;
            for (let meteor of activeMeteors) {
                const dy = Math.abs(startY - meteor.startY);
                if (dy < METEOR_CONFIG.MIN_DISTANCE) {
                    tooClose = true;
                    break;
                }
            }
            if (!tooClose) return startY;
            attempts++;
        }
        return -Math.random() * (h * METEOR_CONFIG.START_Y_RANGE);
    }

    function createMeteor() {
        const svg = document.getElementById('meteor-svg');
        if (!svg) return null;

        const w = window.innerWidth;
        const h = window.innerHeight;
        const startY = getValidStartY();
        const startX = w + METEOR_CONFIG.START_X_MIN + Math.random() * (METEOR_CONFIG.START_X_MAX - METEOR_CONFIG.START_X_MIN);
        const endX = METEOR_CONFIG.END_X_MIN + Math.random() * (METEOR_CONFIG.END_X_MAX - METEOR_CONFIG.END_X_MIN);
        const endY = startY + h * METEOR_CONFIG.END_Y_OFFSET;
        const controlX = w * (METEOR_CONFIG.CONTROL_X_RATIO_MIN + Math.random() * (METEOR_CONFIG.CONTROL_X_RATIO_MAX - METEOR_CONFIG.CONTROL_X_RATIO_MIN));
        const controlY = startY + h * METEOR_CONFIG.CONTROL_Y_OFFSET;

        const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
        const d = `M ${startX} ${startY} Q ${controlX} ${controlY} ${endX} ${endY}`;
        path.setAttribute("d", d);
        path.setAttribute("class", "meteor-path");

        const duration = METEOR_CONFIG.DURATION_MIN + Math.random() * (METEOR_CONFIG.DURATION_MAX - METEOR_CONFIG.DURATION_MIN);
        path.style.animation = `meteor-move ${duration}s linear forwards`;

        svg.appendChild(path);

        const meteorInfo = {
            path: path,
            startY: startY,
            timeoutId: setTimeout(() => {
                if (path.parentNode) path.remove();
                const index = activeMeteors.indexOf(meteorInfo);
                if (index > -1) activeMeteors.splice(index, 1);
            }, duration * 1000)
        };

        activeMeteors.push(meteorInfo);
        return meteorInfo;
    }

    function updateMeteorShower() {
        activeMeteors = activeMeteors.filter(meteor => meteor.path.parentNode !== null);
        if (activeMeteors.length < METEOR_CONFIG.MAX_COUNT) {
            createMeteor();
        }
    }

    function startMeteorShower() {
        if (window.meteorInterval) clearInterval(window.meteorInterval);
        for (let i = 0; i < 3; i++) {
            setTimeout(() => {
                if (activeMeteors.length < METEOR_CONFIG.MAX_COUNT) {
                    createMeteor();
                }
            }, i * 300);
        }
        window.meteorInterval = setInterval(updateMeteorShower, METEOR_CONFIG.GENERATE_INTERVAL);
    }

    // ==================== 启动所有效果 ====================
    generateStars();
    flickerAnimation();
    startMeteorShower();

    // Logo 点击跳转 GitHub
    if (logo) {
        logo.addEventListener('click', () => {
            window.open('https://github.com/SlimeBoyOwO/LingChat', '_blank');
        });
    }
});