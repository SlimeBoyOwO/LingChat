import os
import sys

# --- 配置常量 ---

# 脚本文件名 (自动获取)
SCRIPT_FILENAME = os.path.basename(__file__)

# 生成的 Markdown 文件名
OUTPUT_MD_FILENAME = "project_summary.md"

# 需要排除的目录和文件 (basenames)
# .DS_Store 是 macOS 系统文件
# .git 是版本控制目录
# __pycache__ 是 Python 编译的字节码缓存
# venv, env, .venv 是常见的 Python 虚拟环境目录
# .vscode, .idea 是 IDE 配置文件目录
# node_modules 是 Node.js 依赖包目录 (前端项目常见)
# dist, build, out, public 是常见的前端项目构建输出目录
# .parcel-cache, .next, .nuxt 是前端框架的缓存/构建目录
# tmp, temp 是临时文件目录
EXCLUDED_ITEMS = {
    '.git', '__pycache__', 'venv', 'env', '.venv', '.vscode', '.idea', '.DS_Store',
    'node_modules', 'dist', 'build', 'out', 'public', '.parcel-cache', '.next', '.nuxt',
    'tmp', 'temp', 'coverage' # coverage reports
}

# 对于这些扩展名的文件，如果数量过多，则进行省略处理
ELLIPSIS_EXTENSIONS = {
    '.png', '.jpg', '.jpeg', '.gif', '.bmp', '.svg',  # 图片
    '.ico', # 图标文件
    '.log', '.tmp', '.temp',                         # 日志和临时文件 (重复，但在此保持一致性)
    '.o', '.obj', '.dll', '.so', '.exe', '.bin',      # 编译产物和二进制文件
    '.data', '.dat',                                 # 数据文件
    '.pth', '.pt', '.ckpt', '.h5', '.onnx',          # 模型权重文件 (如果前端项目包含ML/AI模型)
    '.zip', '.tar', '.gz', '.rar',                   # 压缩文件
    '.pdf', '.doc', '.docx', '.xls', '.xlsx', '.ppt', '.pptx', # 文档
    '.mp4', '.mov', '.avi', '.webm', # 视频文件
    '.mp3', '.wav', '.ogg' # 音频文件
}
# 省略处理的阈值，超过这个数量的文件会被折叠
ELLIPSIS_THRESHOLD = 3 # 最多显示3个此类文件，其余用 "..." 表示

# 需要提取内容并显示在概览中的代码文件扩展名
TARGET_CODE_EXTENSIONS = {'.html', '.css', '.js'}

# --- 辅助函数 ---

def get_project_root():
    """获取项目根目录 (即脚本所在的目录)"""
    return os.path.dirname(os.path.abspath(__file__))

def generate_file_tree(root_dir, script_name, md_name):
    """
    生成项目文件结构树。

    参数:
    root_dir (str): 项目根目录的绝对路径。
    script_name (str): 要排除的脚本文件名。
    md_name (str): 要排除的生成的 markdown 文件名。

    返回:
    str: 表示文件树的字符串。
    """
    tree_lines = [f"{os.path.basename(root_dir)}/"] # 树的起始，项目根目录名
    
    # 将脚本名和md文件名加入排除列表 (仅basename)
    current_exclusions = EXCLUDED_ITEMS.copy()
    current_exclusions.add(script_name)
    current_exclusions.add(md_name)

    def recurse_dir(current_path, prefix):
        """
        递归地遍历目录并构建树形结构。

        参数:
        current_path (str): 当前正在扫描的目录路径。
        prefix (str): 用于表示层级关系的前缀字符串 (例如 "│   ", "└── ")。
        """
        try:
            entries = os.listdir(current_path)
        except PermissionError:
            tree_lines.append(f"{prefix}└── [无法访问:权限不足] {os.path.basename(current_path)}/")
            return
        
        # 过滤掉需要排除的项
        entries = [e for e in entries if e not in current_exclusions]
        entries.sort() # 保证顺序一致性

        # 分离目录和文件
        dirs = [e for e in entries if os.path.isdir(os.path.join(current_path, e))]
        files = [e for e in entries if os.path.isfile(os.path.join(current_path, e))]

        # 对文件进行省略处理
        display_files = []
        files_by_ext_map = {} # key: extension, value: list of filenames

        # 不需要省略处理的文件类型，直接加入
        other_files = []

        for f_name in files:
            ext = os.path.splitext(f_name)[1].lower()
            if ext in ELLIPSIS_EXTENSIONS:
                if ext not in files_by_ext_map:
                    files_by_ext_map[ext] = []
                files_by_ext_map[ext].append(f_name)
            else:
                other_files.append(f_name)
        
        # 处理需要省略的文件类型
        for ext in sorted(files_by_ext_map.keys()): # 按扩展名排序，保证一致性
            file_list = files_by_ext_map[ext]
            if len(file_list) > ELLIPSIS_THRESHOLD:
                display_files.extend(file_list[:ELLIPSIS_THRESHOLD])
                display_files.append(f"... ({len(file_list) - ELLIPSIS_THRESHOLD} more {ext} files)")
            else:
                display_files.extend(file_list)
        
        # 合并其他文件，并排序
        display_files.extend(other_files)
        display_files.sort() # 最终排序，确保"... "条目位置合理

        # 合并目录和处理后的文件列表
        all_items_to_display = dirs + display_files
        
        for i, item_name in enumerate(all_items_to_display):
            is_last = (i == len(all_items_to_display) - 1)
            connector = "└── " if is_last else "├── "
            
            # 特殊处理 "..." 字符串，它不是真实的文件或目录
            if item_name.startswith("... (") and " more " in item_name and " files)" in item_name :
                tree_lines.append(f"{prefix}{connector}{item_name}")
                continue

            full_item_path = os.path.join(current_path, item_name)
            
            if os.path.isdir(full_item_path):
                tree_lines.append(f"{prefix}{connector}{item_name}/")
                new_prefix = prefix + ("    " if is_last else "│   ")
                recurse_dir(full_item_path, new_prefix)
            else: # 是文件
                tree_lines.append(f"{prefix}{connector}{item_name}")

    # 从项目根目录的下一级开始递归
    recurse_dir(root_dir, "") # 初始前缀为空
    return "\n".join(tree_lines)


def get_code_contents(root_dir, script_name):
    """
    获取项目中所有 TARGET_CODE_EXTENSIONS 中定义的文件以及 README.md 文件的内容。

    参数:
    root_dir (str): 项目根目录的绝对路径。
    script_name (str): 要排除的脚本文件名。

    返回:
    str: 包含所有相关文件内容的 Markdown 格式字符串。
    """
    content_blocks = []
    files_to_extract_info = [] # (relative_path, absolute_path)

    # 1. 查找 README.md (根目录优先)
    readme_path_root = os.path.join(root_dir, "README.md")
    if os.path.isfile(readme_path_root):
        files_to_extract_info.append(("README.md", readme_path_root))
    
    readme_path_root_lower = os.path.join(root_dir, "readme.md") # 有些人会用小写
    if os.path.isfile(readme_path_root_lower) and readme_path_root_lower != readme_path_root:
         files_to_extract_info.append(("readme.md", readme_path_root_lower))


    # 2. 递归查找所有目标文件
    for dirpath, dirnames, filenames in os.walk(root_dir, topdown=True):
        # 从 dirnames 中移除要排除的目录，防止 os.walk 进入这些目录
        dirnames[:] = [d for d in dirnames if d not in EXCLUDED_ITEMS]

        for filename in filenames:
            # 排除脚本自身
            if filename == script_name:
                continue

            relative_path = os.path.relpath(os.path.join(dirpath, filename), root_dir)
            # 规范化路径分隔符为 /
            relative_path_display = relative_path.replace(os.sep, '/')
            
            absolute_path = os.path.join(dirpath, filename)
            
            file_extension = os.path.splitext(filename)[1].lower()

            if file_extension in TARGET_CODE_EXTENSIONS:
                # 确保不重复添加
                if not any(info[1] == absolute_path for info in files_to_extract_info):
                    files_to_extract_info.append((relative_path_display, absolute_path))
            elif filename.lower() == "readme.md": # 查找子目录中的 README.md
                # 确保不重复添加 (根目录的已处理)
                if not any(info[1] == absolute_path for info in files_to_extract_info):
                     files_to_extract_info.append((relative_path_display, absolute_path))
    
    # 排序：README.md (根目录的) 优先，然后按路径字母顺序
    def sort_key(item_info):
        rel_path = item_info[0]
        if rel_path.lower() == "readme.md": # 根目录的README
            return (0, rel_path)
        return (1, rel_path)

    files_to_extract_info.sort(key=sort_key)

    # 3. 读取文件内容并格式化
    for relative_path_display, absolute_path in files_to_extract_info:
        try:
            with open(absolute_path, 'r', encoding='utf-8', errors='ignore') as f:
                code = f.read()
            
            lang_type = ""
            file_extension = os.path.splitext(relative_path_display)[1].lower()
            
            if file_extension == ".html":
                lang_type = "html"
            elif file_extension == ".css":
                lang_type = "css"
            elif file_extension == ".js":
                lang_type = "javascript" # 使用 'javascript' 而不是 'js' 获得更好的高亮
            elif relative_path_display.lower().endswith(".md"):
                lang_type = "markdown" 
            
            content_blocks.append(f"### 文件: `{relative_path_display}`\n")
            content_blocks.append(f"```{lang_type}\n{code.strip()}\n```\n")
        except Exception as e:
            content_blocks.append(f"### 文件: `{relative_path_display}`\n")
            content_blocks.append(f"```\n(无法读取文件内容: {e})\n```\n")
            print(f"警告: 无法读取文件 {absolute_path}: {e}")

    return "\n".join(content_blocks)

# --- 主程序 ---
def main():
    """主函数，执行脚本的核心逻辑。"""
    project_root = get_project_root()
    output_md_path = os.path.join(project_root, OUTPUT_MD_FILENAME)

    print(f"项目根目录: {project_root}")
    print(f"脚本文件名: {SCRIPT_FILENAME}")
    print(f"输出 Markdown 文件: {output_md_path}")

    # 1. 生成文件结构树
    print("正在生成文件结构树...")
    try:
        file_tree_str = generate_file_tree(project_root, SCRIPT_FILENAME, OUTPUT_MD_FILENAME)
    except Exception as e:
        print(f"生成文件树时出错: {e}")
        file_tree_str = f"生成文件树失败: {e}"


    # 2. 获取代码内容
    print(f"正在提取 {', '.join(TARGET_CODE_EXTENSIONS)} 和 README.md 文件内容...")
    try:
        code_contents_str = get_code_contents(project_root, SCRIPT_FILENAME)
    except Exception as e:
        print(f"提取文件内容时出错: {e}")
        code_contents_str = f"提取文件内容失败: {e}"


    # 3. 写入 Markdown 文件
    print(f"正在将结果写入 {output_md_path}...")
    try:
        with open(output_md_path, 'w', encoding='utf-8') as f:
            f.write(f"# {os.path.basename(project_root)} 项目概览\n\n")
            
            f.write("## 项目结构\n")
            f.write("```text\n") # 使用 text 代码块来保持树形结构的格式
            f.write(file_tree_str)
            f.write("\n```\n\n")
            
            f.write("## 文件内容\n")
            if code_contents_str:
                f.write(code_contents_str)
            else:
                f.write(f"未找到 {', '.join(TARGET_CODE_EXTENSIONS)} 或 README.md 文件，或提取失败。\n")
        
        print(f"成功生成项目概览文件: {output_md_path}")
    except IOError as e:
        print(f"写入 Markdown 文件失败: {e}")
    except Exception as e:
        print(f"发生未知错误: {e}")


if __name__ == "__main__":
    main()