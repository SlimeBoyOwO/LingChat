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

    return parser
