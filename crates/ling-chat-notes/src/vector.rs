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

/// 与 embedding 模块的句向量引擎（bge-small-zh-v1.5）输出维度一致。
pub const EMBEDDING_DIM: usize = 512;

const TABLE: &str = "vector_entries";

static REGISTER_EXTENSION: Once = Once::new();

/// 注册 sqlite-vec 扩展。
///
/// sqlite3_auto_extension 对之后打开的所有连接生效，因此必须在第一次建连前
/// 调用；重复调用是幂等的。
fn register_extension() {
    REGISTER_EXTENSION.call_once(|| unsafe {
        let entry = std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ());
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
/// 与调用频率下不需要连接池。
pub struct VectorStore {
    conn: Mutex<Connection>,
}

impl VectorStore {
    /// 打开或创建指定路径的向量库，并确保 vec0 表存在。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, VectorError> {
        register_extension();
        let conn = Connection::open(path).map_err(VectorError::Open)?;
        conn.execute_batch(&format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS {TABLE} USING vec0(\
             id TEXT PRIMARY KEY, \
             embedding FLOAT[{EMBEDDING_DIM}] distance_metric=cosine)"
        ))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 在数据目录下打开 vector_index.db。
    ///
    /// 索引可由笔记 / 记忆重新生成，因此和 game_data 目录分开存放，避免被资源
    /// 同步覆盖。
    pub fn open_in_data_dir(data_dir: impl AsRef<Path>) -> Result<Self, VectorError> {
        let data_dir = data_dir.as_ref();
        std::fs::create_dir_all(data_dir)?;
        Self::open(data_dir.join("vector_index.db"))
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
                let blob = vector_to_blob(vector)?;
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
        let blob = vector_to_blob(query)?;
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
}

fn vector_to_blob(vector: &[f32]) -> Result<Vec<u8>, VectorError> {
    if vector.len() != EMBEDDING_DIM {
        return Err(VectorError::Dimension {
            expected: EMBEDDING_DIM,
            actual: vector.len(),
        });
    }
    // sqlite-vec 直接按 float 解释 BLOB；所有目标平台都是小端。
    Ok(vector.iter().flat_map(|v| v.to_le_bytes()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (tempfile::TempDir, VectorStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::open(dir.path().join("vector_index.db")).unwrap();
        (dir, store)
    }

    fn axis(index: usize) -> Vec<f32> {
        let mut vector = vec![0.0; EMBEDDING_DIM];
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
        assert!(store.search(&axis(0), 8).unwrap().is_empty());
        assert!(store.search(&axis(0), 0).unwrap().is_empty());

        let error = store.upsert("a", &[1.0, 0.0]).unwrap_err();
        assert!(matches!(
            error,
            VectorError::Dimension {
                expected: EMBEDDING_DIM,
                actual: 2
            }
        ));

        let error = store
            .upsert_many([("b", axis(0).as_slice()), ("c", [1.0f32, 0.0].as_slice())])
            .unwrap_err();
        assert!(matches!(
            error,
            VectorError::Dimension {
                expected: EMBEDDING_DIM,
                actual: 2
            }
        ));
        assert!(store.search(&axis(0), 8).unwrap().is_empty());
    }
}
