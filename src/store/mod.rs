//! SQLite 캐시. 조회한 것을 저장해 다음에 즉시 보여준다.

mod cache;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rusqlite::{Connection, Transaction, TransactionBehavior};

use crate::config::ensure_private_dir;
use crate::i18n::t;

/// 스키마 버전. 1(관계 테이블 없음)이면 테이블만 더하고, 그 밖의 다른 값이면 캐시를 비우고 새로 만든다.
pub const SCHEMA_VERSION: i64 = 2;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS issues (
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
CREATE TABLE IF NOT EXISTS comments (
  issue_id   TEXT PRIMARY KEY,
  data       TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS view_results (
  view_key   TEXT PRIMARY KEY,
  issue_ids  TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS team_refs (
  team_id    TEXT PRIMARY KEY,
  data       TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS workspace_labels (
  id         INTEGER PRIMARY KEY CHECK (id = 1),
  data       TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS branch_map (
  repo       TEXT NOT NULL,
  branch     TEXT NOT NULL,
  identifier TEXT,
  fetched_at INTEGER NOT NULL,
  PRIMARY KEY (repo, branch)
);
CREATE TABLE IF NOT EXISTS relations (
  issue_id   TEXT PRIMARY KEY,
  data       TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
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
                Store::open_once(path).context(t().cache_recreate_failed)
            }
            Err(e) => Err(e.context(t().cache_open_failed)),
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
        set_wal(&conn)?;
        let store = Store { conn };
        store.init()?;
        Ok(store)
    }

    /// 스키마를 만든다. 다른 프로세스가 같은 파일을 동시에 처음 열 수 있으므로,
    /// 버전 확인부터 `user_version` 기록까지를 한 쓰기 트랜잭션으로 묶는다.
    fn init(&self) -> Result<()> {
        if user_version(&self.conn)? == SCHEMA_VERSION {
            return Ok(());
        }
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        // 잠금을 기다리는 사이 다른 프로세스가 만들었을 수 있으니 다시 확인한다
        let version = user_version(&tx)?;
        if version != SCHEMA_VERSION {
            // 0은 새 파일이거나 만들다 끊긴 파일, 1은 관계 테이블만 없는 파일이다.
            // 테이블을 모두 IF NOT EXISTS로 만들므로 둘은 비우지 않고 채운다
            if !matches!(version, 0 | 1) {
                drop_all_tables(&tx)?;
            }
            tx.execute_batch(SCHEMA)?;
            tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        tx.commit()?;
        Ok(())
    }
}

fn user_version(conn: &Connection) -> Result<i64> {
    Ok(conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
}

/// WAL로 바꾼다. 다른 연결이 같은 순간 바꾸는 중이면 busy_timeout이 적용되지 않아
/// 바로 BUSY가 나므로, 5초 안에서 직접 다시 시도한다.
fn set_wal(conn: &Connection) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match conn.pragma_update(None, "journal_mode", "WAL") {
            Ok(()) => return Ok(()),
            Err(rusqlite::Error::SqliteFailure(f, _))
                if matches!(
                    f.code,
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                ) && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(e.into()),
        }
    }
}

fn drop_all_tables(conn: &Connection) -> Result<()> {
    let names: Vec<String> = {
        let mut st = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        )?;
        st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
    };
    for name in names {
        conn.execute_batch(&format!("DROP TABLE IF EXISTS \"{name}\""))?;
    }
    Ok(())
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

    #[test]
    fn concurrent_first_open_succeeds() {
        // 팔레트와 사이드 패널이 처음 뜰 때처럼, 빈 경로를 두 프로세스가 동시에 연다
        for round in 0..20 {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("cache.db");
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
            let handles: Vec<_> = (0..2)
                .map(|i| {
                    let (path, barrier) = (path.clone(), barrier.clone());
                    std::thread::spawn(move || -> Result<(), String> {
                        barrier.wait();
                        let s = Store::open(&path).map_err(|e| format!("open: {e:#}"))?;
                        s.meta_set(&format!("k{i}"), "v")
                            .map_err(|e| format!("write: {e:#}"))
                    })
                })
                .collect();
            for h in handles {
                let r = h.join().unwrap();
                assert!(r.is_ok(), "round {round}: {r:?}");
            }
        }
    }

    #[test]
    fn half_initialized_file_recovers() {
        // 스키마를 만들다가 user_version을 쓰기 전에 끊긴 파일
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
                .unwrap();
        }
        let store = Store::open(&path).unwrap();
        store.meta_set("k", "v").unwrap();
        assert_eq!(store.meta_get("k").unwrap().as_deref(), Some("v"));
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

    #[test]
    fn version_1_file_keeps_its_data_and_gains_relations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.db");
        {
            let store = Store::open(&path).unwrap();
            store.meta_set("k", "v").unwrap();
            // 버전 1 파일: 관계 테이블이 없다
            store.conn.execute_batch("DROP TABLE relations").unwrap();
            store.conn.pragma_update(None, "user_version", 1).unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(
            store.meta_get("k").unwrap().as_deref(),
            Some("v"),
            "기존 캐시가 남는다"
        );
        let v: i64 = store
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        store
            .set_relations("i1", &crate::linear::types::IssueRelations::default(), 1)
            .unwrap();
    }
}
