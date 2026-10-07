# 2부 herdr 수동 확인 결과

날짜: 2026-10-07 · herdr 0.9.3 · macOS · 브랜치 `feat/part2-palette` (최종 리뷰 수정 0dbbfd7 반영 빌드)

## 연결

- `herdr plugin link`: 성공. herdr가 매니페스트를 받아들였다. 확인한 항목은 다음과 같다.
  - palette 액션의 `selection` 컨텍스트
  - popup pane 80%×80%
  - linear.app 이슈 링크 핸들러
- 단축키: `~/.config/herdr/config.toml`에 `prefix+i`, `ctrl+alt+i`를 더했다(`herdr-linear.palette`).
  - `herdr server reload-config` 결과 진단이 없었다.
  - 원래 설정은 `config.toml.bak-herdr-linear-20261007`로 백업했다.

## 사용자 확인

사용자가 핵심 흐름 네 가지를 직접 해 보고 "잘 된다"고 확인했다.

| 체크리스트 # | 항목 | 결과 |
|---|---|---|
| 1 | 단축키로 popup 열기, 목록의 Linear 색 | 통과 |
| 3 | 한글 검색 | 통과 |
| 5 | Enter로 상세 열기, Esc로 뒤로 | 통과 |
| 6 | 브라우저 열기(`o`), 복사(`y`) | 통과 |

따로 확인하지 않은 항목은 아래와 같다. 쓰면서 문제가 보이면 고친다.
- 2: 키 없을 때 첫 실행
- 4: 깊은 검색
- 7: 최근 본·전체 탭
- 8: 현재 브랜치 고정
- 9: 선택 텍스트로 열기
- 10: 링크 Ctrl+클릭
- 11: 다른 창이 떠 있을 때 알림
- 12: 오프라인
- 13: 한글 자모 키

## 사용자 요청 (다음 작업)

- 복사 기본값을 티켓 URL로 한다. 가장 많이 쓸 동작이라 첫째에 둔다.
- 열린 PR이 있으면 PR 링크를 복사할 수 있게 한다.
- 팔레트를 popup이 아니라 pane에 띄운다(스펙의 사이드 패널).
