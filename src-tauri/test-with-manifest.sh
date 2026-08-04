#!/usr/bin/env bash
# Windows 专用测试启动器：
# lib 测试二进制不会嵌入 tauri-build 的应用 manifest；代码（含依赖）一旦引用
# comctl32 v6 的函数（如 TaskDialogIndirect），测试进程启动即报
# 0xc0000139 STATUS_ENTRYPOINT_NOT_FOUND。
# 本脚本先编译测试，再把 test.manifest 复制为测试 exe 的同名外部 manifest
# （Windows 加载器对无内嵌 manifest 的 exe 会读取同名 .manifest 文件），然后运行。
#
# 用法：bash test-with-manifest.sh [cargo test 的额外参数，如 ai_service]
set -e
cd "$(dirname "$0")"

cargo test --lib --no-run "$@"
exe=$(ls -t target/debug/deps/ling_chat_lib-*.exe 2>/dev/null | head -1)
if [ -n "$exe" ]; then
  cp -f test.manifest "$exe.manifest"
fi
cargo test --lib "$@"
