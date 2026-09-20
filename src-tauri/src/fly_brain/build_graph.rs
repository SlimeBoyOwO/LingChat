//! 基础权重（graph.npz）离线构建器：FlyWire v783 Codex CSV → 与 graph.rs 加载器
//! 完全兼容的 npz。
//!
//! 移植自只读参考 fly-snake `tools/build_graph.py`（规则逐行对齐，保证产物与
//! Python 版逐位一致）：
//! - classification.csv.gz（root_id,flow,super_class,…,side,…）→ 神经元表（顺序即
//!   文件行序）；coordinates.csv.gz（root_id,position "[x y z]"）→ coords（缺 NaN）；
//! - connections.csv.gz（pre_root_id,post_root_id,neuropil,syn_count,nt_type）：
//!   逐行过滤 syn_count < min_syn（默认 5，**合并前**逐行过滤）、丢自连接与两端
//!   不在表内的边；nt 决定符号（GABA/GLUT 抑制为负，其余兴奋）并保留 nt 编码
//!   （同 build_graph.py 的 NT_CODES：""=0 ACH=1 GABA=2 GLUT=3 DA=4 OCT=5 SER=6）；
//! - 同 (pre,post) 多 neuropil 行合并：64 位复合键排序去重，syn 按原始行序累加
//!   （稳定排序 → 与 numpy unique+bincount 同序同结果），nt 取首次出现；
//! - weight = sign·sqrt(|syn|)·W_SCALE(0.3)（f32）；
//! - CSR：按 post 稳定排序得 indices/indptr；按 pre 稳定排序得出边 out_edges/
//!   out_indptr（发放门控快速路径）；
//! - 左右轴自动判定（left/right 群均值间隔按标准差归一化，取可分性最大的轴）；
//! - 写出 zip(deflate) + npy v1.0 头，条目名/dtype/0 维 <U 大字符串与现有
//!   graph.rs 加载器逐一对齐；先写 .tmp 再原子替换。

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::Instant;

use flate2::read::GzDecoder;
use tracing::info;

/// 权重缩放（与 build_graph.py 的 W_SCALE 一致）。
pub const W_SCALE: f32 = 0.3;
/// 默认 syn_count 过滤阈值。
pub const DEFAULT_MIN_SYN: i64 = 5;

/// 构建统计（冒烟与日志用）。
pub struct BuildStats {
    pub n: usize,
    pub edges: usize,
    pub ms: u128,
    pub lr_axis: Option<usize>,
}

// ─── CSV 读取 ───

fn open_gz_lines(path: &Path) -> Result<impl BufRead, String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("打开 {} 失败: {e}", path.display()))?;
    Ok(BufReader::with_capacity(1 << 20, GzDecoder::new(file)))
}

/// classification.csv.gz → (root_ids, super_class, side)（顺序一致）。
fn read_classification(path: &Path) -> Result<(Vec<u64>, Vec<String>, Vec<String>), String> {
    let mut ids = Vec::new();
    let mut sc = Vec::new();
    let mut side = Vec::new();
    let mut lines = open_gz_lines(path)?.lines();
    let header = lines
        .next()
        .ok_or_else(|| "classification 空文件".to_string())?
        .map_err(|e| e.to_string())?;
    let cols: Vec<&str> = header.trim_end().split(',').collect();
    let idx = |name: &str| {
        cols.iter()
            .position(|c| *c == name)
            .ok_or_else(|| format!("classification 缺列 {name}"))
    };
    let (i_id, i_sc, i_side) = (idx("root_id")?, idx("super_class")?, idx("side")?);
    for line in lines {
        let line = line.map_err(|e| e.to_string())?;
        let p: Vec<&str> = line.trim_end().split(',').collect();
        if p.len() <= i_id.max(i_sc).max(i_side) {
            continue;
        }
        let Ok(id) = p[i_id].parse::<u64>() else {
            continue;
        };
        ids.push(id);
        sc.push(p[i_sc].to_string());
        side.push(p[i_side].to_string());
    }
    Ok((ids, sc, side))
}

/// coordinates.csv.gz → coords（(n,3) 展平 f32，缺失 NaN）。返回命中数。
fn read_coordinates(
    path: &Path,
    idx_of: &HashMap<u64, u32>,
    n: usize,
) -> Result<(Vec<f32>, usize), String> {
    let mut coords = vec![f32::NAN; n * 3];
    let mut hit = 0usize;
    let mut lines = open_gz_lines(path)?.lines();
    let header = lines
        .next()
        .ok_or_else(|| "coordinates 空文件".to_string())?
        .map_err(|e| e.to_string())?;
    let cols: Vec<&str> = header.trim_end().split(',').collect();
    let idx = |name: &str| {
        cols.iter()
            .position(|c| *c == name)
            .ok_or_else(|| format!("coordinates 缺列 {name}"))
    };
    let (i_id, i_pos) = (idx("root_id")?, idx("position")?);
    for line in lines {
        let line = line.map_err(|e| e.to_string())?;
        let p: Vec<&str> = line.trim_end().split(',').collect();
        if p.len() <= i_id.max(i_pos) {
            continue;
        }
        let Ok(id) = p[i_id].parse::<u64>() else {
            continue;
        };
        let Some(&j) = idx_of.get(&id) else { continue };
        let s = p[i_pos]
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']');
        let mut it = s.split_whitespace();
        let (Ok(x), Ok(y), Ok(z)) = (
            it.next().unwrap_or("").parse::<f64>(),
            it.next().unwrap_or("").parse::<f64>(),
            it.next().unwrap_or("").parse::<f64>(),
        ) else {
            continue;
        };
        // 与 Python 一致：f64 解析后转 f32
        coords[j as usize * 3] = x as f32;
        coords[j as usize * 3 + 1] = y as f32;
        coords[j as usize * 3 + 2] = z as f32;
        hit += 1;
    }
    Ok((coords, hit))
}

/// nt_type → 编码（对齐 build_graph.py NT_CODES）。
fn nt_code(nt_upper: &str) -> u8 {
    match nt_upper {
        "ACH" => 1,
        "GABA" => 2,
        "GLUT" => 3,
        "DA" => 4,
        "OCT" => 5,
        "SER" => 6,
        _ => 0,
    }
}

/// connections.csv.gz → 合并后的 (pre, post, syn_signed, nt)（键排序唯一）。
/// 逐行过滤 syn_count<min_syn、丢自连接与表外端点；GABA/GLUT 为负号。
fn read_connections(
    path: &Path,
    idx_of: &HashMap<u64, u32>,
    min_syn: i64,
) -> Result<(Vec<u32>, Vec<u32>, Vec<f32>, Vec<u8>), String> {
    let mut lines = open_gz_lines(path)?.lines();
    let header = lines
        .next()
        .ok_or_else(|| "connections 空文件".to_string())?
        .map_err(|e| e.to_string())?;
    let cols: Vec<&str> = header.trim_end().split(',').collect();
    let idx = |name: &str| {
        cols.iter()
            .position(|c| *c == name)
            .ok_or_else(|| format!("connections 缺列 {name}"))
    };
    let (i_pre, i_post, i_syn, i_nt) = (
        idx("pre_root_id")?,
        idx("post_root_id")?,
        idx("syn_count")?,
        idx("nt_type")?,
    );
    // (key, syn_signed, nt_code)，保持原始行序
    let mut recs: Vec<(u64, f32, u8)> = Vec::with_capacity(2_500_000);
    let mut nrows = 0u64;
    let t0 = Instant::now();
    for line in lines {
        let line = line.map_err(|e| e.to_string())?;
        // 列数已知（pre,post,neuropil,syn_count,nt_type），用 split 取位（对齐 Python）
        let p: Vec<&str> = line.split(',').collect();
        if p.len() <= i_pre.max(i_post).max(i_syn).max(i_nt) {
            continue;
        }
        let Ok(s) = p[i_syn].parse::<i64>() else {
            continue;
        };
        nrows += 1;
        if s < min_syn {
            continue;
        }
        let (Ok(pre_id), Ok(post_id)) = (p[i_pre].parse::<u64>(), p[i_post].parse::<u64>()) else {
            continue;
        };
        let (Some(&a), Some(&b)) = (idx_of.get(&pre_id), idx_of.get(&post_id)) else {
            continue;
        };
        if a == b {
            continue; // 自连接
        }
        let nt = p[i_nt].trim().to_uppercase();
        let sign = if nt == "GABA" || nt == "GLUT" {
            -1.0
        } else {
            1.0
        };
        let key = (a as u64) << 32 | b as u64;
        recs.push((key, sign * s as f32, nt_code(&nt)));
        if nrows % 5_000_000 == 0 {
            info!(
                "connections: {}M 行, 保留 {}, {}s",
                nrows / 1_000_000,
                recs.len(),
                t0.elapsed().as_secs()
            );
        }
    }
    info!(
        "connections: {} 行, 阈值内且两端在表内 {} 条, {}s",
        nrows,
        recs.len(),
        t0.elapsed().as_secs()
    );

    // 合并重复 (pre,post)：键稳定排序（保持原始行序）→ 同键按行序累加、nt 取首现
    recs.sort_by_key(|r| r.0);
    let mut pre = Vec::with_capacity(recs.len());
    let mut post = Vec::with_capacity(recs.len());
    let mut syn = Vec::with_capacity(recs.len());
    let mut nt = Vec::with_capacity(recs.len());
    let mut i = 0;
    while i < recs.len() {
        let key = recs[i].0;
        let mut sum = 0.0f32;
        let first_nt = recs[i].2;
        let mut j = i;
        while j < recs.len() && recs[j].0 == key {
            sum += recs[j].1;
            j += 1;
        }
        pre.push((key >> 32) as u32);
        post.push((key & 0xFFFF_FFFF) as u32);
        syn.push(sum);
        nt.push(first_nt);
        i = j;
    }
    Ok((pre, post, syn, nt))
}

/// 左右轴判定（对齐 pick_lr_axis：left/right 群各轴均值间隔按总体标准差归一化）。
fn pick_lr_axis(coords: &[f32], side: &[String], n: usize) -> Option<usize> {
    let mut best: Option<usize> = None;
    let mut best_score = 0.0f64;
    let mut n_valid = 0usize;
    for i in 0..n {
        if !coords[i * 3].is_nan() && (side[i] == "left" || side[i] == "right") {
            n_valid += 1;
        }
    }
    if n_valid < 1000 {
        return None;
    }
    for ax in 0..3 {
        let (mut sl, mut sl2, mut nl, mut sr, mut sr2, mut nr) =
            (0f64, 0f64, 0u64, 0f64, 0f64, 0u64);
        for i in 0..n {
            if coords[i * 3].is_nan() {
                continue;
            }
            let v = coords[i * 3 + ax] as f64;
            match side[i].as_str() {
                "left" => {
                    sl += v;
                    sl2 += v * v;
                    nl += 1;
                },
                "right" => {
                    sr += v;
                    sr2 += v * v;
                    nr += 1;
                },
                _ => {},
            }
        }
        if nl == 0 || nr == 0 {
            continue;
        }
        let ml = sl / nl as f64;
        let mr = sr / nr as f64;
        // np.std 默认总体标准差（ddof=0）
        let std_l = (sl2 / nl as f64 - ml * ml).max(0.0).sqrt();
        let std_r = (sr2 / nr as f64 - mr * mr).max(0.0).sqrt();
        let score = (ml - mr).abs() / (std_l + std_r + 1e-9);
        if score > best_score {
            best_score = score;
            best = Some(ax);
        }
    }
    best
}

// ─── npy/npz 写出 ───

/// 写单个 npy 条目（v1.0 头 + 负载），对齐 numpy 格式：
/// magic + [1,0] + u16 头长 + `{'descr': '…', 'fortran_order': False, 'shape': …, }`
/// 空格补齐到 64 字节对齐 + '\n'。
fn write_npy<W: Write>(
    w: &mut W,
    descr: &str,
    shape: &[usize],
    payload: &[u8],
) -> Result<(), String> {
    let shape_str = match shape.len() {
        0 => "()".to_string(),
        1 => format!("({},)", shape[0]),
        _ => format!(
            "({})",
            shape
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    let dict = format!(
        "{{'descr': '{}', 'fortran_order': False, 'shape': {}, }}",
        descr, shape_str
    );
    let base = 10 + dict.len() + 1; // magic6+ver2+hlen2 + dict + '\n'
    let pad = (64 - base % 64) % 64;
    let header = format!("{}{}\n", dict, " ".repeat(pad));
    if header.len() > u16::MAX as usize {
        return Err("npy 头过长".into());
    }
    w.write_all(b"\x93NUMPY\x01\x00")
        .and_then(|_| w.write_all(&(header.len() as u16).to_le_bytes()))
        .and_then(|_| w.write_all(header.as_bytes()))
        .and_then(|_| w.write_all(payload))
        .map_err(|e| format!("写 npy 失败: {e}"))
}

fn f32_bytes(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}
fn u32_bytes(v: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}
fn i64_bytes(v: &[i64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 8);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}
fn u64_bytes(v: &[u64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 8);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}
/// 单个 '\0' 分隔字符串 → UTF-32 LE 码点字节（numpy <U 0 维）。
fn unicode_bytes(s: &str) -> (String, Vec<u8>) {
    let n = s.chars().count();
    let mut out = Vec::with_capacity(n * 4);
    for c in s.chars() {
        out.extend_from_slice(&(c as u32).to_le_bytes());
    }
    (format!("<U{n}"), out)
}

fn npz_put<W: Write + std::io::Seek>(
    zw: &mut zip::ZipWriter<W>,
    opts: zip::write::SimpleFileOptions,
    name: &str,
    descr: &str,
    shape: &[usize],
    payload: &[u8],
) -> Result<(), String> {
    zw.start_file(format!("{name}.npy"), opts)
        .map_err(|e| format!("zip start {name}: {e}"))?;
    write_npy(zw, descr, shape, payload)
}

// ─── 主流程 ───

/// 从 raw 目录（含三个 csv.gz）构建 graph.npz 到 out_path（调用方负责 .tmp→rename）。
pub fn build_from_raw(raw_dir: &Path, out_path: &Path, min_syn: i64) -> Result<BuildStats, String> {
    let t0 = Instant::now();
    // 1) classification
    let (ids, sc, side) = read_classification(&raw_dir.join("classification.csv.gz"))?;
    let n = ids.len();
    info!("classification: {} 神经元", n);
    let idx_of: HashMap<u64, u32> = ids
        .iter()
        .enumerate()
        .map(|(i, &r)| (r, i as u32))
        .collect();

    // 2) coordinates
    let (coords, hit) = read_coordinates(&raw_dir.join("coordinates.csv.gz"), &idx_of, n)?;
    info!("coordinates: 命中 {}/{}", hit, n);

    // 3) connections（最耗时）
    let (pre, post, syn, nt) =
        read_connections(&raw_dir.join("connections.csv.gz"), &idx_of, min_syn)?;
    let e = pre.len();
    info!("connections: 合并后 {} 边", e);

    // 4) 权重：符号 × sqrt 压缩 × W_SCALE（f32，与 numpy 逐位一致）
    let weight: Vec<f32> = syn
        .iter()
        .map(|&s| s.signum() * s.abs().sqrt() * W_SCALE)
        .collect();

    // 5) 按 post 稳定排序 → CSR
    let mut order: Vec<u32> = (0..e as u32).collect();
    order.sort_by_key(|&i| post[i as usize]);
    let indices: Vec<u32> = order.iter().map(|&i| pre[i as usize]).collect();
    let weight_s: Vec<f32> = order.iter().map(|&i| weight[i as usize]).collect();
    let edge_nt: Vec<u8> = order.iter().map(|&i| nt[i as usize]).collect();
    let mut indptr = vec![0i64; n + 1];
    for &p in &post {
        indptr[p as usize + 1] += 1;
    }
    for i in 0..n {
        indptr[i + 1] += indptr[i];
    }

    // 6) 出边 CSR：order2 = 按 pre 稳定排序。
    // 注意：合并后数组本身已按 (pre<<32|post) 键排序，按 pre 稳定排序即恒等排列，
    // 因此 build_graph.py 的 `out_edges = order2` 实际产出的是恒等排列
    // （语义上并非"出边 → post 排序边 id"的逆置换——参考实现的固有构造）。
    // 任务契约要求与现有 graph.npz 逐数组一致，此处忠实复刻该构造（已用 Python
    // 核验：现网 graph.npz 的 out_edges 恰为 arange）。若未来要修正为真实出边
    // 逆置换，需双端同步重建（会改变动力学且破坏文件兼容，见本次报告）。
    let mut order2: Vec<u32> = (0..e as u32).collect();
    order2.sort_by_key(|&i| pre[i as usize]);
    let out_edges: Vec<u32> = order2;
    let mut out_indptr = vec![0i64; n + 1];
    for &p in &pre {
        out_indptr[p as usize + 1] += 1;
    }
    for i in 0..n {
        out_indptr[i + 1] += out_indptr[i];
    }

    // 7) 左右轴
    let lr_axis = pick_lr_axis(&coords, &side, n);
    info!("左右轴判定: {:?}（可分性最大轴）", lr_axis);

    // 8) 序列化（zip deflate + npy v1.0）
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let file = std::fs::File::create(out_path)
        .map_err(|e| format!("create {}: {e}", out_path.display()))?;
    let mut zw = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    npz_put(
        &mut zw,
        opts,
        "n_neurons",
        "<i8",
        &[],
        &(n as i64).to_le_bytes(),
    )?;
    npz_put(&mut zw, opts, "root_ids", "<u8", &[n], &u64_bytes(&ids))?;
    npz_put(&mut zw, opts, "coords", "<f4", &[n, 3], &f32_bytes(&coords))?;
    let sc_join = sc.join("\0");
    let (d, b) = unicode_bytes(&sc_join);
    npz_put(&mut zw, opts, "super_class", &d, &[], &b)?;
    let side_join = side.join("\0");
    let (d, b) = unicode_bytes(&side_join);
    npz_put(&mut zw, opts, "side", &d, &[], &b)?;
    npz_put(&mut zw, opts, "indices", "<u4", &[e], &u32_bytes(&indices))?;
    npz_put(
        &mut zw,
        opts,
        "indptr",
        "<i8",
        &[n + 1],
        &i64_bytes(&indptr),
    )?;
    npz_put(&mut zw, opts, "weight", "<f4", &[e], &f32_bytes(&weight_s))?;
    let nt_bytes: Vec<u8> = edge_nt.to_vec();
    npz_put(&mut zw, opts, "edge_nt", "|u1", &[e], &nt_bytes)?;
    npz_put(
        &mut zw,
        opts,
        "out_indptr",
        "<i8",
        &[n + 1],
        &i64_bytes(&out_indptr),
    )?;
    npz_put(
        &mut zw,
        opts,
        "out_edges",
        "<u4",
        &[e],
        &u32_bytes(&out_edges),
    )?;
    let (d, b) = unicode_bytes(&min_syn.to_string());
    npz_put(&mut zw, opts, "meta_min_syn", &d, &[], &b)?;
    let (d, b) = unicode_bytes(&W_SCALE.to_string());
    npz_put(&mut zw, opts, "meta_w_scale", &d, &[], &b)?;
    let lr_str = lr_axis
        .map(|a| a.to_string())
        .unwrap_or_else(|| "None".to_string());
    let (d, b) = unicode_bytes(&lr_str);
    npz_put(&mut zw, opts, "meta_lr_axis", &d, &[], &b)?;

    zw.finish().map_err(|e| format!("zip finish: {e}"))?;
    info!(
        "构建完成 {}: {} 神经元, {} 边, 总耗时 {}s",
        out_path.display(),
        n,
        e,
        t0.elapsed().as_secs()
    );
    Ok(BuildStats {
        n,
        edges: e,
        ms: t0.elapsed().as_millis(),
        lr_axis,
    })
}
