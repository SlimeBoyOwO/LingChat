# LingChat 开发命令

.PHONY: check format fix typecheck test install dev

# 代码检查 (ruff)
check:
	uv run ruff check ling_chat tests

# 代码格式化 (ruff)
format:
	uv run ruff format ling_chat tests

# 自动修复问题 (ruff)
fix:
	uv run ruff check ling_chat tests --fix

# 类型检查 (ty)
typecheck:
	uv run ty check ling_chat tests

# 运行测试
test:
	uv run pytest

# 安装和同步依赖
install:
	uv sync --extra dev

# 启动py开发服务器
dev:
	uv run main.py

# 启动前端开发服务器
pnpmdev:
	cd frontend_vue && pnpm dev

# 前端完全检查（构建）
pnpmbuild:
	cd frontend_vue && pnpm build

