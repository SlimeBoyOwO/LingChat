#include <windows.h>
#include <winternl.h>      // 提供 RTL_OSVERSIONINFOEXW 结构
#include <vector>
#include <string>
#include <iostream>
#include <algorithm>
#include <cstdlib>
#include <cctype>

#pragma comment(lib, "ntdll.lib")   // 链接 ntdll.lib，以便使用 RtlGetVersion

using namespace std;

// 获取 Windows 版本信息（主版本号、次版本号、友好名称）
struct WinVersion {
    DWORD major;
    DWORD minor;
    string name;
};

WinVersion GetWindowsVersion() {
    WinVersion ver = {0, 0, "Unknown"};
    
    // 使用 RtlGetVersion 获取真实版本
    RTL_OSVERSIONINFOEXW osvi;
    ZeroMemory(&osvi, sizeof(osvi));
    osvi.dwOSVersionInfoSize = sizeof(osvi);
    typedef LONG (WINAPI* RtlGetVersionPtr)(PRTL_OSVERSIONINFOEXW);
    HMODULE hNtdll = GetModuleHandleW(L"ntdll.dll");
    if (hNtdll) {
        RtlGetVersionPtr RtlGetVersion = (RtlGetVersionPtr)GetProcAddress(hNtdll, "RtlGetVersion");
        if (RtlGetVersion && RtlGetVersion(&osvi) == 0) {
            ver.major = osvi.dwMajorVersion;
            ver.minor = osvi.dwMinorVersion;
            // 根据版本号构建友好名称
            if (ver.major == 10 && ver.minor == 0) {
                ver.name = "Windows 10/11 (NT 10.0)";
            } else if (ver.major == 6 && ver.minor == 3) {
                ver.name = "Windows 8.1 (NT 6.3)";
            } else if (ver.major == 6 && ver.minor == 2) {
                ver.name = "Windows 8 (NT 6.2)";
            } else if (ver.major == 6 && ver.minor == 1) {
                ver.name = "Windows 7 (NT 6.1)";
            } else if (ver.major == 6 && ver.minor == 0) {
                ver.name = "Windows Vista (NT 6.0)";
            } else if (ver.major == 5 && ver.minor == 2) {
                ver.name = "Windows Server 2003/XP x64 (NT 5.2)";
            } else if (ver.major == 5 && ver.minor == 1) {
                ver.name = "Windows XP (NT 5.1)";
            } else if (ver.major == 5 && ver.minor == 0) {
                ver.name = "Windows 2000 (NT 5.0)";
            } else {
                ver.name = "Unknown Windows";
            }
        }
    }
    return ver;
}

// 将字符串转换为小写（用于不区分大小写比较）
string ToLower(const string& str) {
    string lower = str;
    transform(lower.begin(), lower.end(), lower.begin(), ::tolower);
    return lower;
}

// 从文件夹名中尝试提取 Python 版本号（如 "3.13.7" 或 "3.8"）
string ExtractPythonVersion(const string& folderName) {
    size_t pos = folderName.find("python-");
    if (pos != string::npos) {
        string rest = folderName.substr(pos + 7);
        string version;
        for (char c : rest) {
            if (isdigit(c) || c == '.')
                version += c;
            else
                break;
        }
        if (!version.empty())
            return version;
    }
    pos = folderName.find("Python");
    if (pos != string::npos && folderName.length() >= pos + 3) {
        string numPart = folderName.substr(pos + 6);
        if (numPart.length() >= 2 && isdigit(numPart[0]) && isdigit(numPart[1])) {
            char major = numPart[0];
            char minor = numPart[1];
            string version;
            version.push_back(major);
            version.push_back('.');
            version.push_back(minor);
            return version;
        }
    }
    return "";
}

bool HasPythonExe(const string& dirPath) {
    string pythonPath = dirPath + "\\python.exe";
    WIN32_FIND_DATAA findData;
    HANDLE hFind = FindFirstFileA(pythonPath.c_str(), &findData);
    if (hFind == INVALID_HANDLE_VALUE)
        return false;
    FindClose(hFind);
    return true;
}

struct PythonEnv {
    string path;
    string folderName;
    string version;
};

vector<PythonEnv> ScanPythonEnvironments() {
    vector<PythonEnv> envs;
    WIN32_FIND_DATAA findData;
    HANDLE hFind = FindFirstFileA("*", &findData);
    if (hFind == INVALID_HANDLE_VALUE)
        return envs;

    do {
        // 只处理目录
        if (findData.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) {
            string folderName = findData.cFileName;
            if (folderName != "." && folderName != "..") {
                // 只扫描包含 "python" 字样的文件夹（不区分大小写）
                string lowerName = ToLower(folderName);
                if (lowerName.find("python") != string::npos) {
                    string dirPath = folderName;
                    if (HasPythonExe(dirPath)) {
                        PythonEnv env;
                        env.path = dirPath;
                        env.folderName = folderName;
                        env.version = ExtractPythonVersion(folderName);
                        envs.push_back(env);
                    }
                }
            }
        }
    } while (FindNextFileA(hFind, &findData) != 0);
    FindClose(hFind);
    return envs;
}

int SelectPythonEnv(const vector<PythonEnv>& envs, const WinVersion& winVer) {
    if (envs.empty()) {
        cout << "未找到任何包含 python.exe 的 Python 环境目录。" << endl;
        return -1;
    }

    cout << "找到 " << envs.size() << " 个 Python 环境：" << endl;
    for (size_t i = 0; i < envs.size(); ++i) {
        cout << "  " << (i+1) << ". " << envs[i].folderName;
        bool recommended = false;
        if (winVer.major == 6 && winVer.minor == 1) {
            if (envs[i].version.find("3.8") == 0)
                recommended = true;
        } else if (winVer.major >= 10) {
            if (envs[i].version.find("3.13") == 0)
                recommended = true;
        }
        if (recommended)
            cout << " [推荐]";
        cout << endl;
    }
    cout << "请选择要使用的 Python 环境 (1-" << envs.size() << "): ";
    int choice;
    cin >> choice;
    if (cin.fail() || choice < 1 || choice > static_cast<int>(envs.size())) {
        cin.clear();
        cin.ignore(10000, '\n');
        cout << "无效选择。" << endl;
        return -1;
    }
    return choice - 1;
}

bool RunMainPy(const string& pythonPath) {
    string command = pythonPath + "\\python.exe main.py";
    cout << "执行命令: " << command << endl;
    int ret = system(command.c_str());
    if (ret != 0) {
        cout << "程序执行失败，返回码: " << ret << endl;
        return false;
    }
    return true;
}

int main() {
    SetConsoleOutputCP(CP_UTF8);
    bool restart = true;
    while (restart) {
        WinVersion winVer = GetWindowsVersion();
        cout << "当前系统版本: " << winVer.name << endl;

        if (winVer.major < 6 || (winVer.major == 6 && winVer.minor == 0)) {
            cout << "哦，我的朋友，你为什么要用古董运行这个项目，换台电脑或者升级系统可以提升兼容性，如果仍要运行。。。" << endl;
            cout << "按回车键继续..." << endl;
            cin.ignore(10000, '\n');
            cin.get();
        } else if (winVer.major == 6 && winVer.minor == 1) {
            cout << "检测到 Windows 7 系统，推荐使用 Python 3.8。" << endl;
        } else if (winVer.major >= 10) {
            cout << "检测到 Windows 10/11 系统，推荐使用 Python 3.13。" << endl;
        }

        vector<PythonEnv> envs = ScanPythonEnvironments();
        int selected = SelectPythonEnv(envs, winVer);
        if (selected < 0) {
            cout << "未选择有效的 Python 环境，程序退出。" << endl;
            break;
        }

        RunMainPy(envs[selected].path);

        char ch;
        do {
            cout << "按 Y 重新启动程序，按 N 退出: ";
            cin >> ch;
            cin.ignore(10000, '\n');
            if (ch == 'Y' || ch == 'y') {
                restart = true;
                break;
            } else if (ch == 'N' || ch == 'n') {
                restart = false;
                break;
            } else {
                cout << "无效输入，请按 Y 或 N。" << endl;
            }
        } while (true);
    }
    return 0;
}
//别怀疑了，我拿AI改的