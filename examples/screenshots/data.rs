//! 스크린샷용 가짜 데이터. 실제 Linear 데이터·사람 이름을 쓰지 않는다.
//! 이슈 제목은 사용자 데이터라서 모든 언어에서 같다.

use herdr_linear::linear::types::{Comment, Issue, IssueRelations, RelatedIssue, Viewer};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// 화면의 "지금": 2026-10-01 10:30 UTC.
pub const NOW: i64 = 1_790_850_600_000;

/// 상태 (이름, 종류, 색).
type State = (&'static str, &'static str, &'static str);
const IN_PROGRESS: State = ("In Progress", "started", "#f2c94c");
const IN_REVIEW: State = ("In Review", "started", "#5e6ad2");
const TODO: State = ("Todo", "unstarted", "#e2e2e2");
const BACKLOG: State = ("Backlog", "backlog", "#bec2c8");
const DONE: State = ("Done", "completed", "#5e6ad2");

fn parse<T: DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).expect("fake data matches the Linear types")
}

fn state((name, kind, color): State) -> Value {
    json!({
        "id": format!("st-{}", name.to_lowercase().replace(' ', "-")),
        "name": name, "type": kind, "color": color
    })
}

fn label(name: &str, color: &str) -> Value {
    json!({ "id": format!("lb-{name}"), "name": name, "color": color })
}

/// 이슈 JSON (Linear 응답 모양).
fn issue(
    identifier: &str,
    title: &str,
    st: State,
    priority: i64,
    labels: Vec<Value>,
    updated: &str,
) -> Value {
    let (key, num) = identifier.split_once('-').expect("KEY-123");
    let team = if key == "ENG" {
        "Engineering"
    } else {
        "Operations"
    };
    json!({
        "id": format!("id-{identifier}"),
        "identifier": identifier,
        "number": num.parse::<f64>().expect("issue number"),
        "title": title,
        "description": null,
        "priority": priority as f64,
        "estimate": null,
        "url": format!("https://linear.app/acme/issue/{identifier}"),
        "branchName": format!("alex/{}", identifier.to_lowercase()),
        "dueDate": null,
        "createdAt": "2026-09-20T09:00:00.000Z",
        "updatedAt": updated,
        "archivedAt": null,
        "trashed": null,
        "team": { "id": format!("team-{key}"), "key": key, "name": team },
        "state": state(st),
        "assignee": { "id": "u-alex", "name": "Alex Rivera", "displayName": "alex" },
        "project": null,
        "cycle": null,
        "parent": null,
        "labels": { "nodes": labels }
    })
}

pub fn viewer() -> Viewer {
    parse(json!({
        "id": "u-alex", "name": "Alex Rivera", "displayName": "alex", "email": "alex@example.com",
        "organization": { "id": "org-acme", "name": "Acme", "urlKey": "acme" },
        "teams": { "nodes": [
            { "id": "team-ENG", "key": "ENG", "name": "Engineering" },
            { "id": "team-OPS", "key": "OPS", "name": "Operations" }
        ] }
    }))
}

const DESCRIPTION: &str =
    "Users on Safari 17 get stuck in a redirect loop after signing in with SSO.

## Steps to reproduce
1. Sign in with Google SSO on Safari
2. Wait for the callback page
3. The page reloads forever

## Cause
The `session_id` cookie is dropped because it is set with `SameSite=None` but without `Secure`.

```js
res.cookie(\"session_id\", id, { sameSite: \"none\", secure: true });
```

See the [incident notes](https://example.com/incidents/42).";

/// 현재 브랜치에 연결된 이슈. 상세 화면도 이 이슈다.
pub fn pinned() -> Issue {
    let mut v = issue(
        "ENG-142",
        "Fix login redirect loop on Safari",
        IN_PROGRESS,
        2,
        vec![label("bug", "#eb5757"), label("auth", "#4ea7fc")],
        "2026-10-01T10:12:00.000Z",
    );
    v["description"] = json!(DESCRIPTION);
    // 프로젝트·사이클·상위 줄이 팔레트 미리 보기(64칸)에 모든 언어에서 다 들어가게 짧게 둔다
    v["project"] = json!({ "id": "pj-sso", "name": "SSO" });
    v["cycle"] = json!({ "id": "cy-14", "number": 14.0, "name": null });
    v["parent"] = json!({
        "id": "id-ENG-120", "identifier": "ENG-120", "title": "Single sign-on for all apps",
        "state": state(IN_PROGRESS)
    });
    v["attachments"] = json!({ "nodes": [ {
        "url": "https://github.com/acme/web/pull/482",
        "sourceType": "github",
        "createdAt": "2026-10-01T09:40:00.000Z",
        "metadata": { "status": "open", "number": 482.0, "repoName": "acme/web", "draft": false }
    } ] });
    parse(v)
}

/// "내 이슈" 탭.
pub fn mine() -> Vec<Issue> {
    vec![
        pinned(),
        parse(issue(
            "ENG-139",
            "Add rate limit headers to the public API",
            IN_REVIEW,
            3,
            vec![label("api", "#4ea7fc")],
            "2026-10-01T08:05:00.000Z",
        )),
        parse(issue(
            "ENG-131",
            "Session expires during long uploads",
            TODO,
            1,
            vec![label("bug", "#eb5757")],
            "2026-09-30T17:20:00.000Z",
        )),
        parse(issue(
            "OPS-58",
            "Rotate staging database credentials",
            TODO,
            2,
            vec![label("security", "#f2994a")],
            "2026-09-30T11:00:00.000Z",
        )),
        parse(issue(
            "ENG-120",
            "Single sign-on for all apps",
            IN_PROGRESS,
            2,
            vec![label("epic", "#bb87fc")],
            "2026-09-29T15:45:00.000Z",
        )),
        parse(issue(
            "ENG-127",
            "Dark mode for the settings page",
            BACKLOG,
            4,
            vec![label("design", "#bb87fc")],
            "2026-09-28T10:00:00.000Z",
        )),
        parse(issue(
            "ENG-118",
            "Flaky checkout test on CI",
            TODO,
            0,
            vec![label("ci", "#95a2b3")],
            "2026-09-27T13:30:00.000Z",
        )),
        parse(issue(
            "ENG-104",
            "Document the webhook retry policy",
            BACKLOG,
            3,
            vec![label("docs", "#26b5ce")],
            "2026-09-25T09:15:00.000Z",
        )),
    ]
}

pub fn comments() -> Vec<Comment> {
    parse(json!([
        { "id": "c1", "body": "Reproduced on Safari 17.4. Chrome and Firefox are fine.",
          "createdAt": "2026-10-01T09:12:00.000Z", "editedAt": null,
          "user": { "id": "u-sam", "name": "Sam Lee", "displayName": "sam" } },
        { "id": "c2", "body": "Fix is up in the PR. Adding a regression test for the callback now.",
          "createdAt": "2026-10-01T10:05:00.000Z", "editedAt": null,
          "user": { "id": "u-alex", "name": "Alex Rivera", "displayName": "alex" } }
    ]))
}

fn related(identifier: &str, title: &str, st: State) -> RelatedIssue {
    parse(
        json!({ "id": format!("id-{identifier}"), "identifier": identifier, "title": title, "state": state(st) }),
    )
}

pub fn relations() -> IssueRelations {
    IssueRelations {
        children: vec![
            related("ENG-143", "Add Safari to the end-to-end test matrix", DONE),
            related("ENG-144", "Set Secure on all auth cookies", IN_PROGRESS),
            related("ENG-145", "Regression test for the SSO callback", TODO),
        ],
        more_children: false,
        blocked_by: vec![related(
            "OPS-58",
            "Rotate staging database credentials",
            TODO,
        )],
        blocking: vec![related(
            "ENG-131",
            "Session expires during long uploads",
            TODO,
        )],
        related: vec![related("ENG-97", "Cookie consent banner", DONE)],
    }
}
