import argparse

def get_parser():
    parser = argparse.ArgumentParser(description="LingChat CLI")
    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # install 子命令
    install_parser = subparsers.add_parser("install", help="Install modules")
    install_parser.add_argument(
        "modules",
        nargs="+",
        choices=["vits", "sbv2", "18emo"],
        help="Modules to install"
    )

    # run 主程序启动选项
    parser.add_argument(
        "--run",
        nargs="+",
        choices=["vits", "sbv2", "18emo", "webview"],
        help="Modules to run"
    )

    return parser
