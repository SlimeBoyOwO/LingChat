import os
from PIL import Image

# 输入文件夹路径（包含原始图片）
input_folder = "avatar"

# 输出文件夹路径（将保存压缩后的图片）
output_folder = "avatar_bk"

# 支持的图片扩展名
supported_extensions = ['.jpg', '.jpeg', '.png', '.bmp', '.gif', '.tiff']

# 创建输出文件夹（如果不存在）
os.makedirs(output_folder, exist_ok=True)

# 遍历输入文件夹中的所有文件
for filename in os.listdir(input_folder):
    # 检查文件扩展名
    ext = os.path.splitext(filename)[1].lower()
    if ext in supported_extensions:
        try:
            # 构建完整文件路径
            input_path = os.path.join(input_folder, filename)
            output_path = os.path.join(output_folder, filename)
            
            # 打开图片文件
            with Image.open(input_path) as img:
                # 计算新尺寸（原尺寸的一半）
                width, height = img.size
                new_size = (width // 2, height // 2)
                
                # 使用LANCZOS高质量下采样滤镜调整尺寸
                resized_img = img.resize(new_size, Image.LANCZOS)
                
                # 保存图片（保留原始格式和元数据）
                resized_img.save(output_path, quality=100, exif=img.info.get('exif', b''))
                
            print(f"成功压缩: {filename} ({width}x{height} -> {new_size[0]}x{new_size[1]})")
        
        except Exception as e:
            print(f"处理 {filename} 时出错: {str(e)}")

print("所有图片处理完成！")