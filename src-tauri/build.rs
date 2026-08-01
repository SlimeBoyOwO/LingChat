// 改动此文件触发 build.rs 重跑：tauri-build 重新读取 frontendDist 打包进 .so。
//（tauri-build 不追踪 dist 变化，改完前端后需改此文件再 build）
fn main() {
    tauri_build::build()
}
