# 관계 API 실측 (v0.1.2)

- 날짜: 2026-10-07
- 방법: 읽기 전용 GraphQL 쿼리. 키는 0600 임시 헤더 파일로 넘기고 바로 지웠다. 이 문서에는 이슈 제목·사람 이름·워크스페이스 정보를 남기지 않는다.

| 확인 항목 (관계 스펙 6장) | 결과 | 계획에 주는 영향 |
|---|---|---|
| `blocks` 방향: 막힌 이슈의 `inverseRelations` | errors: 0, checked: 5, all_ok: true | all_ok면 스펙대로 |
| `blocks` 방향: 막는 이슈의 `relations` | errors: 0, checked: 6, all_ok: true | all_ok면 스펙대로 |
| 새 상세 쿼리 복잡도 | 408 (errors: 0, children: 0, relation_nodes: 1) | 2,000 미만이면 `RELATION_PAGE_SIZE` 50 유지 |
| 목록 50건 복잡도 (상위 상태 포함) | 21 (1부 16, +5) (errors: 0, nodes: 50) | 크게 늘지 않으면 `IssueFields`에 상위 상태 유지 |
| 보관된 하위 이슈 | 스키마 문서: `children`의 `includeArchived` 기본값 false | 그대로 둔다 |
| 볼 수 없는 팀과의 관계 | 이 키로는 재현할 수 없음 | 설계대로 두고, 읽을 수 없는 노드는 하나씩 버린다 |
