document.body.classList.add('loading-active');

// 简单的进度条动画，90%后背景透明，100%后模糊消失
let progress = 0;
const bar = document.getElementById('progress-bar');
const loader = document.getElementById('loader');
let bgTransparent = false;
let blurRemoved = false;
function animateProgress() {
    progress += Math.random() * 8 + 2; // 随机增长
    if (progress > 100) progress = 100;
    bar.style.width = progress + '%';
    // 90%后背景透明
    if (progress >= 90 && !bgTransparent) {
        loader.classList.add('bg-transparent');
        bgTransparent = true;
    }
    // 100%后模糊消失并删除loader
    if (progress >= 100 && !blurRemoved) {
        setTimeout(() => {
            loader.classList.add('no-blur');
            setTimeout(() => {
                loader.classList.add('hidden');
                setTimeout(() => {
                    loader.remove();
                    document.body.classList.remove('loading-active');
                }, 800); // 等待opacity过渡完成后移除DOM
            }, 250); // 背景透明后0.25s再去除模糊
        }, 250);
        blurRemoved = true;
    }
    if (progress < 100) {
        const delay = Math.random() * 700 + 100; // 100~800ms
        setTimeout(animateProgress, delay);
    }
}
animateProgress(); 