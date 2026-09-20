//! 果蝇脑连接组图数据加载（npz/npy 解析）。
//!
//! 移植自参考项目 fly-snake 的数据层（`brain.py` 的 `Brain.__init__` 读取部分），
//! 数据文件为 `np.savez_compressed` 产物（zip(deflate) 内含多个 .npy），
//! 路径约定 `data/third_party/fly_brain/graph.npz`。
//!
//! npy 头为手写解析：magic `\x93NUMPY` + 版本（1.0: u16 LE 头长 / 2.0: u32 LE 头长）
//! + Python 字面量 dict（提取 `descr` / `fortran_order` / `shape`）。
//! 按数据文件的实际约定做如下假设（解析失败会直接报错，不静默容错）：
//! - 全部条目 `fortran_order == False`（C 序）、小端定长 dtype；
//! - `super_class` / `side` 为 `<U…` 0 维大字符串（UTF-32 LE 码点，`\0` 分隔成
//!   n 段，按顺序对应神经元；与 Python 的 `str(arr).split("\0")` 行为一致，
//!   numpy 转 str 时去掉尾部 `\0` 填充，这里同样 trim）。

use std::io::Read;
use std::path::Path;

/// 解析后的 npy 条目元信息。
struct NpyMeta {
    descr: String,
    shape: Vec<usize>,
    data_offset: usize,
}

/// 单个 npy 条目（头 + 原始负载字节）。
struct NpyEntry {
    meta: NpyMeta,
    payload: Vec<u8>,
}

impl NpyEntry {
    /// 从完整 npy 字节流解析头部，返回元信息与数据区偏移。
    fn parse(buf: Vec<u8>) -> Result<NpyEntry, String> {
        if buf.len() < 10 || &buf[..6] != b"\x93NUMPY" {
            return Err("不是合法的 npy（magic 不匹配）".into());
        }
        let major = buf[6];
        let (header_len, off) = match major {
            1 => (u16::from_le_bytes([buf[8], buf[9]]) as usize, 10usize),
            2 | 3 => (
                u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]) as usize,
                12usize,
            ),
            v => return Err(format!("不支持的 npy 版本 {v}")),
        };
        let header_end = off + header_len;
        if buf.len() < header_end {
            return Err("npy 头长度越界".into());
        }
        let header = std::str::from_utf8(&buf[off..header_end])
            .map_err(|e| format!("npy 头非 UTF-8: {e}"))?;
        let descr = parse_dict_str(header, "descr")?;
        let fortran = parse_dict_bool(header, "fortran_order")?;
        if fortran {
            return Err("不支持 fortran_order=True 的条目".into());
        }
        let shape = parse_dict_shape(header)?;
        Ok(NpyEntry {
            meta: NpyMeta {
                descr,
                shape,
                data_offset: header_end,
            },
            payload: buf,
        })
    }

    fn data(&self) -> &[u8] {
        &self.payload[self.meta.data_offset..]
    }

    /// 校验元素个数与数据区长度一致。
    fn check_len(&self, elem_size: usize) -> Result<usize, String> {
        let count: usize = self.meta.shape.iter().product();
        let need = count * elem_size;
        if self.data().len() < need {
            return Err(format!(
                "条目数据长度不足: shape={:?} 需要 {} 字节, 实际 {}",
                self.meta.shape,
                need,
                self.data().len()
            ));
        }
        Ok(count)
    }

    fn as_i64_scalar(&self) -> Result<i64, String> {
        if self.meta.descr != "<i8" || !self.meta.shape.is_empty() {
            return Err(format!(
                "期望 <i8 0 维标量, 实际 {} {:?}",
                self.meta.descr, self.meta.shape
            ));
        }
        self.check_len(8)?;
        Ok(i64::from_le_bytes(self.data()[..8].try_into().unwrap()))
    }

    fn as_u64_vec(&self) -> Result<Vec<u64>, String> {
        if self.meta.descr != "<u8" {
            return Err(format!("期望 <u8, 实际 {}", self.meta.descr));
        }
        self.check_len(8)?;
        Ok(self
            .data()
            .chunks_exact(8)
            .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
            .collect())
    }

    fn as_i64_vec(&self) -> Result<Vec<i64>, String> {
        if self.meta.descr != "<i8" {
            return Err(format!("期望 <i8, 实际 {}", self.meta.descr));
        }
        self.check_len(8)?;
        Ok(self
            .data()
            .chunks_exact(8)
            .map(|c| i64::from_le_bytes(c.try_into().unwrap()))
            .collect())
    }

    fn as_u32_vec(&self) -> Result<Vec<u32>, String> {
        if self.meta.descr != "<u4" {
            return Err(format!("期望 <u4, 实际 {}", self.meta.descr));
        }
        self.check_len(4)?;
        Ok(self
            .data()
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect())
    }

    fn as_u8_vec(&self) -> Result<Vec<u8>, String> {
        if self.meta.descr != "|u1" && self.meta.descr != "<u1" {
            return Err(format!("期望 |u1, 实际 {}", self.meta.descr));
        }
        self.check_len(1)?;
        Ok(self.data().to_vec())
    }

    fn as_f32_vec(&self) -> Result<Vec<f32>, String> {
        if self.meta.descr != "<f4" {
            return Err(format!("期望 <f4, 实际 {}", self.meta.descr));
        }
        self.check_len(4)?;
        Ok(self
            .data()
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect())
    }

    /// `<U…` 0 维大字符串 → 按 `\0` 分隔的段数组（对齐 numpy str() 去尾填充）。
    fn as_unicode_segments(&self) -> Result<Vec<String>, String> {
        if !self.meta.descr.starts_with("<U") || !self.meta.shape.is_empty() {
            return Err(format!(
                "期望 <U… 0 维字符串, 实际 {} {:?}",
                self.meta.descr, self.meta.shape
            ));
        }
        let mut s = String::with_capacity(self.data().len() / 4);
        for c in self.data().chunks_exact(4) {
            let cp = u32::from_le_bytes(c.try_into().unwrap());
            s.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
        }
        let trimmed = s.trim_end_matches('\0');
        Ok(trimmed.split('\0').map(|x| x.to_string()).collect())
    }

    /// `<U…` 0 维短字符串 → 单个 String（meta_* 条目用）。
    fn as_unicode_string(&self) -> Result<String, String> {
        Ok(self.as_unicode_segments()?.join("\0"))
    }
}

/// 在 npy 头 dict 中提取 `'key': '…'` 的字符串值。
fn parse_dict_str(header: &str, key: &str) -> Result<String, String> {
    let pat = format!("'{key}':");
    let pos = header
        .find(&pat)
        .ok_or_else(|| format!("npy 头缺少 {key}"))?;
    let rest = header[pos + pat.len()..].trim_start();
    let quote = rest
        .chars()
        .next()
        .filter(|c| *c == '\'' || *c == '"')
        .ok_or_else(|| format!("npy 头 {key} 值不是字符串"))?;
    let end = rest[1..]
        .find(quote)
        .ok_or_else(|| format!("npy 头 {key} 字符串未闭合"))?;
    Ok(rest[1..1 + end].to_string())
}

/// 在 npy 头 dict 中提取 `'key': True/False`。
fn parse_dict_bool(header: &str, key: &str) -> Result<bool, String> {
    let pat = format!("'{key}':");
    let pos = header
        .find(&pat)
        .ok_or_else(|| format!("npy 头缺少 {key}"))?;
    let rest = header[pos + pat.len()..].trim_start();
    if rest.starts_with("True") {
        Ok(true)
    } else if rest.starts_with("False") {
        Ok(false)
    } else {
        Err(format!("npy 头 {key} 不是布尔值"))
    }
}

/// 在 npy 头 dict 中提取 `'shape': (…)`，支持 `()` / `(n,)` / `(n, m)`。
fn parse_dict_shape(header: &str) -> Result<Vec<usize>, String> {
    let pos = header
        .find("'shape':")
        .ok_or_else(|| "npy 头缺少 shape".to_string())?;
    let rest = header[pos + "'shape':".len()..].trim_start();
    let rest = rest
        .strip_prefix('(')
        .ok_or_else(|| "npy 头 shape 不是元组".to_string())?;
    let end = rest
        .find(')')
        .ok_or_else(|| "npy 头 shape 未闭合".to_string())?;
    let inner = &rest[..end];
    let inner = inner.trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    inner
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty()) // 尾逗号（如 `(n,)`）产生的空段
        .map(|p| {
            p.parse::<usize>()
                .map_err(|e| format!("npy 头 shape 维度解析失败: {p}: {e}"))
        })
        .collect()
}

/// 从 npz（zip）中读取指定条目并解析 npy 头。
fn read_entry(
    archive: &mut zip::ZipArchive<std::io::BufReader<std::fs::File>>,
    name: &str,
) -> Result<NpyEntry, String> {
    let file_name = format!("{name}.npy");
    let mut entry = archive
        .by_name(&file_name)
        .map_err(|e| format!("npz 缺少条目 {file_name}: {e}"))?;
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut buf)
        .map_err(|e| format!("读取 npz 条目 {file_name} 失败: {e}"))?;
    NpyEntry::parse(buf).map_err(|e| format!("条目 {file_name}: {e}"))
}

fn open_npz(path: &Path) -> Result<zip::ZipArchive<std::io::BufReader<std::fs::File>>, String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("打开 {} 失败: {e}", path.display()))?;
    zip::ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|e| format!("{} 不是合法的 npz(zip): {e}", path.display()))
}

/// 脑连接组静态图数据（对齐 fly-snake `brain.py` 的 npz 条目约定）。
///
/// 存放 post 排序 CSR（`indptr`/`indices`/`weight`，E=2,484,367）与按 pre 排序的
/// 出边 CSR（`out_indptr`/`out_edges`，发放门控快速路径用），以及神经元的
/// 解剖/分类标注（`coords`/`super_class`/`side`/`root_ids`）。
pub struct BrainGraph {
    pub n: usize,
    pub root_ids: Option<Vec<u64>>,
    /// (n, 3) 展平，C 序。
    pub coords: Option<Vec<f32>>,
    pub super_class: Vec<String>,
    pub side: Vec<String>,
    /// post 排序 CSR 的 pre 索引（E,）。
    pub indices: Vec<u32>,
    /// post 排序 CSR 的行指针（n+1,）。
    pub indptr: Vec<i64>,
    pub weight: Vec<f32>,
    /// 边的神经递质类型编码（E,）；4 == DA（多巴胺），供奖惩后 DA 刺激。
    pub edge_nt: Option<Vec<u8>>,
    /// 出边 CSR 行指针（n+1,）。
    pub out_indptr: Vec<i64>,
    /// 出边 → post 排序数组中的边 id（E,）。
    pub out_edges: Vec<u32>,
    /// 左右轴（meta_lr_axis），positions 归一化与 BrainIO 坐标回退用。
    pub meta_lr_axis: Option<usize>,
}

impl BrainGraph {
    /// 完整加载 graph.npz（约 90MB 内存）。
    pub fn load(path: &Path) -> Result<Self, String> {
        let mut archive = open_npz(path)?;
        let n = read_entry(&mut archive, "n_neurons")?.as_i64_scalar()? as usize;

        // 必填：两套 CSR + 权重
        let indices = read_entry(&mut archive, "indices")?.as_u32_vec()?;
        let indptr = read_entry(&mut archive, "indptr")?.as_i64_vec()?;
        let weight = read_entry(&mut archive, "weight")?.as_f32_vec()?;
        let out_indptr = read_entry(&mut archive, "out_indptr")?.as_i64_vec()?;
        let out_edges = read_entry(&mut archive, "out_edges")?.as_u32_vec()?;
        if indptr.len() != n + 1 || out_indptr.len() != n + 1 {
            return Err(format!(
                "indptr 长度 {} / out_indptr 长度 {} 与 n+1={} 不符",
                indptr.len(),
                out_indptr.len(),
                n + 1
            ));
        }
        let e = weight.len();
        if indices.len() != e || out_edges.len() != e {
            return Err("indices/weight/out_edges 长度不一致".into());
        }

        // 可选：标注类条目（缺失时回退为空串/None，与 Python 一致）
        let root_ids = read_entry(&mut archive, "root_ids")
            .ok()
            .and_then(|x| x.as_u64_vec().ok());
        let coords = read_entry(&mut archive, "coords").ok().and_then(|x| {
            if x.meta.shape.len() == 2 && x.meta.shape[0] == n && x.meta.shape[1] == 3 {
                x.as_f32_vec().ok()
            } else {
                None
            }
        });
        let super_class = read_entry(&mut archive, "super_class")
            .ok()
            .and_then(|x| x.as_unicode_segments().ok())
            .filter(|v| v.len() == n)
            .unwrap_or_else(|| vec![String::new(); n]);
        let side = read_entry(&mut archive, "side")
            .ok()
            .and_then(|x| x.as_unicode_segments().ok())
            .filter(|v| v.len() == n)
            .unwrap_or_else(|| vec![String::new(); n]);
        let edge_nt = read_entry(&mut archive, "edge_nt")
            .ok()
            .and_then(|x| x.as_u8_vec().ok())
            .filter(|v| v.len() == e);
        let meta_lr_axis = read_entry(&mut archive, "meta_lr_axis")
            .ok()
            .and_then(|x| x.as_unicode_string().ok())
            .and_then(|s| s.parse::<usize>().ok());

        Ok(BrainGraph {
            n,
            root_ids,
            coords,
            super_class,
            side,
            indices,
            indptr,
            weight,
            edge_nt,
            out_indptr,
            out_edges,
            meta_lr_axis,
        })
    }
}

/// positions 懒加载所需的图子集（只解压 coords/super_class/meta_lr_axis 三个条目，
/// 避免为出点云而完整加载 ~90MB 图数据）。
pub struct PositionsSource {
    pub n: usize,
    pub coords: Vec<f32>,
    pub super_class: Vec<String>,
    pub meta_lr_axis: Option<usize>,
}

impl PositionsSource {
    pub fn load(path: &Path) -> Result<Self, String> {
        let mut archive = open_npz(path)?;
        let n = read_entry(&mut archive, "n_neurons")?.as_i64_scalar()? as usize;
        let coords_entry = read_entry(&mut archive, "coords")?;
        if coords_entry.meta.shape.len() != 2
            || coords_entry.meta.shape[0] != n
            || coords_entry.meta.shape[1] != 3
        {
            return Err(format!("coords 形状异常: {:?}", coords_entry.meta.shape));
        }
        let coords = coords_entry.as_f32_vec()?;
        let super_class = read_entry(&mut archive, "super_class")
            .ok()
            .and_then(|x| x.as_unicode_segments().ok())
            .filter(|v| v.len() == n)
            .unwrap_or_else(|| vec![String::new(); n]);
        let meta_lr_axis = read_entry(&mut archive, "meta_lr_axis")
            .ok()
            .and_then(|x| x.as_unicode_string().ok())
            .and_then(|s| s.parse::<usize>().ok());
        Ok(PositionsSource {
            n,
            coords,
            super_class,
            meta_lr_axis,
        })
    }
}
