import os
import pathlib
import subprocess
import sys


def pause_and_exit(prompt: str = "", exit_code: int = 1):
    input(prompt)
    exit(exit_code)


def check_python_venv():
    cwd = os.getcwd()

    # check current dir is project root
    if cwd != os.path.dirname(os.path.dirname(os.path.abspath(__file__))):
        pause_and_exit("请在LingChat目录下运行此脚本! 执行 python scripts/start.py")

    venv_dir = os.path.join(cwd, ".venv")
    if not os.path.exists(venv_dir):
        # create virtual environment
        input(f"没有找到虚拟环境, 按任意键创建虚拟环境.")
        if subprocess.check_call(["python", "-m", "venv", ".venv"]) != 0:
            pause_and_exit("创建虚拟环境失败")

        # install dependencies
        install_res = subprocess.check_call(
            ["python", "-m", "pip", "install", "-r", "requirements.log"]
        )

        if install_res != 0:
            pause_and_exit("安装依赖项失败.")

        pause_and_exit("安装完成, 请重新执行此脚本.", 0)

    # Get the current Python executable path
    python_executable = sys.executable

    if not python_executable.startswith(venv_dir):
        pause_and_exit(
            "请在虚拟环境中运行此脚本! 执行 .venv/Scripts/activate 以激活虚拟环境."
        )


def check_node_env():
    try:
        output = subprocess.check_output(["node", "-v"])
        print("Node.js is installed and available")
        print(output.decode("utf-8").strip())
    except FileNotFoundError:
        pause_and_exit("需要安装Node.js")

    assert os.path.exists("frontend"), "找不到 frontend 目录!"

    if not os.path.exists("frontend/node_modules"):
        print("正在安装 node 所需要的包.")
        subprocess.check_call(["npm", "install"], cwd="frontend")


def get_runtime_log_dir():
    return pathlib.Path("runtime_logs")


def check_runtime_logging_env():
    log_dir = get_runtime_log_dir()
    os.makedirs(log_dir, exist_ok=True)


def start_vits():
    if not os.path.exists("third_party/vits-simple-api-windows-cpu-v0.6.16"):
        pause_and_exit("找不到 vits !")

    fout_path = os.path.join(get_runtime_log_dir(), "vits_out.log")
    ferr_path = os.path.join(get_runtime_log_dir(), "vits_err.log")
    with open(fout_path, "w") as fout, open(ferr_path, "w") as ferr:
        subprocess.Popen(
            [
                "third_party/vits-simple-api-windows-cpu-v0.6.16/py310/python.exe",
                "app.py",
            ],
            cwd="third_party/vits-simple-api-windows-cpu-v0.6.16",
            stdout=fout,
            stderr=ferr,
        )


def start_backend():
    fout_path = os.path.join(get_runtime_log_dir(), "backend_out.log")
    ferr_path = os.path.join(get_runtime_log_dir(), "backend_err.log")

    with open(fout_path, "w") as fout, open(ferr_path, "w") as ferr:
        subprocess.Popen(
            [
                "python",
                "backend/webChat.windows.py",  # TODO: remove 'windows' suffix in file.
            ],
            stdout=fout,
            stderr=ferr,
        )


def start_frontend():
    fout_path = os.path.join(get_runtime_log_dir(), "frontend_out.log")
    ferr_path = os.path.join(get_runtime_log_dir(), "frontend_err.log")

    with open(fout_path, "w") as fout, open(ferr_path, "w") as ferr:
        subprocess.Popen(["node", "frontend/server.js"], stdout=fout, stderr=ferr)


def main():
    check_python_venv()
    check_node_env()
    check_runtime_logging_env()
    start_vits()
    start_backend()
    start_frontend()


if __name__ == "__main__":
    main()
