# sherpa-onnx 源码补丁

两份自编脚本都会在**构建时**把 sherpa-onnx 源码下到 `target/` 下（版本取自
`Cargo.lock` 里 `sherpa-onnx-sys` 的锁定版本），解压后按文件名顺序把本目录下的
`*.patch` 逐个打上去：

| 脚本                                                       | 缓存目录                            |
| ---------------------------------------------------------- | ----------------------------------- |
| `scripts/build-desktop-sherpa-dml.mjs`（Windows DirectML） | `target/sherpa-onnx-directml/`      |
| `scripts/build-android-sherpa-nnapi.mjs`（Android NNAPI）  | `target/sherpa-onnx-android-nnapi/` |

源码准备与打补丁的代码是同一份：`scripts/lib/sherpa-source.mjs`。

也就是说：**仓库里只存 diff，不存改过的源码树**。升级 sherpa-onnx 时补丁打不上会直接
报错（`git apply` / `patch` 都失败即退出非 0），不会静默编出一份「看起来对」的库。

约定：

- 文件名带版本，便于一眼看出适用版本，例如
  `1.13.5-add-webgpu-provider-case.patch`；
- 用 `git diff` 生成、`git apply -p1` 能直接打上（`-p1` 即去掉 `a/`、`b/` 前缀）；
- 补丁内容变化会自动让缓存失效（`patchFingerprint()` 参与缓存目录里的 `state.json`），
  不需要手工 `--force`。

当前是空的：两个变体都只需要 cmake 开关（Windows 的
`-DSHERPA_ONNX_ENABLE_DIRECTML=ON`、Android 的 `-DANDROID_PLATFORM=android-27`），
不必改 C++。
