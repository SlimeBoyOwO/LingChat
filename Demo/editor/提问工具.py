import os
import sys
from collections import defaultdict

# 脚本文件名 (自动获取)
SCRIPT_FILENAME = os.path.basename(__file__)

# 生成的 Markdown 文件名
OUTPUT_MD_FILENAME = "project_summary.md"

# 1. 指定需要提取并展示其内容的文件扩展名

INCLUDE_CONTENT_EXTENSIONS = {
    '.py',      # Python
    '.md',      # Markdown
    '.js',      # JavaScript
    '.ts',      
    '.css',     # CSS
    '.tsx',
    '.html',    # HTML
    '.go',      # Go
    '.yaml',    # YAML / YML
    '.yml',
    '.m',       # MATLAB
    '.sh',      # Shell 脚本
    '.conf',    # 配置文件
    '.ini',
    '.toml',
    '.json',
    'Dockerfile', # Dockerfile (无扩展名，作为特例)
    '.dockerignore'
}

# 2. 指定需要完全忽略的目录和文件
EXCLUDED_ITEMS = {
    '.git',
    '__pycache__',
    'node_modules',
    'venv',
    'env',
    '.venv',
    '.vscode',
    '.idea',
    '.DS_Store',
    'dist',
    'build',
    'target',
    'package-lock.json',
    'others'
}

# 3. 对于这些扩展名的文件，如果数量过多，则在【文件树】中进行折叠显示
ELLIPSIS_EXTENSIONS = {
    # 图片
    '.png', '.jpg', '.jpeg', '.gif', '.bmp', '.svg', '.ico',
    # 日志和临时文件
    '.log', '.tmp', '.temp',
    # 编译产物和二进制文件
    '.o', '.obj', '.dll', '.so', '.exe', '.bin', '.class', '.jar',
    # 数据和模型文件
    '.data', '.dat', '.db', '.sqlite3',
    '.pth', '.pt', '.ckpt', '.h5', '.onnx',
    # 压缩文件
    '.zip', '.tar', '.gz', '.rar',
    # 文档
    '.pdf', '.doc', '.docx', '.xls', '.xlsx', '.ppt', '.pptx',
    # 字体
    '.woff', '.woff2', '.ttf', '.eot'
}
# 折叠处理的阈值，超过这个数量的同类型文件将被折叠
ELLIPSIS_THRESHOLD = 3 # 最多显示3个，其余用 "..." 表示

# 4. 文件扩展名到 Markdown 语言标识符的映射，用于语法高亮
LANGUAGE_MAP = {
    '.py': 'python',
    '.md': 'markdown',
    '.js': 'javascript',
    '.css': 'css',
    '.html': 'html',
    '.go': 'go',
    '.yaml': 'yaml',
    '.yml': 'yaml',
    '.m': 'matlab',
    '.sh': 'shell',
    '.conf': 'ini',
    '.ini': 'ini',
    '.toml': 'toml',
    'dockerfile': 'dockerfile', # 特例，基于文件名
    '.dockerignore': 'text'
}

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
    tree_lines = [f"{os.path.basename(root_dir)}/"]
    
    current_exclusions = EXCLUDED_ITEMS.copy()
    current_exclusions.add(script_name)
    current_exclusions.add(md_name)

    def recurse_dir(current_path, prefix):
        try:
            entries = os.listdir(current_path)
        except PermissionError:
            tree_lines.append(f"{prefix}└── [无法访问:权限不足] {os.path.basename(current_path)}/")
            return
        
        entries = sorted([e for e in entries if e not in current_exclusions])
        
        dirs = [e for e in entries if os.path.isdir(os.path.join(current_path, e))]
        files = [e for e in entries if os.path.isfile(os.path.join(current_path, e))]

        # 对文件进行省略处理
        files_by_ext = defaultdict(list)
        other_files = []
        for f_name in files:
            ext = os.path.splitext(f_name)[1].lower()
            if ext in ELLIPSIS_EXTENSIONS:
                files_by_ext[ext].append(f_name)
            else:
                other_files.append(f_name)
        
        display_files = []
        for ext in sorted(files_by_ext.keys()):
            file_list = files_by_ext[ext]
            if len(file_list) > ELLIPSIS_THRESHOLD:
                display_files.extend(file_list[:ELLIPSIS_THRESHOLD])
                display_files.append(f"... ({len(file_list) - ELLIPSIS_THRESHOLD} more {ext} files)")
            else:
                display_files.extend(file_list)
        
        display_files.extend(other_files)
        display_files.sort()

        all_items_to_display = dirs + display_files
        
        for i, item_name in enumerate(all_items_to_display):
            is_last = (i == len(all_items_to_display) - 1)
            connector = "└── " if is_last else "├── "
            
            if item_name.startswith("..."):
                tree_lines.append(f"{prefix}{connector}{item_name}")
                continue

            full_item_path = os.path.join(current_path, item_name)
            if os.path.isdir(full_item_path):
                tree_lines.append(f"{prefix}{connector}{item_name}/")
                new_prefix = prefix + ("    " if is_last else "│   ")
                recurse_dir(full_item_path, new_prefix)
            else:
                tree_lines.append(f"{prefix}{connector}{item_name}")

    recurse_dir(root_dir, "")
    return "\n".join(tree_lines)


def get_code_contents(root_dir, script_name):
    """
    获取项目中所有在 INCLUDE_CONTENT_EXTENSIONS 中定义的文件内容。

    参数:
    root_dir (str): 项目根目录的绝对路径。
    script_name (str): 要排除的脚本文件名。

    返回:
    str: 包含所有相关文件内容的 Markdown 格式字符串。
    """
    content_blocks = []
    files_to_extract = []

    for dirpath, dirnames, filenames in os.walk(root_dir, topdown=True):
        # 从遍历中排除指定的目录，效率更高
        dirnames[:] = [d for d in dirnames if d not in EXCLUDED_ITEMS]

        for filename in filenames:
            if filename == script_name:
                continue

            # 检查扩展名或完整文件名是否在白名单中
            ext = os.path.splitext(filename)[1].lower()
            if ext in INCLUDE_CONTENT_EXTENSIONS or filename in INCLUDE_CONTENT_EXTENSIONS:
                absolute_path = os.path.join(dirpath, filename)
                relative_path = os.path.relpath(absolute_path, root_dir).replace(os.sep, '/')
                files_to_extract.append((relative_path, absolute_path))

    # 排序：根目录的README优先，然后是其他README，最后按路径字母排序
    def sort_key(file_info):
        rel_path = file_info[0].lower()
        if rel_path == "readme.md":
            return (0, rel_path)
        if rel_path.endswith("/readme.md"):
            return (1, rel_path)
        return (2, rel_path)

    files_to_extract.sort(key=sort_key)

    # 读取文件内容并格式化
    for relative_path, absolute_path in files_to_extract:
        try:
            with open(absolute_path, 'r', encoding='utf-8', errors='ignore') as f:
                code = f.read()
            
            # 确定代码块的语言类型
            filename_lower = os.path.basename(relative_path).lower()
            ext_lower = os.path.splitext(filename_lower)[1]
            
            lang_type = LANGUAGE_MAP.get(ext_lower, '') # 优先按扩展名
            if not lang_type and filename_lower in LANGUAGE_MAP:
                lang_type = LANGUAGE_MAP[filename_lower] # 再按完整文件名 (如 Dockerfile)
            
            content_blocks.append(f"### 文件: `{relative_path}`\n")
            content_blocks.append(f"```{lang_type}\n{code.strip()}\n```\n")
        except Exception as e:
            content_blocks.append(f"### 文件: `{relative_path}`\n")
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
    print("正在根据配置提取文件内容...")
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
            f.write("```text\n")
            f.write(file_tree_str)
            f.write("\n```\n\n")
            
            f.write("## 文件内容\n")
            if code_contents_str:
                f.write(code_contents_str)
            else:
                f.write("根据您的配置，未找到需要提取内容的文件。\n")
        
        print(f"成功生成项目概览文件: {output_md_path}")
    except IOError as e:
        print(f"写入 Markdown 文件失败: {e}")
    except Exception as e:
        print(f"发生未知错误: {e}")


if __name__ == "__main__":
    main()