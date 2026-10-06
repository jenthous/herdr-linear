//! 캐시 읽기/쓰기.

use anyhow::Result;
use rusqlite::{OptionalExtension, params};

use super::Store;
use crate::linear::types::{Comment, Issue};

impl Store {
    pub fn meta_get(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn meta_set(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// API 키의 워크스페이스가 바뀌었으면 캐시를 모두 비운다. 비웠으면 true.
    pub fn ensure_org(&self, org_id: &str) -> Result<bool> {
        let existing = self.meta_get("org_id")?;
        if existing.as_deref() == Some(org_id) {
            return Ok(false);
        }
        let cleared = existing.is_some();
        if cleared {
            self.clear_all()?;
        }
        self.meta_set("org_id", org_id)?;
        Ok(cleared)
    }

    pub fn clear_all(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM meta; DELETE FROM issues; DELETE FROM comments; DELETE FROM view_results;
             DELETE FROM team_refs; DELETE FROM workspace_labels; DELETE FROM branch_map;",
        )?;
        Ok(())
    }

    /// 이슈를 저장한다. 보관·휴지통 이슈는 저장하지 않고 캐시에서 지운다.
    /// 지운 이슈 id를 돌려준다.
    pub fn upsert_issues(&self, issues: &[Issue], now_ms: i64) -> Result<Vec<String>> {
        let tx = self.conn.unchecked_transaction()?;
        let mut removed = Vec::new();
        for issue in issues {
            if issue.is_gone() {
                tx.execute("DELETE FROM issues WHERE id = ?1", params![issue.id])?;
                tx.execute(
                    "DELETE FROM comments WHERE issue_id = ?1",
                    params![issue.id],
                )?;
                removed.push(issue.id.clone());
                continue;
            }
            // 팀 이동 등으로 식별자가 다른 이슈에 넘어간 경우 오래된 행을 지운다
            tx.execute(
                "DELETE FROM issues WHERE identifier = ?1 AND id <> ?2",
                params![issue.identifier, issue.id],
            )?;
            tx.execute(
                "INSERT INTO issues (id, identifier, team_id, number, title, description, state_type,
                                     priority, assignee_id, updated_at, data, fetched_at, viewed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL)
                 ON CONFLICT(id) DO UPDATE SET
                   identifier = excluded.identifier, team_id = excluded.team_id,
                   number = excluded.number, title = excluded.title,
                   description = excluded.description, state_type = excluded.state_type,
                   priority = excluded.priority, assignee_id = excluded.assignee_id,
                   updated_at = excluded.updated_at, data = excluded.data,
                   fetched_at = excluded.fetched_at",
                params![
                    issue.id,
                    issue.identifier,
                    issue.team.id,
                    issue.number,
                    issue.title,
                    issue.description,
                    issue.state.state_type,
                    issue.priority,
                    issue.assignee.as_ref().map(|a| a.id.as_str()),
                    issue.updated_at,
                    serde_json::to_string(issue)?,
                    now_ms,
                ],
            )?;
        }
        tx.commit()?;
        Ok(removed)
    }

    /// id 또는 식별자(대소문자 무시)로 찾는다.
    pub fn get_issue(&self, id_or_identifier: &str) -> Result<Option<Issue>> {
        let data: Option<String> = self
            .conn
            .query_row(
                "SELECT data FROM issues WHERE id = ?1 OR identifier = ?2",
                params![id_or_identifier, id_or_identifier.to_uppercase()],
                |r| r.get(0),
            )
            .optional()?;
        Ok(match data {
            Some(d) => Some(serde_json::from_str(&d)?),
            None => None,
        })
    }

    pub fn remove_issue(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM issues WHERE id = ?1", params![id])?;
        self.conn
            .execute("DELETE FROM comments WHERE issue_id = ?1", params![id])?;
        Ok(())
    }

    /// 캐시된 모든 이슈 (로컬 검색용).
    pub fn all_issues(&self) -> Result<Vec<Issue>> {
        let mut st = self.conn.prepare("SELECT data FROM issues")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            if let Ok(issue) = serde_json::from_str(&row?) {
                out.push(issue);
            }
        }
        Ok(out)
    }

    /// 보기 탭 결과(순서 있는 id 목록)를 저장한다.
    pub fn set_view(&self, key: &str, ids: &[String], now_ms: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO view_results (view_key, issue_ids, fetched_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(view_key) DO UPDATE SET issue_ids = excluded.issue_ids, fetched_at = excluded.fetched_at",
            params![key, serde_json::to_string(ids)?, now_ms],
        )?;
        Ok(())
    }

    /// 저장된 보기 결과와 저장 시각. 캐시에 없는 id는 건너뛴다.
    pub fn get_view(&self, key: &str) -> Result<Option<(Vec<Issue>, i64)>> {
        let row: Option<(String, i64)> = self
            .conn
            .query_row(
                "SELECT issue_ids, fetched_at FROM view_results WHERE view_key = ?1",
                params![key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((ids_json, fetched_at)) = row else {
            return Ok(None);
        };
        let ids: Vec<String> = serde_json::from_str(&ids_json)?;
        let mut issues = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(issue) = self.get_issue(&id)? {
                issues.push(issue);
            }
        }
        Ok(Some((issues, fetched_at)))
    }

    pub fn set_comments(&self, issue_id: &str, comments: &[Comment], now_ms: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO comments (issue_id, data, fetched_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(issue_id) DO UPDATE SET data = excluded.data, fetched_at = excluded.fetched_at",
            params![issue_id, serde_json::to_string(comments)?, now_ms],
        )?;
        Ok(())
    }

    pub fn get_comments(&self, issue_id: &str) -> Result<Option<(Vec<Comment>, i64)>> {
        let row: Option<(String, i64)> = self
            .conn
            .query_row(
                "SELECT data, fetched_at FROM comments WHERE issue_id = ?1",
                params![issue_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(match row {
            Some((data, at)) => Some((serde_json::from_str(&data)?, at)),
            None => None,
        })
    }

    /// 상세 화면을 열었다고 기록한다 ("최근 본").
    pub fn mark_viewed(&self, issue_id: &str, now_ms: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE issues SET viewed_at = ?2 WHERE id = ?1",
            params![issue_id, now_ms],
        )?;
        Ok(())
    }

    /// 최근 본 이슈 (최근 순).
    pub fn recent_viewed(&self, limit: usize) -> Result<Vec<Issue>> {
        let mut st = self.conn.prepare(
            "SELECT data FROM issues WHERE viewed_at IS NOT NULL ORDER BY viewed_at DESC LIMIT ?1",
        )?;
        let rows = st.query_map(params![limit as i64], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            if let Ok(issue) = serde_json::from_str(&row?) {
                out.push(issue);
            }
        }
        Ok(out)
    }

    /// `cutoff_ms`보다 오래 안 받고 안 본 이슈와 그 코멘트를 지운다. 지운 이슈 수.
    pub fn evict_older_than(&self, cutoff_ms: i64) -> Result<usize> {
        let n = self.conn.execute(
            "DELETE FROM issues WHERE MAX(fetched_at, COALESCE(viewed_at, 0)) < ?1",
            params![cutoff_ms],
        )?;
        self.conn.execute(
            "DELETE FROM comments WHERE issue_id NOT IN (SELECT id FROM issues)",
            [],
        )?;
        Ok(n)
    }

    /// (레포, 브랜치) → 연결된 이슈 식별자. 저장된 "없음"은 `Some((None, at))`.
    pub fn branch_get(&self, repo: &str, branch: &str) -> Result<Option<(Option<String>, i64)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT identifier, fetched_at FROM branch_map WHERE repo = ?1 AND branch = ?2",
                params![repo, branch],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    pub fn branch_set(
        &self,
        repo: &str,
        branch: &str,
        identifier: Option<&str>,
        now_ms: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO branch_map (repo, branch, identifier, fetched_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(repo, branch) DO UPDATE SET identifier = excluded.identifier, fetched_at = excluded.fetched_at",
            params![repo, branch, identifier, now_ms],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::IssueBuilder;

    fn store() -> Store {
        Store::open_in_memory().unwrap()
    }

    #[test]
    fn upsert_and_get_by_id_or_identifier() {
        let s = store();
        let issue = IssueBuilder::new("i1", "ENG-1", "로그인").build();
        s.upsert_issues(std::slice::from_ref(&issue), 100).unwrap();
        assert_eq!(s.get_issue("i1").unwrap(), Some(issue.clone()));
        assert_eq!(s.get_issue("eng-1").unwrap(), Some(issue));
        assert_eq!(s.get_issue("ENG-2").unwrap(), None);
    }

    #[test]
    fn upsert_keeps_viewed_at_and_updates_fields() {
        let s = store();
        s.upsert_issues(&[IssueBuilder::new("i1", "ENG-1", "옛 제목").build()], 100)
            .unwrap();
        s.mark_viewed("i1", 150).unwrap();
        s.upsert_issues(&[IssueBuilder::new("i1", "ENG-1", "새 제목").build()], 200)
            .unwrap();
        assert_eq!(s.get_issue("i1").unwrap().unwrap().title, "새 제목");
        assert_eq!(s.recent_viewed(10).unwrap().len(), 1);
    }

    #[test]
    fn archived_issue_is_removed_not_saved() {
        let s = store();
        s.upsert_issues(&[IssueBuilder::new("i1", "ENG-1", "a").build()], 100)
            .unwrap();
        s.set_comments("i1", &[], 100).unwrap();
        let removed = s
            .upsert_issues(
                &[IssueBuilder::new("i1", "ENG-1", "a").archived().build()],
                200,
            )
            .unwrap();
        assert_eq!(removed, vec!["i1".to_string()]);
        assert_eq!(s.get_issue("i1").unwrap(), None);
        assert_eq!(s.get_comments("i1").unwrap(), None);
    }

    #[test]
    fn identifier_moved_to_new_id_replaces_old_row() {
        let s = store();
        s.upsert_issues(&[IssueBuilder::new("old", "ENG-1", "a").build()], 100)
            .unwrap();
        s.upsert_issues(&[IssueBuilder::new("new", "ENG-1", "b").build()], 200)
            .unwrap();
        assert_eq!(s.get_issue("old").unwrap(), None);
        assert_eq!(s.get_issue("ENG-1").unwrap().unwrap().id, "new");
    }

    #[test]
    fn view_keeps_order_and_skips_missing() {
        let s = store();
        s.upsert_issues(
            &[
                IssueBuilder::new("a", "ENG-1", "a").build(),
                IssueBuilder::new("b", "ENG-2", "b").build(),
            ],
            100,
        )
        .unwrap();
        s.set_view("mine", &["b".into(), "gone".into(), "a".into()], 300)
            .unwrap();
        let (issues, at) = s.get_view("mine").unwrap().unwrap();
        assert_eq!(
            issues.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            vec!["b", "a"]
        );
        assert_eq!(at, 300);
        assert_eq!(s.get_view("all").unwrap(), None);
    }

    #[test]
    fn comments_roundtrip() {
        let s = store();
        let c = Comment {
            id: "c1".into(),
            body: "확인할게요".into(),
            created_at: "2026-10-01T00:00:00.000Z".into(),
            edited_at: None,
            user: None,
        };
        s.set_comments("i1", std::slice::from_ref(&c), 500).unwrap();
        assert_eq!(s.get_comments("i1").unwrap(), Some((vec![c], 500)));
    }

    #[test]
    fn recent_viewed_is_newest_first_with_limit() {
        let s = store();
        s.upsert_issues(
            &[
                IssueBuilder::new("a", "ENG-1", "a").build(),
                IssueBuilder::new("b", "ENG-2", "b").build(),
                IssueBuilder::new("c", "ENG-3", "c").build(),
            ],
            100,
        )
        .unwrap();
        s.mark_viewed("a", 1).unwrap();
        s.mark_viewed("c", 3).unwrap();
        s.mark_viewed("b", 2).unwrap();
        let ids: Vec<String> = s
            .recent_viewed(2)
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(ids, vec!["c", "b"]);
    }

    #[test]
    fn eviction_uses_latest_of_fetched_and_viewed() {
        let s = store();
        s.upsert_issues(
            &[
                IssueBuilder::new("old", "ENG-1", "a").build(),
                IssueBuilder::new("seen", "ENG-2", "b").build(),
            ],
            100,
        )
        .unwrap();
        s.set_comments("old", &[], 100).unwrap();
        s.mark_viewed("seen", 1_000).unwrap();
        assert_eq!(s.evict_older_than(500).unwrap(), 1);
        assert_eq!(s.get_issue("old").unwrap(), None);
        assert_eq!(s.get_comments("old").unwrap(), None);
        assert!(s.get_issue("seen").unwrap().is_some());
    }

    #[test]
    fn org_change_clears_everything() {
        let s = store();
        assert!(!s.ensure_org("org1").unwrap());
        s.upsert_issues(&[IssueBuilder::new("a", "ENG-1", "a").build()], 100)
            .unwrap();
        assert!(!s.ensure_org("org1").unwrap());
        assert!(s.ensure_org("org2").unwrap());
        assert_eq!(s.get_issue("a").unwrap(), None);
        assert_eq!(s.meta_get("org_id").unwrap().as_deref(), Some("org2"));
    }

    #[test]
    fn branch_map_stores_hits_and_misses() {
        let s = store();
        s.branch_set("/repo", "me/eng-1-x", Some("ENG-1"), 10)
            .unwrap();
        s.branch_set("/repo", "main", None, 20).unwrap();
        assert_eq!(
            s.branch_get("/repo", "me/eng-1-x").unwrap(),
            Some((Some("ENG-1".into()), 10))
        );
        assert_eq!(s.branch_get("/repo", "main").unwrap(), Some((None, 20)));
        assert_eq!(s.branch_get("/other", "main").unwrap(), None);
    }
}
