//! redb 存储层：便签逐条存储，元数据单独一张表。
//!
//! 表设计：
//! | 表      | key      | value                          | 说明 |
//! |---------|----------|--------------------------------|------|
//! | notes   | u64 (id) | &[u8] (Note 的 JSON 字节)      | 一条便签一个 key，改动只写这一条 |
//! | meta    | &str     | &str                           | next_id / last_open / schema_version |
//!
//! 数据库文件损坏或无法打开时直接返回错误（fail-fast），不会静默退回 JSON。

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use redb::{Database, ReadableTable, TableDefinition};

use crate::model::{Note, Notes};

pub type DbResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const NOTES: TableDefinition<u64, &[u8]> = TableDefinition::new("notes");
const META: TableDefinition<&str, &str> = TableDefinition::new("meta");

const KEY_NEXT_ID: &str = "next_id";
const KEY_LAST_OPEN: &str = "last_open";
const KEY_SCHEMA: &str = "schema_version";
const SCHEMA_VERSION: &str = "1";

const DB_FILE: &str = "memo.redb";
const LEGACY_FILE: &str = "notes.json";
const LEGACY_BACKUP: &str = "notes.json.migrated.bak";

/// 数据目录：
/// - Windows：`%APPDATA%\Memo`
/// - 其它：`$XDG_DATA_HOME/memo` 或 `$HOME/.local/share/memo`
pub fn data_dir() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Memo")
    } else {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
            })
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("memo")
    }
}

pub struct Db {
    inner: Database,
    legacy_path: PathBuf,
    /// 本次启动从旧 notes.json 迁移进来的便签数量，用于提示用户。
    pub migrated_from_legacy: Option<usize>,
}

impl Db {
    pub fn open() -> DbResult<Self> {
        Self::open_at(&data_dir())
    }

    /// 可指定目录，便于测试；正式入口是 [`Db::open`]。
    pub fn open_at(dir: &Path) -> DbResult<Self> {
        fs::create_dir_all(dir)?;
        let path = dir.join(DB_FILE);
        let legacy_path = dir.join(LEGACY_FILE);
        let inner = Database::create(&path)?;

        let mut db = Self {
            inner,
            legacy_path,
            migrated_from_legacy: None,
        };

        db.ensure_tables()?;

        // 库为空且存在旧版 JSON 时，一次性导入并备份旧文件。
        if db.is_empty()? && db.legacy_path.exists() {
            let count = db.migrate_from_legacy()?;
            db.migrated_from_legacy = Some(count);
        }

        Ok(db)
    }

    fn is_empty(&self) -> DbResult<bool> {
        let read = self.inner.begin_read()?;
        let table = read.open_table(NOTES)?;
        // iter 必须绑定到局部变量：表达式末尾才析构的临时值会借用 table，
        // 而 table 会先于该临时值被 drop。
        let mut iter = table.iter()?;
        let empty = iter.next().is_none();
        Ok(empty)
    }

    // ---------- 读 ----------

    /// 一次性把库内容读进内存缓存（启动时调用一次）。
    pub fn load(&self) -> DbResult<Notes> {
        let read = self.inner.begin_read()?;
        let mut notes = Vec::new();
        {
            let table = read.open_table(NOTES)?;
            for entry in table.iter()? {
                let (_key, value) = entry?;
                notes.push(serde_json::from_slice::<Note>(value.value())?);
            }
        }

        let (mut next_id, last_open) = {
            let meta = read.open_table(META)?;
            let next_id = meta
                .get(KEY_NEXT_ID)?
                .and_then(|v| v.value().parse::<u64>().ok())
                .unwrap_or(0);
            let last_open = meta
                .get(KEY_LAST_OPEN)?
                .and_then(|v| v.value().parse::<u64>().ok());
            (next_id, last_open)
        };

        // 元数据落后于实际数据时，从最大 id 推回，避免新建便签覆盖已有记录。
        let max_id = notes.iter().map(|n| n.id).max().unwrap_or(0);
        next_id = next_id.max(max_id.saturating_add(1));

        Ok(Notes {
            notes,
            next_id,
            last_open,
        })
    }

    // ---------- 写 ----------

    pub fn delete(&self, id: u64) -> DbResult<()> {
        let write = self.inner.begin_write()?;
        {
            let mut table = write.open_table(NOTES)?;
            let _ = table.remove(id)?;
        }
        write.commit()?;
        Ok(())
    }

    /// 一次事务完成一次自动保存：写入当前便签 + 两个元数据。
    /// 比拆成三个事务更省 IO，也避免中途崩溃造成状态不一致。
    pub fn save_state(
        &self,
        note: Option<&Note>,
        last_open: Option<u64>,
        next_id: u64,
    ) -> DbResult<()> {
        let bytes = note.map(serde_json::to_vec).transpose()?;
        let write = self.inner.begin_write()?;
        {
            if let (Some(note), Some(bytes)) = (note, &bytes) {
                let mut table = write.open_table(NOTES)?;
                let _ = table.insert(note.id, bytes.as_slice())?;
            }
            let mut meta = write.open_table(META)?;
            let _ = meta.insert(KEY_NEXT_ID, next_id.to_string().as_str())?;
            match last_open {
                Some(id) => {
                    let _ = meta.insert(KEY_LAST_OPEN, id.to_string().as_str())?;
                }
                None => {
                    let _ = meta.remove(KEY_LAST_OPEN)?;
                }
            }
        }
        write.commit()?;
        Ok(())
    }

    /// 建库时确保两张表都存在并写入 schema 版本。
    /// 写事务里 open_table 会创建缺失的表，否则后续 load()/is_empty()
    /// 只读打开一张尚不存在的表会直接报 TableDoesNotExist。
    fn ensure_tables(&self) -> DbResult<()> {
        let write = self.inner.begin_write()?;
        {
            let _ = write.open_table(NOTES)?;
            let mut meta = write.open_table(META)?;
            let _ = meta.insert(KEY_SCHEMA, SCHEMA_VERSION)?;
        }
        write.commit()?;
        Ok(())
    }

    // ---------- 旧版迁移 ----------

    fn migrate_from_legacy(&self) -> DbResult<usize> {
        let raw = fs::read_to_string(&self.legacy_path)?;
        let old: Notes = serde_json::from_str(&raw)?;

        let max_id = old.notes.iter().map(|n| n.id).max().unwrap_or(0);
        let next_id = old.next_id.max(max_id.saturating_add(1));

        let write = self.inner.begin_write()?;
        {
            let mut table = write.open_table(NOTES)?;
            for note in &old.notes {
                let bytes = serde_json::to_vec(note)?;
                let _ = table.insert(note.id, bytes.as_slice())?;
            }
            let mut meta = write.open_table(META)?;
            let _ = meta.insert(KEY_NEXT_ID, next_id.to_string().as_str())?;
            if let Some(id) = old.last_open {
                let _ = meta.insert(KEY_LAST_OPEN, id.to_string().as_str())?;
            }
        }
        write.commit()?;

        // 重命名而非删除：保留可回溯的备份，同时避免下次启动重复导入。
        let backup = self.legacy_path.with_file_name(LEGACY_BACKUP);
        fs::rename(&self.legacy_path, &backup)?;

        Ok(old.notes.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("memo-db-test-{tag}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn crud_and_meta_roundtrip() {
        let dir = temp_dir("crud");
        let db = Db::open_at(&dir).unwrap();

        let mut note = Note::new(7, 2);
        note.title = "标题".into();
        note.content = "第一行\n第二行".into();
        db.save_state(Some(&note), Some(7), 8).unwrap();

        let loaded = db.load().unwrap();
        assert_eq!(loaded.notes.len(), 1);
        assert_eq!(loaded.notes[0], note);
        assert_eq!(loaded.next_id, 8);
        assert_eq!(loaded.last_open, Some(7));

        db.delete(7).unwrap();
        assert!(db.load().unwrap().notes.is_empty());

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn migrates_legacy_json_once() {
        let dir = temp_dir("migrate");
        let legacy = dir.join(LEGACY_FILE);
        let json = r#"{"notes":[{"id":3,"title":"旧便签","content":"内容","color":1,"created":1,"updated":2}],"next_id":4,"last_open":3}"#;
        fs::write(&legacy, json).unwrap();

        let db = Db::open_at(&dir).unwrap();
        assert_eq!(db.migrated_from_legacy, Some(1));
        let loaded = db.load().unwrap();
        assert_eq!(loaded.notes.len(), 1);
        assert_eq!(loaded.notes[0].title, "旧便签");
        assert_eq!(loaded.next_id, 4);
        assert_eq!(loaded.last_open, Some(3));
        assert!(!legacy.exists());
        assert!(dir.join(LEGACY_BACKUP).exists());

        // 再次打开不会重复导入（先释放上一个实例持有的文件锁）
        drop(db);
        let db2 = Db::open_at(&dir).unwrap();
        assert_eq!(db2.migrated_from_legacy, None);
        assert_eq!(db2.load().unwrap().notes.len(), 1);

        fs::remove_dir_all(dir).ok();
    }
}
