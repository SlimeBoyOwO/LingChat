import os
import subprocess
import sys


def check_python_venv():
    cwd = os.getcwd()

    # check current dir is project root
    if cwd != os.path.dirname(os.path.dirname(os.path.abspath(__file__))):
        input("请在LingChat目录下运行此脚本! 执行 python scripts/start.py")
        exit(1)

    venv_dir = os.path.join(cwd, ".venv")
    if not os.path.exists(venv_dir):
        # create virtual environment
        input(f"没有找到虚拟环境, 按任意键创建虚拟环境.")
        subprocess.check_call(["python", "-m", "venv", ".venv"])

        # install dependencies
        subprocess.check_call(
            ["python", "-m", "pip", "install", "-r", "requirements.txt"]
        )
        input("安装完成, 请重新执行此脚本.")
        exit(1)

    # Get the current Python executable path
    python_executable = sys.executable

    if not python_executable.startswith(venv_dir):
        input("请在虚拟环境中运行此脚本! 执行 .venv/Scripts/activate 以激活虚拟环境.")
        exit(1)


def check_node_env():
    try:
        output = subprocess.check_output(["node", "-v"])
        print("Node.js is installed and available")
        print(output.decode("utf-8").strip())
    except FileNotFoundError:
        input("需要安装Node.js")
        exit(1)

    assert os.path.exists("frontend"), "找不到 frontend 目录!"

    if not os.path.exists("frontend/node_modules"):
        print("正在安装 node 所需要的包.")
        subprocess.check_call(["npm", "install"], cwd="frontend")


def start_vits():
    if not os.path.exists("third_party/vits-simple-api-windows-cpu-v0.6.16"):
        input("找不到 vits !")
        exit(1)

    subprocess.run(
        ["third_party/vits-simple-api-windows-cpu-v0.6.16/py310/python.exe", "app.py"],
        cwd="third_party/vits-simple-api-windows-cpu-v0.6.16",
    )


def start_backend():
    subprocess.run(
        [
            "python",
            "backend/webChat.windows.py",  # TODO: remove 'windows' suffix in file.
        ]
    )


def start_frontend():
    subprocess.run(["node", "frontend/server.js"])


def main():
    check_python_venv()
    check_node_env()
    start_vits()
    start_backend()
    start_frontend()


if __name__ == "__main__":
    main()
