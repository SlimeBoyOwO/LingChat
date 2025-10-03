import argparse

def get_parser():
    parser = argparse.ArgumentParser(description="CLI for install/run options")

    parser.add_argument(
        "--install",
        nargs="+",
        choices=["vits", "sbv2", "18emo"],
        help="Modules to install"
    )

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
