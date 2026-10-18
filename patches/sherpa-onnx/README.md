# sherpa-onnx 源码补丁

`scripts/build-desktop-sherpa-dml.mjs` 会在**构建时**把 sherpa-onnx 源码下到
`target/sherpa-onnx-directml/`（版本取自 `Cargo.lock` 里 `sherpa-onnx-sys` 的锁定版本），
解压后按文件名顺序把本目录下的 `*.patch` 逐个打上去。

也就是说：**仓库里只存 diff，不存改过的源码树**。升级 sherpa-onnx 时补丁打不上会直接
报错（`git apply` / `patch` 都失败即退出非 0），不会静默编出一份「看起来对」的库。

约定：

- 文件名带版本，便于一眼看出适用版本，例如
  `1.13.5-add-webgpu-provider-case.patch`；
- 用 `git diff` 生成、`git apply -p1` 能直接打上（`-p1` 即去掉 `a/`、`b/` 前缀）；
- 补丁内容变化会自动让缓存失效（`patchFingerprint()` 参与 `target/sherpa-onnx-directml/state.json`），
  不需要手工 `--force`。

当前是空的：Windows 的 DirectML 变体只需要 cmake 开关
（`-DSHERPA_ONNX_ENABLE_DIRECTML=ON`），不必改 C++。
