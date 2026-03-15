import os
import sys
from PIL import Image
import argparse
from pathlib import Path


def convert_to_webp(input_path, output_path=None, quality=80, lossless=False):
    """
    将图片转换为WebP格式，保持PNG透明度
    
    :param input_path: 输入图片路径
    :param output_path: 输出WebP图片路径，如果为None则自动生成
    :param quality: 质量参数，0-100，仅在有损压缩时有效
    :param lossless: 是否使用无损压缩
    :return: 转换后的文件路径
    """
    if not os.path.exists(input_path):
        raise FileNotFoundError(f"输入文件不存在: {input_path}")
    
    # 获取文件扩展名
    _, ext = os.path.splitext(input_path)
    if ext.lower() not in ['.jpg', '.jpeg', '.png', '.bmp', '.tiff', '.tif', '.gif']:
        raise ValueError(f"不支持的文件格式: {ext}")
    
    # 打开图片
    try:
        with Image.open(input_path) as img:
            # 获取图片信息
            img_format = img.format
            
            # 如果是PNG格式，需要检查是否有透明度
            has_transparency = False
            if img_format == 'PNG':
                if img.mode in ('RGBA', 'LA') or 'transparency' in img.info:
                    has_transparency = True
                print(f"检测到PNG图片，透明度: {'是' if has_transparency else '否'}")
            
            # 如果没有指定输出路径，则生成默认路径
            if output_path is None:
                base_path = Path(input_path).stem
                output_path = str(Path(input_path).with_name(base_path + '.webp'))
            
            # 确保输出目录存在
            output_dir = Path(output_path).parent
            output_dir.mkdir(parents=True, exist_ok=True)
            
            # 准备保存选项
            save_kwargs = {
                'format': 'WEBP',
                'quality': quality if not lossless else 100,  # 无损模式下忽略质量参数
                'lossless': lossless
            }
            
            # 如果原图有透明度，确保保存时保留透明度
            if has_transparency:
                if img.mode != 'RGBA':
                    img = img.convert('RGBA')
                print(f"保留透明度信息...")
            
            # 保存为WebP格式
            img.save(output_path, **save_kwargs)
            
            # 获取文件大小信息
            input_size = os.path.getsize(input_path)
            output_size = os.path.getsize(output_path)
            compression_ratio = (1 - output_size/input_size) * 100 if input_size > 0 else 0
            
            print(f"✓ 成功转换: {input_path} -> {output_path}")
            print(f"  文件大小: {input_size} bytes -> {output_size} bytes (压缩率: {compression_ratio:.1f}%)")
            return output_path
    except Exception as e:
        print(f"✗ 转换失败: {input_path}")
        print(f"  错误详情: {str(e)}")
        raise


def batch_convert(input_folder, output_folder=None, quality=80, lossless=False, supported_formats=None):
    """
    批量转换文件夹中的所有图片
    
    :param input_folder: 输入文件夹路径
    :param output_folder: 输出文件夹路径
    :param quality: 质量参数
    :param lossless: 是否使用无损压缩
    :param supported_formats: 支持的输入格式列表
    """
    if supported_formats is None:
        supported_formats = {'.jpg', '.jpeg', '.png', '.bmp', '.tiff', '.tif', '.gif'}
    
    input_folder_path = Path(input_folder)
    if not input_folder_path.is_dir():
        raise ValueError(f"输入路径不是一个有效的文件夹: {input_folder}")
    
    if output_folder is None:
        output_folder_path = input_folder_path
    else:
        output_folder_path = Path(output_folder)
        output_folder_path.mkdir(parents=True, exist_ok=True)
    
    # 获取所有支持的文件
    files = []
    for ext in supported_formats:
        files.extend(list(input_folder_path.glob(f'*{ext}')))
        files.extend(list(input_folder_path.glob(f'*{ext.upper()}')))
    
    if not files:
        print(f"在 {input_folder} 中没有找到支持的图片文件")
        return
    
    print(f"找到 {len(files)} 个图片文件，开始批量转换...")
    
    success_count = 0
    fail_count = 0
    
    for file_path in files:
        try:
            output_filename = file_path.with_suffix('.webp').name
            output_path = output_folder_path / output_filename
            
            convert_to_webp(str(file_path), str(output_path), quality, lossless)
            success_count += 1
        except Exception as e:
            print(f"  跳过文件: {file_path.name} ({str(e)})")
            fail_count += 1
    
    print(f"\n批量转换完成!")
    print(f"成功: {success_count}, 失败: {fail_count}")


def main():
    parser = argparse.ArgumentParser(
        description='将图片转换为WebP格式',
        formatter_class=argparse.RawTextHelpFormatter
    )
    parser.add_argument('input', help='输入图片路径或文件夹路径')
    parser.add_argument('-o', '--output', help='输出WebP图片路径或文件夹路径')
    parser.add_argument('-q', '--quality', type=int, default=93, help='WebP质量 (0-100)，默认为93')
    parser.add_argument('--lossless', action='store_true', help='使用无损压缩\n注意: 无损压缩时，质量参数会被忽略')
    parser.add_argument('-b', '--batch', action='store_true', help='批量转换模式')
    
    args = parser.parse_args()
    
    # 验证质量参数
    if args.quality < 0 or args.quality > 100:
        print("错误: 质量参数必须在0-100之间")
        sys.exit(1)
    
    try:
        if args.batch:
            # 批量转换模式
            batch_convert(
                input_folder=args.input,
                output_folder=args.output,
                quality=args.quality,
                lossless=args.lossless
            )
        else:
            # 单个文件转换模式
            convert_to_webp(
                input_path=args.input,
                output_path=args.output,
                quality=args.quality,
                lossless=args.lossless
            )
    except KeyboardInterrupt:
        print("\n用户取消了操作")
        sys.exit(1)
    except Exception as e:
        print(f"错误: {str(e)}")
        sys.exit(1)


if __name__ == "__main__":
    main()