# Others 目录说明

此目录包含一些辅助脚本和工具，用于支持主项目开发、部署和维护。

## 主要脚本功能

### compare_env_files.py

用于比较`.env`和`.env.example`文件，将示例文件中的缺失键复制到实际的环境文件中，帮助维护环境变量配置的一致性。

用法：

首先 cd 到 others 目录下

然后执行 `python compare_env_files.py` 即可快速同步

### image_to_webp_converter.py

将各种图片格式（JPG、PNG、BMP等）转换为WebP格式，支持有损和无损压缩，并可保留PNG的透明度。

### 可以向deepseek快速提问项目的奇妙小工具.py

用于生成项目概览文件，扫描整个项目结构并将所有Python文件和README.md的内容汇总到一个Markdown文件中，方便AI模型理解整个项目。

### logger_new.py

一个功能丰富的日志记录器，支持彩色输出、带时间戳的日志记录、多种动画加载效果和文件日志记录。

### memory_mcp.py

实现了与记忆系统相关的功能，包括添加记忆、搜索记忆和列出记忆的工具函数。

### process.py

用于批量压缩图片尺寸的脚本，将 **avatar** 文件夹中的图片缩小一半并保存到 **avatar_bk** 文件夹。

### install.bat

Windows批处理脚本，用于自动下载和安装vits-simple-api及其相关模型文件。

## 子目录说明

### document/

包含Linux部署相关文档和安装脚本。

### memory_rag/

记忆系统相关的代码，实现了图数据库RAG（检索增强生成）功能。

### release/

存放发布相关的文件和资源。

## 其他工具

### memory_client.py

在memory_rag目录中，提供与记忆系统交互的客户端接口。
