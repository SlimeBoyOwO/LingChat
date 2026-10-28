//! 基于 sqlite-vec 的本地向量存储。
//!
//! 复用系统 SQLite，数据落在数据目录的 vector_index.db。sqlite-vec 的 C 源码
//! 由 crate 静态编译进二进制，进程内通过 sqlite3_auto_extension 注册一次，
//! 移动端不需要额外分发动态库。
//!
//! 这里只提供最小的写入、删除与 Top-K 检索；笔记 / 记忆的文本切分、嵌入计算
//! 与结果回查仍由调用方负责。

use std::path::Path;
use std::sync::{Mutex, MutexGuard, Once};

use rusqlite::{Connection, params};
use thiserror::Error;

const TABLE: &str = "vector_entries";

static REGISTER_EXTENSION: Once = Once::new();

/// 注册 sqlite-vec 扩展。
///
/// sqlite3_auto_extension 对之后打开的所有连接生效，因此必须在第一次建连前
/// 调用；重复调用是幂等的。
fn register_extension() {
    REGISTER_EXTENSION.call_once(|| unsafe {
        // sqlite-vec 的 Rust 绑定把入口声明成无参 extern fn，这里补回真实的
        // sqlite3 扩展初始化签名，再交给 auto_extension 注册。
        let entry = std::mem::transmute::<*const (), rusqlite::auto_extension::RawAutoExtension>(
            sqlite_vec::sqlite3_vec_init as *const (),
        );
        rusqlite::ffi::sqlite3_auto_extension(Some(entry));
    });
}

/// 一次向量检索结果。
#[derive(Clone, Debug, PartialEq)]
pub struct VectorHit {
    pub id: String,
    /// 余弦距离，越小越相似。
    pub distance: f32,
}

/// 向量库读写失败。
#[derive(Debug, Error)]
pub enum VectorError {
    #[error("打开向量库失败: {0}")]
    Open(#[source] rusqlite::Error),
    #[error("创建数据目录失败: {0}")]
    Io(#[from] std::io::Error),
    #[error("向量库操作失败: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("向量维度应为 {expected}，实际为 {actual}")]
    Dimension { expected: usize, actual: usize },
    #[error("向量库连接锁已损坏")]
    Lock,
}

/// 单文件、进程内共享的向量库。
///
/// `rusqlite::Connection` 不是 `Sync`，这里用互斥锁串行化访问；当前的数据规模
/// 与调用频率下不需要连接池。维度在建库时固定：vec0 表的 DDL 依赖它，同一库文件
/// 里不允许出现两种维度。
pub struct VectorStore {
    conn: Mutex<Connection>,
    dimension: usize,
}

impl VectorStore {
    /// 打开或创建指定路径、指定维度的向量库，并确保 vec0 表存在。
    pub fn open(path: impl AsRef<Path>, dimension: usize) -> Result<Self, VectorError> {
        if dimension == 0 {
            return Err(VectorError::Dimension {
                expected: 0,
                actual: 0,
            });
        }
        register_extension();
        let conn = Connection::open(path).map_err(VectorError::Open)?;
        // 预热与检索可能同时写同一个角色的库，短暂争用等一等而不是直接报 locked。
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(VectorError::Sqlite)?;
        conn.execute_batch(&format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS {TABLE} USING vec0(\
             id TEXT PRIMARY KEY, \
             embedding FLOAT[{dimension}] distance_metric=cosine)"
        ))?;
        Ok(Self {
            conn: Mutex::new(conn),
            dimension,
        })
    }

    /// 本库的向量维度。
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// 写入或覆盖一条向量。外部 ID 相同的旧向量会被替换。
    pub fn upsert(&self, id: &str, vector: &[f32]) -> Result<(), VectorError> {
        self.upsert_many([(id, vector)])
    }

    /// 在一个事务内批量写入或覆盖向量。
    ///
    /// 同一 ID 只保留最后一条；任意一条维度不合法时整批回滚。
    pub fn upsert_many<'a, I>(&self, items: I) -> Result<(), VectorError>
    where
        I: IntoIterator<Item = (&'a str, &'a [f32])>,
    {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        {
            let mut delete = tx.prepare_cached(&format!("DELETE FROM {TABLE} WHERE id = ?1"))?;
            let mut insert = tx.prepare_cached(&format!(
                "INSERT INTO {TABLE}(id, embedding) VALUES (?1, ?2)"
            ))?;
            for (id, vector) in items {
                let blob = self.vector_to_blob(vector)?;
                delete.execute([id])?;
                insert.execute(params![id, blob])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// 删除一条向量；不存在时视为成功。
    pub fn delete(&self, id: &str) -> Result<(), VectorError> {
        let conn = self.conn()?;
        conn.execute(&format!("DELETE FROM {TABLE} WHERE id = ?1"), [id])?;
        Ok(())
    }

    /// 在一个事务内批量删除；不存在的 ID 视为成功。
    pub fn delete_many<'a, I>(&self, ids: I) -> Result<(), VectorError>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        {
            let mut delete = tx.prepare_cached(&format!("DELETE FROM {TABLE} WHERE id = ?1"))?;
            for id in ids {
                delete.execute([id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// 列出库内所有外部 ID（不含向量），供调用方对账清理。
    pub fn ids(&self) -> Result<Vec<String>, VectorError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!("SELECT id FROM {TABLE}"))?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// 按余弦距离返回最相近的 top_k 条记录。
    pub fn search(&self, query: &[f32], top_k: usize) -> Result<Vec<VectorHit>, VectorError> {
        if top_k == 0 {
            return Ok(Vec::new());
        }
        let blob = self.vector_to_blob(query)?;
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT id, distance FROM {TABLE} WHERE embedding MATCH ?1 AND k = ?2"
        ))?;
        let rows = stmt.query_map(params![blob, top_k as i64], |row| {
            Ok(VectorHit {
                id: row.get(0)?,
                distance: row.get(1)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    fn conn(&self) -> Result<MutexGuard<'_, Connection>, VectorError> {
        self.conn.lock().map_err(|_| VectorError::Lock)
    }

    /// 按本库维度校验并序列化为 sqlite-vec 的 float BLOB。
    fn vector_to_blob(&self, vector: &[f32]) -> Result<Vec<u8>, VectorError> {
        if vector.len() != self.dimension {
            return Err(VectorError::Dimension {
                expected: self.dimension,
                actual: vector.len(),
            });
        }
        // sqlite-vec 直接按 float 解释 BLOB；所有目标平台都是小端。
        Ok(vector.iter().flat_map(|v| v.to_le_bytes()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 库维度由调用方指定，测试用小维度即可覆盖建表与校验逻辑。
    const DIM: usize = 8;

    fn temp_store() -> (tempfile::TempDir, VectorStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::open(dir.path().join("vector_index.db"), DIM).unwrap();
        (dir, store)
    }

    fn axis(index: usize) -> Vec<f32> {
        let mut vector = vec![0.0; DIM];
        vector[index] = 1.0;
        vector
    }

    #[test]
    fn write_search_replace_and_delete() {
        let (_dir, store) = temp_store();
        store
            .upsert_many([("a", axis(0).as_slice()), ("b", axis(1).as_slice())])
            .unwrap();
        store
            .upsert_many([("a", axis(2).as_slice()), ("c", axis(0).as_slice())])
            .unwrap();

        // a 已被覆盖到 axis(2)。
        let hits = store.search(&axis(2), 1).unwrap();
        assert_eq!(hits[0].id, "a");
        assert!(hits[0].distance < 1e-6);

        // c 与查询同向，a 和 b 正交。
        let hits = store.search(&axis(0), 10).unwrap();
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].id, "c");
        assert!((hits[1].distance - 1.0).abs() < 1e-6);
        assert!((hits[2].distance - 1.0).abs() < 1e-6);

        store.delete("c").unwrap();
        store.delete("c").unwrap();
        assert_eq!(store.search(&axis(0), 10).unwrap().len(), 2);

        let mut ids = store.ids().unwrap();
        ids.sort();
        assert_eq!(ids, vec!["a".to_string(), "b".to_string()]);

        store.delete_many(["a", "不存在的 ID"]).unwrap();
        let mut ids = store.ids().unwrap();
        ids.sort();
        assert_eq!(ids, vec!["b".to_string()]);
    }

    #[test]
    fn rejects_invalid_input_and_rolls_back_batch() {
        let (_dir, store) = temp_store();
        assert!(store.search(&axis(0), DIM).unwrap().is_empty());
        assert!(store.search(&axis(0), 0).unwrap().is_empty());

        let error = store.upsert("a", &[1.0, 0.0]).unwrap_err();
        assert!(matches!(
            error,
            VectorError::Dimension {
                expected: DIM,
                actual: 2
            }
        ));

        let error = store
            .upsert_many([("b", axis(0).as_slice()), ("c", [1.0f32, 0.0].as_slice())])
            .unwrap_err();
        assert!(matches!(
            error,
            VectorError::Dimension {
                expected: DIM,
                actual: 2
            }
        ));
        assert!(store.search(&axis(0), DIM).unwrap().is_empty());
    }

    /// 维度是建库时固定的：不同维度各自成库，互不串味。
    #[test]
    fn dimension_is_per_store() {
        let dir = tempfile::tempdir().unwrap();
        let small = VectorStore::open(dir.path().join("small.db"), 4).unwrap();
        let large = VectorStore::open(dir.path().join("large.db"), 16).unwrap();
        assert_eq!(small.dimension(), 4);
        assert_eq!(large.dimension(), 16);

        small.upsert("a", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        assert!(matches!(
            large.upsert("a", &[1.0, 0.0, 0.0, 0.0]).unwrap_err(),
            VectorError::Dimension {
                expected: 16,
                actual: 4
            }
        ));
    }
}
