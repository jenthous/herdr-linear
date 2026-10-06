//! SQLite 캐시. 조회한 것을 저장해 다음에 즉시 보여준다.

mod cache;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::config::ensure_private_dir;

/// 스키마 버전. 다르면 캐시를 비우고 새로 만든다.
pub const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE issues (
  id          TEXT PRIMARY KEY,
  identifier  TEXT NOT NULL UNIQUE,
  team_id     TEXT NOT NULL,
  number      INTEGER NOT NULL,
  title       TEXT NOT NULL,
  description TEXT,
  state_type  TEXT NOT NULL,
  priority    INTEGER NOT NULL,
  assignee_id TEXT,
  updated_at  TEXT NOT NULL,
  data        TEXT NOT NULL,
  fetched_at  INTEGER NOT NULL,
  viewed_at   INTEGER
);
CREATE TABLE comments (
  issue_id   TEXT PRIMARY KEY,
  data       TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);
CREATE TABLE view_results (
  view_key   TEXT PRIMARY KEY,
  issue_ids  TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);
CREATE TABLE team_refs (
  team_id    TEXT PRIMARY KEY,
  data       TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);
CREATE TABLE workspace_labels (
  id         INTEGER PRIMARY KEY CHECK (id = 1),
  data       TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);
CREATE TABLE branch_map (
  repo       TEXT NOT NULL,
  branch     TEXT NOT NULL,
  identifier TEXT,
  fetched_at INTEGER NOT NULL,
  PRIMARY KEY (repo, branch)
);
";

pub struct Store {
    pub(crate) conn: Connection,
}

impl Store {
    /// 캐시 DB를 연다. 파일이 손상됐으면 지우고 새로 만든다.
    /// 잠금(BUSY) 같은 일시적 오류에는 파일을 지우지 않는다.
    pub fn open(path: &Path) -> Result<Store> {
        if let Some(dir) = path.parent() {
            ensure_private_dir(dir)?;
        }
        match Store::open_once(path) {
            Ok(store) => Ok(store),
            Err(e) if is_corrupt(&e) => {
                remove_db_files(path);
                Store::open_once(path).context("캐시 DB를 다시 만들지 못했어요")
            }
            Err(e) => Err(e.context("캐시 DB를 열지 못했어요")),
        }
    }

    pub fn open_in_memory() -> Result<Store> {
        let store = Store {
            conn: Connection::open_in_memory()?,
        };
        store.init()?;
        Ok(store)
    }

    fn open_once(path: &Path) -> Result<Store> {
        let conn = Connection::open(path)?;
        set_private(path);
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let store = Store { conn };
        store.init()?;
        Ok(store)
    }

    fn init(&self) -> Result<()> {
        let version: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version == SCHEMA_VERSION {
            return Ok(());
        }
        if version != 0 {
            self.drop_all_tables()?;
        }
        self.conn.execute_batch(SCHEMA)?;
        self.conn
            .pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(())
    }

    fn drop_all_tables(&self) -> Result<()> {
        let names: Vec<String> = {
            let mut st = self.conn.prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            )?;
            st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
        };
        for name in names {
            self.conn
                .execute_batch(&format!("DROP TABLE IF EXISTS \"{name}\""))?;
        }
        Ok(())
    }
}

fn is_corrupt(e: &anyhow::Error) -> bool {
    matches!(
        e.downcast_ref::<rusqlite::Error>(),
        Some(rusqlite::Error::SqliteFailure(f, _))
            if matches!(f.code, rusqlite::ErrorCode::NotADatabase | rusqlite::ErrorCode::DatabaseCorrupt)
    )
}

/// DB 파일과 WAL 보조 파일을 지운다.
pub fn remove_db_files(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut p = path.as_os_str().to_owned();
        p.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(p));
    }
}

fn set_private(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_schema_with_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state").join("cache.db");
        let store = Store::open(&path).unwrap();
        let v: i64 = store
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        store.meta_set("k", "v").unwrap();
        drop(store);
        let again = Store::open(&path).unwrap();
        assert_eq!(again.meta_get("k").unwrap().as_deref(), Some("v"));
    }

    #[test]
    fn version_mismatch_recreates_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.db");
        {
            let store = Store::open(&path).unwrap();
            store.meta_set("k", "v").unwrap();
            store.conn.pragma_update(None, "user_version", 99).unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.meta_get("k").unwrap(), None);
    }

    #[test]
    fn corrupt_file_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.db");
        std::fs::write(
            &path,
            b"this is not a sqlite database at all, just junk bytes",
        )
        .unwrap();
        let store = Store::open(&path).unwrap();
        store.meta_set("k", "v").unwrap();
        assert_eq!(store.meta_get("k").unwrap().as_deref(), Some("v"));
    }

    #[test]
    fn only_corruption_triggers_recreate() {
        use rusqlite::ffi;
        let busy = rusqlite::Error::SqliteFailure(ffi::Error::new(ffi::SQLITE_BUSY), None);
        assert!(!is_corrupt(&anyhow::Error::from(busy)));
        let not_db = rusqlite::Error::SqliteFailure(ffi::Error::new(ffi::SQLITE_NOTADB), None);
        assert!(is_corrupt(&anyhow::Error::from(not_db)));
    }

    #[test]
    fn two_writers_wait_instead_of_failing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.db");
        let a = Store::open(&path).unwrap();
        a.meta_set("keep", "yes").unwrap();
        // a가 쓰기 잠금을 잡은 동안 b가 열고 쓰면, 파일을 지우지 않고 기다렸다가 성공해야 한다
        a.conn
            .execute_batch("BEGIN IMMEDIATE; INSERT INTO meta (key, value) VALUES ('a', '1');")
            .unwrap();
        let b_path = path.clone();
        let writer = std::thread::spawn(move || {
            let b = Store::open(&b_path).unwrap();
            b.meta_set("b", "2").unwrap();
        });
        std::thread::sleep(Duration::from_millis(300));
        a.conn.execute_batch("COMMIT").unwrap();
        writer.join().unwrap();
        assert_eq!(a.meta_get("keep").unwrap().as_deref(), Some("yes"));
        assert_eq!(a.meta_get("b").unwrap().as_deref(), Some("2"));
    }

    #[cfg(unix)]
    #[test]
    fn db_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state").join("cache.db");
        let _store = Store::open(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let dir_mode = std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);
    }
}
