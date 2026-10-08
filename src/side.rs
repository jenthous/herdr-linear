//! 워크스페이스마다 띄운 사이드 pane 기록 (`side-panes.json`: 워크스페이스 id → pane id).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::config::{ensure_private_dir, write_private_file};

const FILE_NAME: &str = "side-panes.json";

pub struct SidePanes {
    path: PathBuf,
}

impl SidePanes {
    /// 상태 디렉터리의 `side-panes.json`.
    pub fn new(state_dir: &Path) -> SidePanes {
        SidePanes {
            path: state_dir.join(FILE_NAME),
        }
    }

    /// 그 워크스페이스의 사이드 pane id.
    pub fn get(&self, workspace: &str) -> Option<String> {
        self.read().remove(workspace)
    }

    /// 그 워크스페이스의 사이드 pane을 적는다. 있던 기록은 바꾼다.
    pub fn set(&self, workspace: &str, pane: &str) -> Result<()> {
        let mut map = self.read();
        map.insert(workspace.to_string(), pane.to_string());
        self.write(&map)
    }

    /// 그 워크스페이스 기록이 `pane`일 때만 지운다. 다른 프로세스가 새로 적은 기록은 둔다.
    pub fn remove(&self, workspace: &str, pane: &str) -> Result<()> {
        let mut map = self.read();
        if map.get(workspace).map(String::as_str) != Some(pane) {
            return Ok(());
        }
        map.remove(workspace);
        self.write(&map)
    }

    /// 모든 기록 (워크스페이스 id, pane id), 워크스페이스 순.
    pub fn all(&self) -> Vec<(String, String)> {
        self.read().into_iter().collect()
    }

    /// 읽지 못하거나 모양이 틀리면 빈 기록.
    fn read(&self) -> BTreeMap<String, String> {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// 임시 파일에 쓰고 이름을 바꿔서, 쓰다 끊겨도 반쯤 쓴 파일이 남지 않게 한다.
    fn write(&self, map: &BTreeMap<String, String>) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            ensure_private_dir(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        write_private_file(&tmp, serde_json::to_string_pretty(map)?.as_bytes())?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_one_pane_per_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let panes = SidePanes::new(dir.path());
        assert_eq!(panes.get("w1"), None);
        panes.set("w1", "w1:p3").unwrap();
        panes.set("w2", "w2:p1").unwrap();
        panes.set("w1", "w1:p4").unwrap();
        assert_eq!(panes.get("w1").as_deref(), Some("w1:p4"));
        assert_eq!(panes.get("w2").as_deref(), Some("w2:p1"));
    }

    #[test]
    fn removes_only_its_own_record() {
        let dir = tempfile::tempdir().unwrap();
        let panes = SidePanes::new(dir.path());
        panes.set("w1", "w1:p3").unwrap();
        panes.set("w2", "w2:p1").unwrap();
        panes.remove("w1", "w1:p9").unwrap();
        assert_eq!(
            panes.get("w1").as_deref(),
            Some("w1:p3"),
            "다른 pane의 기록은 둔다"
        );
        panes.remove("w1", "w1:p3").unwrap();
        assert_eq!(panes.get("w1"), None);
        assert_eq!(
            panes.get("w2").as_deref(),
            Some("w2:p1"),
            "다른 워크스페이스는 그대로"
        );
        panes.remove("w9", "w9:p1").unwrap();
    }

    #[test]
    fn broken_file_reads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let panes = SidePanes::new(dir.path());
        for broken in ["", "not json", "[1, 2]", "{\"w1\": 3}"] {
            fs::write(dir.path().join(FILE_NAME), broken).unwrap();
            assert_eq!(panes.get("w1"), None, "{broken:?}");
        }
        panes.set("w1", "w1:p1").unwrap();
        assert_eq!(
            panes.get("w1").as_deref(),
            Some("w1:p1"),
            "깨진 파일은 새로 쓴다"
        );
    }

    #[cfg(unix)]
    #[test]
    fn file_is_private_and_leaves_no_temp_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        // 상태 디렉터리가 아직 없어도 만든다
        let state = dir.path().join("state");
        let panes = SidePanes::new(&state);
        panes.set("w1", "w1:p1").unwrap();
        let mode = fs::metadata(state.join(FILE_NAME))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(!state.join("side-panes.json.tmp").exists());
    }

    #[test]
    fn all_lists_every_record() {
        let dir = tempfile::tempdir().unwrap();
        let panes = SidePanes::new(dir.path());
        assert!(panes.all().is_empty());
        panes.set("w2", "w2:p1").unwrap();
        panes.set("w1", "w1:p3").unwrap();
        assert_eq!(
            panes.all(),
            vec![
                ("w1".to_string(), "w1:p3".to_string()),
                ("w2".to_string(), "w2:p1".to_string()),
            ]
        );
    }
}
