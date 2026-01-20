import argparse


def get_parser():
    parser = argparse.ArgumentParser(description="LingChat CLI")
    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # install 子命令
    install_parser = subparsers.add_parser("install", help="Install modules")
    install_parser.add_argument(
        "modules",
        nargs="+",
        choices=["vits", "sbv2", "18emo", "rag"],
        help="Modules to install"
    )
    install_parser.add_argument(
        "--mirror", "-m",
        action="store_true",
        help="Use mirror site for downloading models (especially for RAG model)"
    )

    # run 主程序启动选项
    parser.add_argument(
        "--run",
        nargs="+",
        choices=["vits", "sbv2", "18emo", "webview"],
        help="Modules to run"
    )

    parser.add_argument(
        "--nogui",
        action="store_true",
        help="启用无界面模式（禁用前端界面）"
    )

    return parser
