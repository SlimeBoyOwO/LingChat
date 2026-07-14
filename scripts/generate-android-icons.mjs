import sharp from 'sharp';
import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, '..');

// 支持多种源图标路径
const possibleSrcPaths = [
  path.join(root, 'src-tauri', 'icons', 'icon.png'),
  path.join(root, 'src-tauri', 'icons', 'icon.ico'),
  path.join(root, 'src-tauri', 'icons', 'icon.icns'),
];

const srcPath = possibleSrcPaths.find(p => fs.existsSync(p));
if (!srcPath) {
  console.error('❌ 未找到源图标文件，请确保 src-tauri/icons/icon.png 存在');
  console.error('   支持的格式: PNG, ICO, ICNS');
  process.exit(1);
}

const base = path.join(root, 'src-tauri', 'gen', 'android', 'app', 'src', 'main', 'res');

// 确保输出目录存在
if (!fs.existsSync(base)) {
  console.log('⚠️  Android 目录不存在，跳过生成');
  console.log('   请先运行: pnpm tauri android init');
  process.exit(0);
}

const fgSizes = { mdpi: 108, hdpi: 162, xhdpi: 216, xxhdpi: 324, xxxhdpi: 432 };
const FG_CONTENT_RATIO = 0.60;
const legacySizes = { mdpi: 48, hdpi: 72, xhdpi: 96, xxhdpi: 144, xxxhdpi: 192 };

async function generateAndroidIcons() {
  try {
    const srcMeta = await sharp(srcPath).metadata();
    console.log(`✅ 源图标: ${srcMeta.width}x${srcMeta.height} - ${srcPath}`);

    // 生成传统图标
    console.log('\n📱 生成传统图标...');
    for (const [density, sz] of Object.entries(legacySizes)) {
      const buf = await sharp(srcPath)
        .resize(sz, sz, { fit: 'cover' })
        .png()
        .toBuffer();
      
      const outDir = path.join(base, `mipmap-${density}`);
      fs.mkdirSync(outDir, { recursive: true });
      
      fs.writeFileSync(path.join(outDir, 'ic_launcher.png'), buf);
      fs.writeFileSync(path.join(outDir, 'ic_launcher_round.png'), buf);
      console.log(`  ✅ ${density}: ${sz}x${sz}`);
    }

    // 生成自适应图标前景层
    console.log('\n🎨 生成自适应图标前景层...');
    for (const [density, canvasSz] of Object.entries(fgSizes)) {
      const contentSz = Math.round(canvasSz * FG_CONTENT_RATIO);
      
      const composite = await sharp(srcPath)
        .resize(contentSz, contentSz, { fit: 'cover' })
        .toBuffer();

      const fg = await sharp({
        create: {
          width: canvasSz,
          height: canvasSz,
          channels: 4,
          background: { r: 0, g: 0, b: 0, alpha: 0 },
        },
      })
        .composite([{
          input: composite,
          left: Math.round((canvasSz - contentSz) / 2),
          top: Math.round((canvasSz - contentSz) / 2),
        }])
        .png()
        .toBuffer();

      const outDir = path.join(base, `mipmap-${density}`);
      fs.mkdirSync(outDir, { recursive: true });
      fs.writeFileSync(path.join(outDir, 'ic_launcher_foreground.png'), fg);
      console.log(`  ✅ ${density}: ${canvasSz}x${canvasSz} (内容 ${contentSz}px)`);
    }

    console.log('\n✨ Android 图标生成完成！');
  } catch (err) {
    console.error('❌ 生成失败:', err.message);
    process.exit(1);
  }
}

generateAndroidIcons();
