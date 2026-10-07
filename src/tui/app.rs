//! 팔레트의 상태와 동작. 화면·네트워크와 떨어진 순수 로직이라 테스트로 고정한다.
//!
//! 런타임은 키를 [`Input`]으로 바꿔 [`App::handle`]에 넘기고, 네트워크·캐시 결과를
//! [`Msg`]로 [`App::apply`]에 넘긴다. 앱은 해야 할 일을 [`Effect`]로 돌려준다.

use crate::linear::client::ApiError;
use crate::linear::types::{Comment, Issue, IssueRelations, Viewer};
use crate::markdown::{self, Theme};
use crate::search::query::parse;
use crate::search::rank::{SearchIndex, merge};
use crate::ui::relations::{self, RelRow};

/// 서버 검색을 보내기 전에 입력이 멈춰야 하는 시간.
pub const SEARCH_DEBOUNCE_MS: i64 = 300;
/// 하단 안내 문구를 보여주는 시간.
pub const FLASH_MS: i64 = 3_000;
/// 깊은 검색이 분당 한도에 걸렸을 때 안내.
pub const DEEP_LIMIT_TEXT: &str = "깊은 검색은 분당 30회까지예요. 잠시 뒤 다시 시도하세요";
/// 설정·범위 경고를 보여주는 시간. 일반 안내보다 길다.
pub const WARN_MS: i64 = 10_000;
/// 검색 결과로 보여줄 최대 줄 수.
const MAX_RESULTS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Mine,
    Recent,
    All,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Mine, Tab::Recent, Tab::All];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Mine => "내 이슈",
            Tab::Recent => "최근 본",
            Tab::All => "전체",
        }
    }

    fn index(self) -> usize {
        match self {
            Tab::Mine => 0,
            Tab::Recent => 1,
            Tab::All => 2,
        }
    }

    fn next(self) -> Tab {
        Tab::ALL[(self.index() + 1) % 3]
    }

    fn prev(self) -> Tab {
        Tab::ALL[(self.index() + 2) % 3]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// API 키 입력 화면
    Onboarding,
    /// 글자가 검색어로 들어가는 모드 (팔레트 기본)
    Search,
    /// 한 글자 키가 동작인 모드
    List,
    Detail,
}

/// 사용자 동작. 키(목록·상세 모드)와 Ctrl+K 메뉴에서 같은 것을 쓴다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Open,
    Browser,
    CopyId,
    CopyUrl,
    /// 열린 PR 링크
    CopyPr,
    Refresh,
    DeepSearch,
    Links,
    Back,
    Quit,
    OpenUrl(String),
    /// 상세의 관계 메뉴
    Relations,
    /// 관계 이슈를 연다. 지금 상세는 쌓아 두고 Esc로 돌아온다
    OpenRelated(RelRow),
}

/// 키 입력을 해석한 결과 (`tui::keys`가 만든다).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Char(char),
    Paste(String),
    Backspace,
    ClearLine,
    Up,
    Down,
    PageUp,
    PageDown,
    Top,
    Bottom,
    Enter,
    Esc,
    NextTab,
    PrevTab,
    Menu,
    Search,
    Act(Act),
    Quit,
    /// 마우스 휠
    ScrollUp,
    ScrollDown,
    /// 마우스로 목록의 n번째 줄(`rows` 기준)을 눌렀다. 선택된 줄이면 연다
    ClickRow(usize),
    ClickTab(Tab),
    /// 마우스로 메뉴에 보이는 n번째 항목을 눌렀다
    ClickMenu(usize),
    /// 상세 화면의 관계 줄(`ui::relations::rows` 번호)을 눌렀다
    ClickRelation(usize),
}

/// 앱이 런타임에 요청하는 일.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// 캐시된 viewer·검색 색인을 보내고 viewer를 새로 받는다
    Init,
    /// 탭 내용: 캐시를 먼저 보내고 서버에서 다시 받는다
    LoadTab(Tab),
    /// "전체" 탭 다음 페이지
    LoadMore,
    Search {
        seq: u64,
        query: String,
    },
    DeepSearch {
        seq: u64,
        query: String,
    },
    /// 상세: 캐시를 먼저 보내고 서버에서 다시 받는다. 최근 본에 기록한다
    OpenDetail(String),
    /// 원래 pane의 브랜치에 연결된 이슈를 찾는다
    ResolvePinned,
    OpenUrl(String),
    /// `what`은 "복사됨: …"에 보일 이름
    Copy {
        text: String,
        what: String,
    },
    ValidateKey(String),
}

/// 런타임이 앱에 알려주는 결과.
#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    Viewer(Viewer),
    Index(Vec<Issue>),
    /// `fresh`가 false면 캐시에서 온 것
    Tab {
        tab: Tab,
        issues: Vec<Issue>,
        fresh: bool,
        has_more: bool,
        append: bool,
    },
    Search {
        seq: u64,
        issues: Vec<Issue>,
    },
    Detail {
        id: String,
        issue: Issue,
        comments: Vec<Comment>,
        more: bool,
        /// 관계. 캐시에 없으면 `None`
        relations: Option<IssueRelations>,
        fresh: bool,
    },
    /// 보관·삭제됐거나 없는 이슈. `OpenDetail`에 넘긴 값(id 또는 식별자)이 그대로 온다
    DetailGone(String),
    Pinned(Option<Issue>),
    KeyOk(Viewer),
    KeyBad(String),
    /// 인증 실패. `env`면 `LINEAR_API_KEY` 환경 변수의 키다
    AuthFailed {
        env: bool,
    },
    /// 세어 둔 요청 하나가 실패했다
    Failed(ApiError),
    /// 남은 요청이 적다. 이 시각(리셋)까지 자동 서버 검색을 멈춘다
    Throttled(i64),
    /// 깊은 검색이 분당 한도에 걸렸다. 세어 둔 요청 하나를 끝내고 안내만 한다
    DeepLimited,
    Flash(String),
    /// 설정·범위 경고. 일반 안내보다 오래 보이고 그것에 덮이지 않는다
    Warn(String),
}

/// 상단에 보이는 문제 상태. 한도 초과는 하단에 잠깐 알리기만 해서 여기 없다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    Offline(String),
    Error(String),
}

/// 목록에서 고를 수 있는 줄.
#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    /// 현재 브랜치에 연결된 이슈
    Pinned(Issue),
    Issue(Issue),
    /// "서버에서 검색 (코멘트 포함)"
    DeepSearch,
}

impl Row {
    pub fn issue(&self) -> Option<&Issue> {
        match self {
            Row::Pinned(i) | Row::Issue(i) => Some(i),
            Row::DeepSearch => None,
        }
    }

    fn key(&self) -> String {
        self.issue()
            .map_or_else(|| "deep".to_string(), |i| i.id.clone())
    }
}

/// 상세 화면.
#[derive(Debug, Clone, PartialEq)]
pub struct Detail {
    /// 이슈 id 또는 식별자 (시작할 때는 식별자만 알 수 있다)
    pub id: String,
    pub issue: Option<Issue>,
    pub comments: Vec<Comment>,
    pub more_comments: bool,
    /// 관계. 아직 모르면 `None`
    pub relations: Option<IssueRelations>,
    pub scroll: u16,
    pub max_scroll: u16,
    pub loading: bool,
    pub gone: bool,
    back: Mode,
}

/// Ctrl+K 메뉴 또는 링크 목록.
#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    pub title: &'static str,
    pub items: Vec<(String, Act)>,
    pub filter: String,
    pub selected: usize,
}

impl Menu {
    /// 거르기 글자가 들어간 항목만.
    pub fn visible(&self) -> Vec<&(String, Act)> {
        let f = self.filter.to_lowercase();
        self.items
            .iter()
            .filter(|(label, _)| label.to_lowercase().contains(&f))
            .collect()
    }
}

#[derive(Debug, Clone, Default)]
struct TabData {
    issues: Vec<Issue>,
    loaded: bool,
    has_more: bool,
    loading_more: bool,
}

pub struct App {
    pub mode: Mode,
    pub tab: Tab,
    pub query: String,
    pub rows: Vec<Row>,
    pub selected: usize,
    /// 목록의 스크롤 위치. 런타임이 그린 결과(`Drawn::list_offset`)를 넣어 준다
    pub list_offset: usize,
    pub detail: Option<Detail>,
    /// 관계 이슈를 열 때 쌓아 둔 상세. Esc로 하나씩 돌아간다
    detail_stack: Vec<Detail>,
    pub menu: Option<Menu>,
    pub key_input: String,
    pub key_error: Option<String>,
    pub key_checking: bool,
    pub env_key_invalid: bool,
    /// 응답을 기다리는 요청 수
    pub loading: usize,
    pub updated_at: Option<i64>,
    pub problem: Option<Problem>,
    /// (문구, 보여줄 마지막 시각)
    pub flash: Option<(String, i64)>,
    /// (경고 문구, 보여줄 마지막 시각)
    pub warning: Option<(String, i64)>,
    pub quit: bool,
    pub viewer: Option<Viewer>,
    index: SearchIndex,
    tabs: [TabData; 3],
    pinned: Option<Issue>,
    /// 지금 검색어에 대한 서버 결과 (없으면 로컬 결과만)
    results: Option<Vec<Issue>>,
    search_seq: u64,
    search_due: Option<i64>,
    /// 한도 초과로 자동 요청을 멈출 시각
    paused_until: Option<i64>,
}

impl App {
    fn blank(mode: Mode) -> App {
        App {
            mode,
            tab: Tab::Mine,
            query: String::new(),
            rows: Vec::new(),
            selected: 0,
            list_offset: 0,
            detail: None,
            detail_stack: Vec::new(),
            menu: None,
            key_input: String::new(),
            key_error: None,
            key_checking: false,
            env_key_invalid: false,
            loading: 0,
            updated_at: None,
            problem: None,
            flash: None,
            warning: None,
            quit: false,
            viewer: None,
            index: SearchIndex::new(Vec::new()),
            tabs: Default::default(),
            pinned: None,
            results: None,
            search_seq: 0,
            search_due: None,
            paused_until: None,
        }
    }

    /// 키가 있을 때 시작한다. `open`(식별자)이 있으면 그 이슈 상세로 시작한다.
    pub fn start(open: Option<String>) -> (App, Vec<Effect>) {
        let mut app = App::blank(Mode::Search);
        let mut effects = app.init_effects();
        if let Some(id) = open {
            app.detail = Some(Detail {
                id: id.clone(),
                issue: None,
                comments: Vec::new(),
                more_comments: false,
                relations: None,
                scroll: 0,
                max_scroll: 0,
                loading: true,
                gone: false,
                back: Mode::Search,
            });
            app.mode = Mode::Detail;
            app.loading += 1;
            effects.push(Effect::OpenDetail(id));
        }
        (app, effects)
    }

    /// 키 입력 화면으로 시작한다. `env_invalid`면 환경 변수 키가 틀렸다는 안내만 보인다.
    pub fn onboarding(env_invalid: bool) -> App {
        let mut app = App::blank(Mode::Onboarding);
        app.env_key_invalid = env_invalid;
        app
    }

    fn init_effects(&mut self) -> Vec<Effect> {
        self.loading += 1;
        self.tabs[Tab::Mine.index()].loaded = true;
        vec![
            Effect::Init,
            Effect::LoadTab(Tab::Mine),
            Effect::ResolvePinned,
        ]
    }

    pub fn selected_issue(&self) -> Option<&Issue> {
        self.rows.get(self.selected).and_then(Row::issue)
    }

    /// 상세 화면에 보이는 이슈 (없으면 목록에서 고른 이슈).
    fn current_issue(&self) -> Option<&Issue> {
        match (self.mode, &self.detail) {
            (Mode::Detail, Some(d)) => d.issue.as_ref(),
            _ => self.selected_issue(),
        }
    }

    /// 내 팀 키 (식별자 강조용).
    pub fn team_keys(&self) -> Vec<String> {
        self.viewer
            .as_ref()
            .map(|v| v.teams.nodes.iter().map(|t| t.key.clone()).collect())
            .unwrap_or_default()
    }

    pub fn flash_text(&self, now: i64) -> Option<&str> {
        self.flash
            .as_ref()
            .filter(|(_, until)| now < *until)
            .map(|(t, _)| t.as_str())
    }

    pub fn warning_text(&self, now: i64) -> Option<&str> {
        self.warning
            .as_ref()
            .filter(|(_, until)| now < *until)
            .map(|(t, _)| t.as_str())
    }

    /// 그린 뒤 상세 화면의 최대 스크롤을 알려준다.
    pub fn set_detail_max_scroll(&mut self, max: u16) {
        if let Some(d) = self.detail.as_mut() {
            d.max_scroll = max;
            d.scroll = d.scroll.min(max);
        }
    }

    fn set_flash(&mut self, text: impl Into<String>, now: i64) {
        self.flash = Some((text.into(), now + FLASH_MS));
    }

    fn done_loading(&mut self) {
        self.loading = self.loading.saturating_sub(1);
    }

    /// `until`까지 자동 서버 검색을 멈춘다. 이미 더 늦게까지 멈춰 있으면 그대로 둔다.
    fn pause_until(&mut self, until: i64) {
        self.paused_until = Some(self.paused_until.map_or(until, |p| p.max(until)));
    }

    fn succeeded(&mut self, now: i64) {
        self.problem = None;
        self.updated_at = Some(now);
    }

    /// 목록 줄을 다시 만든다. `keep`이면 고른 이슈를 id로 유지한다.
    fn rebuild(&mut self, keep: bool) {
        let keep_key = keep
            .then(|| self.rows.get(self.selected).map(Row::key))
            .flatten();
        let q = parse(&self.query);
        let mut rows = Vec::new();
        if q.is_empty() {
            if let Some(p) = &self.pinned {
                rows.push(Row::Pinned(p.clone()));
            }
            let pinned_id = self.pinned.as_ref().map(|p| p.id.as_str());
            rows.extend(
                self.tabs[self.tab.index()]
                    .issues
                    .iter()
                    .filter(|i| Some(i.id.as_str()) != pinned_id)
                    .cloned()
                    .map(Row::Issue),
            );
        } else {
            let vid = self.viewer.as_ref().map(|v| v.id.as_str());
            let local = self.index.search(&q, vid);
            let list = match &self.results {
                Some(server) => merge(&local, server, &q, vid),
                None => local,
            };
            rows.extend(list.into_iter().take(MAX_RESULTS).map(Row::Issue));
            if q.has_text() {
                rows.push(Row::DeepSearch);
            }
        }
        self.rows = rows;
        let last = self.rows.len().saturating_sub(1);
        self.selected = keep_key
            .and_then(|k| self.rows.iter().position(|r| r.key() == k))
            .unwrap_or(if keep { self.selected.min(last) } else { 0 });
    }

    fn query_changed(&mut self, now: i64) {
        // 이전 검색어로 보낸 요청의 응답은 이제 버린다 (새 요청이 아직 안 나갔어도)
        self.search_seq += 1;
        self.results = None;
        self.rebuild(false);
        self.search_due = (!parse(&self.query).is_empty()).then_some(now + SEARCH_DEBOUNCE_MS);
    }

    /// 키 입력을 처리한다.
    pub fn handle(&mut self, input: Input, now: i64) -> Vec<Effect> {
        if input == Input::Quit {
            self.quit = true;
            return Vec::new();
        }
        if self.menu.is_some() {
            return self.handle_menu(input, now);
        }
        match self.mode {
            Mode::Onboarding => self.handle_onboarding(input),
            Mode::Search => self.handle_search(input, now),
            Mode::List => self.handle_list(input, now),
            Mode::Detail => self.handle_detail(input, now),
        }
    }

    fn handle_onboarding(&mut self, input: Input) -> Vec<Effect> {
        if self.env_key_invalid {
            if matches!(input, Input::Esc | Input::Enter) {
                self.quit = true;
            }
            return Vec::new();
        }
        match input {
            Input::Char(c) if !c.is_whitespace() => self.key_input.push(c),
            Input::Paste(s) => self
                .key_input
                .extend(s.chars().filter(|c| !c.is_whitespace())),
            Input::Backspace => {
                self.key_input.pop();
            }
            Input::ClearLine => self.key_input.clear(),
            Input::Enter if !self.key_input.is_empty() && !self.key_checking => {
                self.key_checking = true;
                self.key_error = None;
                return vec![Effect::ValidateKey(self.key_input.clone())];
            }
            Input::Esc => self.quit = true,
            _ => {}
        }
        Vec::new()
    }

    fn handle_search(&mut self, input: Input, now: i64) -> Vec<Effect> {
        match input {
            Input::Char(c) => {
                self.query.push(c);
                self.query_changed(now);
            }
            Input::Paste(s) => {
                self.query.push_str(&s.replace(['\n', '\r', '\t'], " "));
                self.query_changed(now);
            }
            Input::Backspace => {
                if self.query.pop().is_some() {
                    self.query_changed(now);
                }
            }
            Input::ClearLine => {
                self.query.clear();
                self.query_changed(now);
            }
            Input::Esc => self.mode = Mode::List,
            other => return self.handle_common(other, now),
        }
        Vec::new()
    }

    fn handle_list(&mut self, input: Input, now: i64) -> Vec<Effect> {
        match input {
            Input::Search => {
                self.mode = Mode::Search;
                Vec::new()
            }
            Input::Esc => {
                self.quit = true;
                Vec::new()
            }
            Input::Act(a) => self.act(a, now),
            other => self.handle_common(other, now),
        }
    }

    /// 검색·목록 모드에 공통인 이동·탭·메뉴.
    fn handle_common(&mut self, input: Input, now: i64) -> Vec<Effect> {
        let last = self.rows.len().saturating_sub(1);
        match input {
            Input::Up => self.selected = self.selected.saturating_sub(1),
            Input::Down => self.selected = (self.selected + 1).min(last),
            Input::PageUp => self.selected = self.selected.saturating_sub(10),
            Input::PageDown => self.selected = (self.selected + 10).min(last),
            Input::Top => self.selected = 0,
            Input::Bottom => self.selected = last,
            Input::Enter => return self.act(Act::Open, now),
            Input::ScrollUp => self.selected = self.selected.saturating_sub(1),
            Input::ScrollDown => self.selected = (self.selected + 1).min(last),
            Input::ClickRow(i) if i < self.rows.len() => {
                if i == self.selected {
                    return self.act(Act::Open, now);
                }
                self.selected = i;
            }
            Input::ClickTab(tab) if tab != self.tab => return self.switch_tab(tab, now),
            Input::NextTab | Input::PrevTab => {
                let tab = if input == Input::NextTab {
                    self.tab.next()
                } else {
                    self.tab.prev()
                };
                return self.switch_tab(tab, now);
            }
            Input::Menu => self.open_menu(),
            _ => {}
        }
        self.maybe_load_more()
    }

    fn handle_detail(&mut self, input: Input, now: i64) -> Vec<Effect> {
        let Some(d) = self.detail.as_mut() else {
            self.mode = Mode::List;
            return Vec::new();
        };
        match input {
            Input::Up => d.scroll = d.scroll.saturating_sub(1),
            Input::Down => d.scroll = d.scroll.saturating_add(1).min(d.max_scroll),
            Input::PageUp => d.scroll = d.scroll.saturating_sub(10),
            Input::PageDown => d.scroll = d.scroll.saturating_add(10).min(d.max_scroll),
            Input::Top => d.scroll = 0,
            Input::Bottom => d.scroll = d.max_scroll,
            Input::ScrollUp => d.scroll = d.scroll.saturating_sub(3),
            Input::ScrollDown => d.scroll = d.scroll.saturating_add(3).min(d.max_scroll),
            Input::Esc => return self.act(Act::Back, now),
            Input::Act(a) => return self.act(a, now),
            Input::Menu => self.open_menu(),
            Input::ClickRelation(i) => {
                if let Some(row) = self.detail_rows().get(i).cloned() {
                    return self.act(Act::OpenRelated(row), now);
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn handle_menu(&mut self, input: Input, now: i64) -> Vec<Effect> {
        let Some(menu) = self.menu.as_mut() else {
            return Vec::new();
        };
        let count = menu.visible().len();
        let input = match input {
            Input::ScrollUp => Input::Up,
            Input::ScrollDown => Input::Down,
            other => other,
        };
        match input {
            Input::ClickMenu(i) if i < count => {
                let act = menu.visible().get(i).map(|(_, a)| a.clone());
                self.menu = None;
                if let Some(a) = act {
                    return self.act(a, now);
                }
            }
            Input::Char(c) => {
                menu.filter.push(c);
                menu.selected = 0;
            }
            Input::Backspace => {
                menu.filter.pop();
                menu.selected = 0;
            }
            Input::Up => menu.selected = menu.selected.saturating_sub(1),
            Input::Down => menu.selected = (menu.selected + 1).min(count.saturating_sub(1)),
            Input::Esc | Input::Menu => self.menu = None,
            Input::Enter => {
                let act = menu.visible().get(menu.selected).map(|(_, a)| a.clone());
                self.menu = None;
                if let Some(a) = act {
                    return self.act(a, now);
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn open_menu(&mut self) {
        let mut items: Vec<(String, Act)> = Vec::new();
        let has_issue = self.current_issue().is_some();
        // 가장 많이 쓰는 티켓 URL 복사를 맨 위에 둔다
        if let Some(issue) = self.current_issue() {
            let pr = issue.open_pr();
            items.push(("URL 복사".into(), Act::CopyUrl));
            if let Some(pr) = pr {
                items.push((format!("PR 링크 복사 ({})", pr.label()), Act::CopyPr));
            }
            items.push(("ID 복사".into(), Act::CopyId));
        }
        if self.mode != Mode::Detail && has_issue {
            items.push(("상세 보기".into(), Act::Open));
        }
        if has_issue {
            items.push(("브라우저에서 열기".into(), Act::Browser));
        }
        if self.mode == Mode::Detail {
            items.push(("링크·이미지 목록".into(), Act::Links));
            items.push(("관계 이슈".into(), Act::Relations));
        }
        if self.mode != Mode::Detail && parse(&self.query).has_text() {
            items.push(("서버에서 깊은 검색 (코멘트 포함)".into(), Act::DeepSearch));
        }
        items.push(("새로고침".into(), Act::Refresh));
        if self.mode == Mode::Detail {
            items.push(("뒤로".into(), Act::Back));
        }
        items.push(("닫기".into(), Act::Quit));
        self.menu = Some(Menu {
            title: "동작",
            items,
            filter: String::new(),
            selected: 0,
        });
    }

    /// 상세 화면 본문과 코멘트의 링크·이미지 목록. 번호는 상세 화면에 보이는 번호와 같다.
    fn open_links(&mut self) {
        let Some(d) = self.detail.as_ref() else {
            return;
        };
        // 상세 화면처럼 공백뿐인 본문은 빼고, 본문 → 코멘트 순서로 번호를 이어 매긴다
        let body = d
            .issue
            .as_ref()
            .and_then(|i| i.description.as_deref())
            .map(str::trim)
            .filter(|b| !b.is_empty());
        let theme = Theme::default();
        let mut items = Vec::new();
        for md in body
            .into_iter()
            .chain(d.comments.iter().map(|c| c.body.as_str()))
        {
            for l in markdown::render_numbered(md, 80, &theme, &[], items.len() + 1).links {
                items.push((
                    format!("[{}] {} — {}", l.index, l.label, l.url),
                    Act::OpenUrl(l.url),
                ));
            }
        }
        self.menu = Some(Menu {
            title: if items.is_empty() {
                "링크 없음"
            } else {
                "링크·이미지"
            },
            items,
            filter: String::new(),
            selected: 0,
        });
    }

    /// 지금 상세의 관계 줄. 화면의 관계 칸과 같은 순서라 메뉴·클릭 번호가 화면과 맞는다.
    fn detail_rows(&self) -> Vec<RelRow> {
        self.detail.as_ref().map_or_else(Vec::new, |d| {
            relations::rows(
                d.issue.as_ref().and_then(|i| i.parent.as_ref()),
                d.relations.as_ref(),
            )
        })
    }

    /// 관계 메뉴. 제목으로 관계를 아직 모르는지, 받지 못했는지, 없는지 알린다.
    fn open_relations(&mut self) {
        let Some(d) = self.detail.as_ref() else {
            return;
        };
        let (known, loading) = (d.relations.is_some(), d.loading);
        let rows = self.detail_rows();
        let title = match (known, loading) {
            (false, true) => "관계 불러오는 중…",
            (false, false) => "관계를 불러오지 못했어요",
            (true, _) if rows.is_empty() => "관계 없음",
            (true, _) => "관계",
        };
        self.menu = Some(Menu {
            title,
            items: rows
                .into_iter()
                .map(|r| (r.filter_text(), Act::OpenRelated(r)))
                .collect(),
            filter: String::new(),
            selected: 0,
        });
    }

    /// 관계 이슈를 연다. 지금 상세는 스크롤 위치째 쌓아 둔다.
    fn open_related(&mut self, row: RelRow) -> Vec<Effect> {
        let Some(current) = self.detail.take() else {
            return Vec::new();
        };
        let back = current.back;
        self.detail_stack.push(current);
        self.detail = Some(Detail {
            id: row.id.clone(),
            issue: None,
            comments: Vec::new(),
            more_comments: false,
            relations: None,
            scroll: 0,
            max_scroll: 0,
            loading: true,
            gone: false,
            back,
        });
        self.mode = Mode::Detail;
        self.loading += 1;
        vec![Effect::OpenDetail(row.id)]
    }

    /// 지금 상세와 쌓아 둔 상세.
    fn details_mut(&mut self) -> impl Iterator<Item = &mut Detail> {
        self.detail.iter_mut().chain(self.detail_stack.iter_mut())
    }

    fn switch_tab(&mut self, tab: Tab, now: i64) -> Vec<Effect> {
        self.tab = tab;
        if !self.query.is_empty() {
            self.query.clear();
            self.query_changed(now);
        }
        self.rebuild(false);
        let data = &mut self.tabs[tab.index()];
        // 최근 본은 로컬 기록이라 열 때마다 다시 읽는다
        if data.loaded && tab != Tab::Recent {
            return Vec::new();
        }
        data.loaded = true;
        self.loading += 1;
        vec![Effect::LoadTab(tab)]
    }

    /// "전체" 탭 끝에 닿으면 다음 페이지를 부른다.
    fn maybe_load_more(&mut self) -> Vec<Effect> {
        let at_end = self.selected + 1 >= self.rows.len();
        let data = &mut self.tabs[Tab::All.index()];
        if self.tab == Tab::All
            && self.query.is_empty()
            && at_end
            && data.has_more
            && !data.loading_more
        {
            data.loading_more = true;
            self.loading += 1;
            return vec![Effect::LoadMore];
        }
        Vec::new()
    }

    fn act(&mut self, act: Act, now: i64) -> Vec<Effect> {
        match act {
            Act::Open => match self.rows.get(self.selected).cloned() {
                Some(Row::DeepSearch) => self.act(Act::DeepSearch, now),
                Some(row) => {
                    let Some(issue) = row.issue().cloned() else {
                        return Vec::new();
                    };
                    let back = self.mode;
                    self.detail = Some(Detail {
                        id: issue.id.clone(),
                        issue: Some(issue.clone()),
                        comments: Vec::new(),
                        more_comments: false,
                        relations: None,
                        scroll: 0,
                        max_scroll: 0,
                        loading: true,
                        gone: false,
                        back,
                    });
                    self.mode = Mode::Detail;
                    self.loading += 1;
                    vec![Effect::OpenDetail(issue.id)]
                }
                None => Vec::new(),
            },
            Act::Browser => self
                .current_issue()
                .map(|i| vec![Effect::OpenUrl(i.url.clone())])
                .unwrap_or_default(),
            Act::CopyId => self
                .current_issue()
                .map(|i| {
                    vec![Effect::Copy {
                        text: i.identifier.clone(),
                        what: i.identifier.clone(),
                    }]
                })
                .unwrap_or_default(),
            Act::CopyUrl => self
                .current_issue()
                .map(|i| {
                    vec![Effect::Copy {
                        text: i.url.clone(),
                        what: format!("{} URL", i.identifier),
                    }]
                })
                .unwrap_or_default(),
            Act::CopyPr => {
                let Some(issue) = self.current_issue() else {
                    return Vec::new();
                };
                match issue.open_pr() {
                    Some(pr) => vec![Effect::Copy {
                        text: pr.url.clone(),
                        what: pr.label(),
                    }],
                    None => {
                        let text = format!("{}에 열린 PR이 없어요", issue.identifier);
                        self.set_flash(text, now);
                        Vec::new()
                    }
                }
            }
            Act::Refresh => {
                if self.mode == Mode::Detail {
                    if let Some(d) = self.detail.as_mut() {
                        d.loading = true;
                        self.loading += 1;
                        return vec![Effect::OpenDetail(d.id.clone())];
                    }
                    return Vec::new();
                }
                if !parse(&self.query).is_empty() {
                    self.search_due = Some(now);
                    self.paused_until = None;
                    return Vec::new();
                }
                self.loading += 1;
                vec![Effect::LoadTab(self.tab), Effect::ResolvePinned]
            }
            Act::DeepSearch => {
                if !parse(&self.query).has_text() {
                    return Vec::new();
                }
                self.search_seq += 1;
                self.search_due = None;
                self.loading += 1;
                vec![Effect::DeepSearch {
                    seq: self.search_seq,
                    query: self.query.clone(),
                }]
            }
            Act::Links => {
                self.open_links();
                Vec::new()
            }
            Act::Relations => {
                self.open_relations();
                Vec::new()
            }
            Act::OpenRelated(row) => self.open_related(row),
            Act::Back => {
                // 관계 이슈에서 왔으면 앞 상세로 돌아간다 (스크롤 위치째)
                if let Some(prev) = self.detail_stack.pop() {
                    self.detail = Some(prev);
                    self.mode = Mode::Detail;
                    return Vec::new();
                }
                let back = self.detail.take().map_or(Mode::List, |d| d.back);
                self.mode = back;
                Vec::new()
            }
            Act::Quit => {
                self.quit = true;
                Vec::new()
            }
            Act::OpenUrl(url) => vec![Effect::OpenUrl(url)],
        }
    }

    /// 시간이 지나 할 일 (검색 디바운스).
    pub fn tick(&mut self, now: i64) -> Vec<Effect> {
        let Some(due) = self.search_due else {
            return Vec::new();
        };
        if now < due {
            return Vec::new();
        }
        if let Some(until) = self.paused_until {
            if now < until {
                return Vec::new();
            }
            self.paused_until = None;
        }
        self.search_due = None;
        self.search_seq += 1;
        self.loading += 1;
        vec![Effect::Search {
            seq: self.search_seq,
            query: self.query.clone(),
        }]
    }

    /// 런타임이 보낸 결과를 반영한다.
    pub fn apply(&mut self, msg: Msg, now: i64) -> Vec<Effect> {
        match msg {
            Msg::Viewer(v) => self.viewer = Some(v),
            Msg::Index(issues) => {
                self.index = SearchIndex::new(issues);
                if !self.query.is_empty() {
                    self.rebuild(true);
                }
            }
            Msg::Tab {
                tab,
                issues,
                fresh,
                has_more,
                append,
            } => {
                self.index.upsert(&issues);
                let data = &mut self.tabs[tab.index()];
                if append {
                    data.issues.extend(issues);
                    data.loading_more = false;
                } else {
                    data.issues = issues;
                }
                data.has_more = has_more;
                if fresh {
                    self.done_loading();
                    // 최근 본은 로컬 기록이라 서버 상태(갱신 시각·오프라인)를 바꾸지 않는다
                    if tab != Tab::Recent {
                        self.succeeded(now);
                    }
                }
                if tab == self.tab {
                    self.rebuild(true);
                }
            }
            Msg::Search { seq, issues } => {
                self.done_loading();
                if seq != self.search_seq {
                    return Vec::new();
                }
                self.succeeded(now);
                self.index.upsert(&issues);
                self.results = Some(issues);
                self.rebuild(true);
            }
            Msg::Detail {
                id,
                issue,
                comments,
                more,
                relations,
                fresh,
            } => {
                if fresh {
                    self.done_loading();
                    self.succeeded(now);
                    self.index.upsert(std::slice::from_ref(&issue));
                }
                // 쌓아 둔 상세도 같은 이슈면 바꾼다 (응답 전에 관계 이슈로 넘어간 경우)
                for d in self.details_mut() {
                    if d.id == id
                        || d.id == issue.id
                        || d.id.eq_ignore_ascii_case(&issue.identifier)
                    {
                        d.id = issue.id.clone();
                        d.issue = Some(issue.clone());
                        d.comments = comments.clone();
                        d.more_comments = more;
                        if let Some(r) = &relations {
                            d.relations = Some(r.clone());
                        }
                        if fresh {
                            d.loading = false;
                        }
                    }
                }
            }
            Msg::DetailGone(id) => {
                self.done_loading();
                let same = |i: &Issue| i.id == id || i.identifier.eq_ignore_ascii_case(&id);
                for d in self.details_mut() {
                    if d.id.eq_ignore_ascii_case(&id) || d.issue.as_ref().is_some_and(same) {
                        d.gone = true;
                        d.loading = false;
                    }
                }
                for data in &mut self.tabs {
                    data.issues.retain(|i| !same(i));
                }
                if let Some(results) = self.results.as_mut() {
                    results.retain(|i| !same(i));
                }
                if self.pinned.as_ref().is_some_and(same) {
                    self.pinned = None;
                }
                self.rebuild(true);
            }
            Msg::Pinned(p) => {
                self.pinned = p;
                self.rebuild(true);
            }
            Msg::KeyOk(v) => {
                let greeting = format!(
                    "{}님, {} 워크스페이스에 연결됐어요",
                    v.name, v.organization.name
                );
                self.viewer = Some(v);
                self.key_checking = false;
                self.key_input.clear();
                self.mode = Mode::Search;
                self.set_flash(greeting, now);
                return self.init_effects();
            }
            Msg::KeyBad(text) => {
                self.key_checking = false;
                self.key_error = Some(text);
            }
            Msg::AuthFailed { env } => {
                self.mode = Mode::Onboarding;
                self.menu = None;
                self.detail = None;
                self.detail_stack.clear();
                self.loading = 0;
                self.env_key_invalid = env;
                self.key_checking = false;
                self.key_input.clear();
                self.key_error = (!env)
                    .then(|| "API 키가 만료됐거나 권한이 없어요. 새 키를 붙여넣으세요".into());
            }
            Msg::Failed(e) => {
                self.done_loading();
                // 어느 요청의 실패인지 모르니 쌓아 둔 상세의 대기도 푼다
                for d in self.details_mut() {
                    d.loading = false;
                }
                self.tabs[Tab::All.index()].loading_more = false;
                match e {
                    ApiError::RateLimited { reset_at_ms } => {
                        // 서버는 응답했으니 오프라인·오류 표시는 지운다. 한도는 잠깐 알리기만 한다
                        self.problem = None;
                        let text = match reset_at_ms {
                            Some(reset) => {
                                self.pause_until(reset);
                                format!(
                                    "Linear API 한도를 넘었어요. {}분 후 다시 시도하세요",
                                    minutes_left(reset, now)
                                )
                            }
                            None => "Linear API 한도를 넘었어요. 잠시 뒤 다시 시도하세요".into(),
                        };
                        self.set_flash(text, now);
                    }
                    ApiError::Offline(m) => self.problem = Some(Problem::Offline(m)),
                    other => self.problem = Some(Problem::Error(other.to_string())),
                }
            }
            Msg::Throttled(until) => {
                self.pause_until(until);
                self.set_flash(
                    format!(
                        "API 한도가 얼마 남지 않아 {}분 동안 자동 서버 검색을 멈춰요",
                        minutes_left(until, now)
                    ),
                    now,
                );
            }
            Msg::DeepLimited => {
                self.done_loading();
                self.set_flash(DEEP_LIMIT_TEXT, now);
            }
            Msg::Flash(text) => self.set_flash(text, now),
            Msg::Warn(text) => {
                // 아직 보이는 경고가 있으면 이어 붙인다 (설정 경고 뒤에 범위 경고가 오는 경우)
                let text = match self.warning_text(now) {
                    Some(prev) => format!("{prev} · {text}"),
                    None => text,
                };
                self.warning = Some((text, now + WARN_MS));
            }
        }
        Vec::new()
    }
}

/// `until`까지 남은 분 (올림).
fn minutes_left(until: i64, now: i64) -> i64 {
    ((until - now).max(0) + 59_999) / 60_000
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linear::types::RelatedIssue;
    use crate::test_support::IssueBuilder;

    const T0: i64 = 1_000_000;

    fn issue(id: &str, identifier: &str, title: &str) -> Issue {
        IssueBuilder::new(id, identifier, title).build()
    }

    fn viewer() -> Viewer {
        serde_json::from_value(serde_json::json!({
            "id": "me", "name": "김민수", "displayName": "minsu", "email": "m@acme.dev",
            "organization": { "id": "org1", "name": "Acme", "urlKey": "acme" },
            "teams": { "nodes": [ { "id": "team-ENG", "key": "ENG", "name": "Eng" } ] }
        }))
        .unwrap()
    }

    fn tab_msg(tab: Tab, issues: Vec<Issue>, fresh: bool) -> Msg {
        Msg::Tab {
            tab,
            issues,
            fresh,
            has_more: false,
            append: false,
        }
    }

    fn started() -> App {
        let (mut app, _) = App::start(None);
        app.apply(Msg::Viewer(viewer()), T0);
        app.apply(
            tab_msg(
                Tab::Mine,
                vec![
                    issue("a", "ENG-1", "로그인 버그"),
                    issue("b", "ENG-2", "결제 화면"),
                ],
                true,
            ),
            T0,
        );
        app
    }

    fn ids(app: &App) -> Vec<String> {
        app.rows
            .iter()
            .map(|r| r.issue().map_or("<deep>".into(), |i| i.identifier.clone()))
            .collect()
    }

    /// 서버 검색 요청 하나의 순번. 순번 값 자체는 내부 카운터라 테스트가 정하지 않는다.
    fn searched(effects: &[Effect], want: &str) -> u64 {
        match effects {
            [Effect::Search { seq, query }] if query == want => *seq,
            other => panic!("'{want}' 서버 검색 하나가 아님: {other:?}"),
        }
    }

    fn type_str(app: &mut App, s: &str, now: i64) {
        for c in s.chars() {
            app.handle(Input::Char(c), now);
        }
    }

    #[test]
    fn start_loads_mine_and_pinned() {
        let (app, effects) = App::start(None);
        assert_eq!(app.mode, Mode::Search);
        assert_eq!(
            effects,
            vec![
                Effect::Init,
                Effect::LoadTab(Tab::Mine),
                Effect::ResolvePinned
            ]
        );
        assert_eq!(app.loading, 1);
    }

    #[test]
    fn start_with_identifier_opens_detail() {
        let (app, effects) = App::start(Some("ENG-7".into()));
        assert_eq!(app.mode, Mode::Detail);
        assert_eq!(effects.last(), Some(&Effect::OpenDetail("ENG-7".into())));
        assert_eq!(app.loading, 2);
    }

    #[test]
    fn cached_then_fresh_tab() {
        let (mut app, _) = App::start(None);
        app.apply(
            tab_msg(Tab::Mine, vec![issue("a", "ENG-1", "옛 제목")], false),
            T0,
        );
        assert_eq!(ids(&app), vec!["ENG-1"]);
        assert_eq!(app.loading, 1, "캐시는 요청을 끝내지 않는다");
        app.apply(
            tab_msg(Tab::Mine, vec![issue("a", "ENG-1", "새 제목")], true),
            T0 + 5,
        );
        assert_eq!(app.rows[0].issue().unwrap().title, "새 제목");
        assert_eq!(app.loading, 0);
        assert_eq!(app.updated_at, Some(T0 + 5));
    }

    #[test]
    fn typing_searches_locally_then_debounces_server_search() {
        let mut app = started();
        type_str(&mut app, "결제", T0);
        assert_eq!(ids(&app), vec!["ENG-2", "<deep>"]);
        assert!(app.tick(T0 + 100).is_empty());
        let seq = searched(&app.tick(T0 + SEARCH_DEBOUNCE_MS), "결제");
        app.apply(
            Msg::Search {
                seq,
                issues: vec![issue("c", "ENG-3", "결제 실패 알림")],
            },
            T0 + 400,
        );
        // 순위가 같으면 서버 결과가 먼저 온다. 검색 행은 항상 마지막
        let found = ids(&app);
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(found.contains(&"ENG-2".to_string()) && found.contains(&"ENG-3".to_string()));
        assert_eq!(found.last().map(String::as_str), Some("<deep>"));
        assert_eq!(app.loading, 0);
    }

    #[test]
    fn stale_search_results_are_ignored() {
        let mut app = started();
        type_str(&mut app, "로", T0);
        let first = searched(&app.tick(T0 + SEARCH_DEBOUNCE_MS), "로");
        type_str(&mut app, "그", T0 + 350);
        searched(&app.tick(T0 + 350 + SEARCH_DEBOUNCE_MS), "로그");
        app.apply(
            Msg::Search {
                seq: first,
                issues: vec![issue("x", "ENG-9", "로그 수집")],
            },
            T0 + 700,
        );
        assert!(!ids(&app).contains(&"ENG-9".to_string()));
        assert_eq!(app.loading, 1, "2번 요청은 아직 기다린다");
    }

    #[test]
    fn selection_kept_by_id_on_merge_and_reset_on_typing() {
        let mut app = started();
        type_str(&mut app, "ENG", T0);
        app.handle(Input::Down, T0);
        let chosen = app.selected_issue().unwrap().id.clone();
        let seq = searched(&app.tick(T0 + SEARCH_DEBOUNCE_MS), "ENG");
        app.apply(
            Msg::Search {
                seq,
                issues: vec![issue("z", "ENG-0", "ENG 맨 앞에 올 결과")],
            },
            T0 + 400,
        );
        assert!(
            ids(&app).contains(&"ENG-0".to_string()),
            "서버 결과가 합쳐졌다"
        );
        assert_eq!(app.selected_issue().unwrap().id, chosen);
        app.handle(Input::Char('-'), T0 + 500);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn pinned_issue_comes_first_without_duplicate() {
        let mut app = started();
        app.apply(Msg::Pinned(Some(issue("b", "ENG-2", "결제 화면"))), T0);
        assert!(matches!(app.rows[0], Row::Pinned(_)));
        assert_eq!(ids(&app), vec!["ENG-2", "ENG-1"]);
        type_str(&mut app, "로그인", T0);
        assert!(!app.rows.iter().any(|r| matches!(r, Row::Pinned(_))));
    }

    #[test]
    fn enter_opens_detail_and_esc_goes_back() {
        let mut app = started();
        let effects = app.handle(Input::Enter, T0);
        assert_eq!(effects, vec![Effect::OpenDetail("a".into())]);
        assert_eq!(app.mode, Mode::Detail);
        app.apply(
            Msg::Detail {
                id: "a".into(),
                issue: issue("a", "ENG-1", "로그인 버그"),
                comments: Vec::new(),
                more: false,
                relations: None,
                fresh: true,
            },
            T0,
        );
        assert!(!app.detail.as_ref().unwrap().loading);
        app.handle(Input::Esc, T0);
        assert_eq!(app.mode, Mode::Search);
        assert!(app.detail.is_none());
    }

    #[test]
    fn detail_by_identifier_accepts_real_id() {
        let (mut app, _) = App::start(Some("eng-1".into()));
        app.apply(
            Msg::Detail {
                id: "a".into(),
                issue: issue("a", "ENG-1", "로그인 버그"),
                comments: Vec::new(),
                more: false,
                relations: None,
                fresh: true,
            },
            T0,
        );
        let d = app.detail.as_ref().unwrap();
        assert_eq!(d.issue.as_ref().unwrap().identifier, "ENG-1");
        assert_eq!(d.id, "a");
    }

    #[test]
    fn late_detail_for_another_issue_is_ignored() {
        let mut app = started();
        app.handle(Input::Enter, T0);
        app.apply(
            Msg::Detail {
                id: "b".into(),
                issue: issue("b", "ENG-2", "결제 화면"),
                comments: vec![],
                more: false,
                relations: None,
                fresh: true,
            },
            T0,
        );
        let d = app.detail.as_ref().unwrap();
        assert_eq!(d.issue.as_ref().unwrap().identifier, "ENG-1");
        assert!(d.loading, "자기 응답을 아직 기다린다");
    }

    #[test]
    fn tab_switch_keeps_the_mode_so_typing_still_searches() {
        let mut app = started();
        app.handle(Input::NextTab, T0);
        assert_eq!(
            app.mode,
            Mode::Search,
            "글자가 동작(r·o·y·q)으로 바뀌면 안 된다"
        );
        type_str(&mut app, "error", T0);
        assert_eq!(app.query, "error");
        app.handle(Input::Esc, T0);
        app.handle(Input::NextTab, T0);
        assert_eq!(app.mode, Mode::List, "목록 모드는 목록 모드로 남는다");
    }

    #[test]
    fn response_for_an_edited_query_is_ignored() {
        let mut app = started();
        type_str(&mut app, "결제", T0);
        let seq = searched(&app.tick(T0 + SEARCH_DEBOUNCE_MS), "결제");
        app.handle(Input::ClearLine, T0 + 400);
        type_str(&mut app, "다크", T0 + 400);
        app.apply(
            Msg::Search {
                seq,
                issues: vec![issue("z", "ENG-77", "결제 승인 실패")],
            },
            T0 + 500,
        );
        assert!(
            !ids(&app).contains(&"ENG-77".to_string()),
            "{:?}",
            ids(&app)
        );
        assert_eq!(app.loading, 0, "늦은 응답도 자기 요청 하나는 끝낸다");
    }

    #[test]
    fn gone_issue_is_marked_and_removed() {
        let mut app = started();
        app.handle(Input::Enter, T0);
        app.apply(Msg::DetailGone("a".into()), T0);
        assert!(app.detail.as_ref().unwrap().gone);
        app.handle(Input::Esc, T0);
        assert_eq!(ids(&app), vec!["ENG-2"]);
    }

    #[test]
    fn gone_matches_the_requested_identifier() {
        // 캐시가 없으면 상세는 식별자만 안다
        let (mut app, _) = App::start(Some("ENG-7".into()));
        app.apply(Msg::DetailGone("ENG-7".into()), T0);
        let d = app.detail.as_ref().unwrap();
        assert!(d.gone && !d.loading);
        // 캐시가 먼저 와서 상세의 id가 바뀐 뒤에도
        let (mut app, _) = App::start(Some("eng-7".into()));
        app.apply(
            Msg::Detail {
                id: "eng-7".into(),
                issue: issue("x7", "ENG-7", "옛 이슈"),
                comments: vec![],
                more: false,
                relations: None,
                fresh: false,
            },
            T0,
        );
        app.apply(Msg::DetailGone("eng-7".into()), T0);
        assert!(app.detail.as_ref().unwrap().gone);
        assert_eq!(app.loading, 1, "상세 요청 하나만 끝났다");
    }

    #[test]
    fn tabs_cycle_and_only_recent_reloads() {
        let mut app = started();
        assert_eq!(
            app.handle(Input::NextTab, T0),
            vec![Effect::LoadTab(Tab::Recent)]
        );
        assert_eq!(app.tab, Tab::Recent);
        assert_eq!(
            app.handle(Input::NextTab, T0),
            vec![Effect::LoadTab(Tab::All)]
        );
        assert!(app.handle(Input::NextTab, T0).is_empty(), "이미 불러온 탭");
        assert_eq!(app.tab, Tab::Mine);
        assert!(app.handle(Input::PrevTab, T0).is_empty(), "이미 불러온 탭");
        assert_eq!(app.tab, Tab::All);
        assert_eq!(
            app.handle(Input::PrevTab, T0),
            vec![Effect::LoadTab(Tab::Recent)],
            "최근 본은 열 때마다 다시 읽는다"
        );
    }

    #[test]
    fn recent_tab_keeps_server_status() {
        let mut app = started();
        app.apply(Msg::Failed(ApiError::Offline("연결 끊김".into())), T0);
        let updated = app.updated_at;
        app.handle(Input::NextTab, T0);
        app.apply(
            tab_msg(Tab::Recent, vec![issue("a", "ENG-1", "로그인 버그")], true),
            T0 + 10,
        );
        assert_eq!(ids(&app), vec!["ENG-1"]);
        assert_eq!(app.loading, 0);
        assert_eq!(app.problem, Some(Problem::Offline("연결 끊김".into())));
        assert_eq!(app.updated_at, updated);
    }

    #[test]
    fn all_tab_loads_more_at_the_end() {
        let mut app = started();
        app.handle(Input::NextTab, T0);
        app.handle(Input::NextTab, T0);
        app.apply(
            Msg::Tab {
                tab: Tab::All,
                issues: vec![issue("a", "ENG-1", "하나"), issue("b", "ENG-2", "둘")],
                fresh: true,
                has_more: true,
                append: false,
            },
            T0,
        );
        assert!(app.handle(Input::Down, T0).contains(&Effect::LoadMore));
        assert!(
            app.handle(Input::Down, T0).is_empty(),
            "불러오는 중엔 다시 안 부른다"
        );
        app.apply(
            Msg::Tab {
                tab: Tab::All,
                issues: vec![issue("c", "ENG-3", "셋")],
                fresh: true,
                has_more: false,
                append: true,
            },
            T0,
        );
        assert_eq!(ids(&app), vec!["ENG-1", "ENG-2", "ENG-3"]);
    }

    #[test]
    fn deep_search_row_runs_deep_search() {
        let mut app = started();
        type_str(&mut app, "세션", T0);
        assert_eq!(ids(&app), vec!["<deep>"]);
        let effects = app.handle(Input::Enter, T0);
        assert!(
            matches!(&effects[..], [Effect::DeepSearch { query, .. }] if query == "세션"),
            "{effects:?}"
        );
    }

    #[test]
    fn list_mode_actions() {
        let mut app = started();
        app.handle(Input::Esc, T0);
        assert_eq!(app.mode, Mode::List);
        assert_eq!(
            app.handle(Input::Act(Act::Browser), T0),
            vec![Effect::OpenUrl(
                "https://linear.app/acme/issue/ENG-1".into()
            )]
        );
        assert_eq!(
            app.handle(Input::Act(Act::CopyId), T0),
            vec![Effect::Copy {
                text: "ENG-1".into(),
                what: "ENG-1".into()
            }]
        );
        app.handle(Input::Search, T0);
        assert_eq!(app.mode, Mode::Search);
        app.handle(Input::Esc, T0);
        app.handle(Input::Esc, T0);
        assert!(app.quit);
    }

    fn with_pr() -> App {
        let (mut app, _) = App::start(None);
        app.apply(
            tab_msg(
                Tab::Mine,
                vec![
                    IssueBuilder::new("a", "ENG-1", "로그인 버그")
                        .pr(
                            "https://github.com/acme/web/pull/15",
                            "open",
                            15,
                            "2026-10-05T00:00:00.000Z",
                        )
                        .build(),
                    issue("b", "ENG-2", "결제 화면"),
                ],
                true,
            ),
            T0,
        );
        app
    }

    #[test]
    fn copy_keys_put_issue_url_first_and_open_pr_second() {
        let mut app = with_pr();
        app.handle(Input::Esc, T0);
        assert_eq!(
            app.handle(Input::Act(Act::CopyUrl), T0),
            vec![Effect::Copy {
                text: "https://linear.app/acme/issue/ENG-1".into(),
                what: "ENG-1 URL".into()
            }]
        );
        assert_eq!(
            app.handle(Input::Act(Act::CopyPr), T0),
            vec![Effect::Copy {
                text: "https://github.com/acme/web/pull/15".into(),
                what: "PR #15".into()
            }]
        );
        app.handle(Input::Down, T0);
        assert!(app.handle(Input::Act(Act::CopyPr), T0).is_empty());
        assert_eq!(app.flash_text(T0), Some("ENG-2에 열린 PR이 없어요"));
    }

    #[test]
    fn menu_starts_with_url_copy_then_pr_then_id() {
        let labels = |app: &App| -> Vec<String> {
            app.menu
                .as_ref()
                .unwrap()
                .visible()
                .iter()
                .map(|(l, _)| l.clone())
                .collect()
        };
        let mut app = with_pr();
        app.handle(Input::Menu, T0);
        assert_eq!(
            labels(&app)[..3],
            ["URL 복사", "PR 링크 복사 (PR #15)", "ID 복사"]
        );
        app.handle(Input::Esc, T0);
        app.handle(Input::Down, T0);
        app.handle(Input::Menu, T0);
        assert_eq!(
            labels(&app)[..2],
            ["URL 복사", "ID 복사"],
            "PR이 없으면 빠진다"
        );
    }

    #[test]
    fn mouse_click_selects_then_opens_and_wheel_moves() {
        let mut app = started();
        assert!(app.handle(Input::ClickRow(1), T0).is_empty());
        assert_eq!(app.selected, 1);
        assert_eq!(
            app.handle(Input::ClickRow(1), T0),
            vec![Effect::OpenDetail("b".into())],
            "선택된 줄을 다시 누르면 연다"
        );
        assert_eq!(app.mode, Mode::Detail);
        app.set_detail_max_scroll(10);
        app.handle(Input::ScrollDown, T0);
        assert_eq!(app.detail.as_ref().unwrap().scroll, 3);
        app.handle(Input::ScrollUp, T0);
        assert_eq!(app.detail.as_ref().unwrap().scroll, 0);
        app.handle(Input::Esc, T0);
        app.handle(Input::ScrollUp, T0);
        assert_eq!(app.selected, 0);
        assert_eq!(
            app.handle(Input::ClickTab(Tab::All), T0),
            vec![Effect::LoadTab(Tab::All)]
        );
        assert_eq!(app.tab, Tab::All);
        assert!(app.handle(Input::ClickRow(99), T0).is_empty(), "없는 줄");
    }

    #[test]
    fn mouse_runs_menu_items_and_wheel_moves_the_menu() {
        let mut app = started();
        app.handle(Input::Menu, T0);
        app.handle(Input::ScrollDown, T0);
        assert_eq!(app.menu.as_ref().unwrap().selected, 1);
        assert_eq!(
            app.handle(Input::ClickMenu(0), T0),
            vec![Effect::Copy {
                text: "https://linear.app/acme/issue/ENG-1".into(),
                what: "ENG-1 URL".into()
            }],
            "누른 항목을 바로 실행한다"
        );
        assert!(app.menu.is_none());
        let mut onboarding = App::onboarding(false);
        assert!(onboarding.handle(Input::ClickRow(0), T0).is_empty());
        assert!(onboarding.handle(Input::ScrollDown, T0).is_empty());
        assert!(onboarding.key_input.is_empty());
    }

    #[test]
    fn menu_filters_and_runs_actions() {
        let mut app = started();
        app.handle(Input::Menu, T0);
        let labels: Vec<String> = app
            .menu
            .as_ref()
            .unwrap()
            .visible()
            .iter()
            .map(|(l, _)| l.clone())
            .collect();
        assert!(labels.contains(&"상세 보기".to_string()), "{labels:?}");
        type_str(&mut app, "URL", T0);
        assert_eq!(app.menu.as_ref().unwrap().visible().len(), 1);
        assert_eq!(
            app.handle(Input::Enter, T0),
            vec![Effect::Copy {
                text: "https://linear.app/acme/issue/ENG-1".into(),
                what: "ENG-1 URL".into()
            }]
        );
        assert!(app.menu.is_none());
    }

    #[test]
    fn links_menu_lists_description_links() {
        let mut app = started();
        app.handle(Input::Enter, T0);
        let with_links = IssueBuilder::new("a", "ENG-1", "로그인 버그")
            .description("[문서](https://x.dev/doc) ![](https://x.dev/a.png)")
            .build();
        app.apply(
            Msg::Detail {
                id: "a".into(),
                issue: with_links,
                comments: Vec::new(),
                more: false,
                relations: None,
                fresh: true,
            },
            T0,
        );
        app.handle(Input::Act(Act::Links), T0);
        let menu = app.menu.as_ref().unwrap();
        assert_eq!(menu.items.len(), 2);
        assert_eq!(
            app.handle(Input::Enter, T0),
            vec![Effect::OpenUrl("https://x.dev/doc".into())]
        );
    }

    #[test]
    fn rate_limit_is_a_short_notice_and_pauses_server_search() {
        let mut app = started();
        app.apply(Msg::Failed(ApiError::Offline("연결 끊김".into())), T0);
        app.apply(
            Msg::Failed(ApiError::RateLimited {
                reset_at_ms: Some(T0 + 60_000),
            }),
            T0,
        );
        assert_eq!(app.problem, None, "상단에 남기지 않는다");
        assert_eq!(
            app.flash_text(T0),
            Some("Linear API 한도를 넘었어요. 1분 후 다시 시도하세요")
        );
        assert_eq!(app.flash_text(T0 + FLASH_MS), None, "잠깐만 보인다");
        type_str(&mut app, "결제", T0);
        assert!(app.tick(T0 + 1_000).is_empty());
        assert_eq!(app.tick(T0 + 60_000).len(), 1);
    }

    #[test]
    fn rate_limit_without_reset_keeps_the_throttle_pause() {
        let mut app = started();
        app.apply(Msg::Throttled(T0 + 120_000), T0);
        app.apply(Msg::Failed(ApiError::RateLimited { reset_at_ms: None }), T0);
        assert_eq!(
            app.flash_text(T0),
            Some("Linear API 한도를 넘었어요. 잠시 뒤 다시 시도하세요")
        );
        type_str(&mut app, "결제", T0);
        assert!(app.tick(T0 + 1_000).is_empty(), "앞서 정한 멈춤이 남는다");
        assert_eq!(app.tick(T0 + 120_000).len(), 1);
    }

    #[test]
    fn refresh_that_hits_the_limit_again_pauses_again() {
        let mut app = started();
        let limited = |reset| {
            Msg::Failed(ApiError::RateLimited {
                reset_at_ms: Some(reset),
            })
        };
        app.apply(limited(T0 + 60_000), T0);
        type_str(&mut app, "결제", T0);
        app.handle(Input::Esc, T0);
        app.handle(Input::Act(Act::Refresh), T0 + 5_000);
        searched(&app.tick(T0 + 5_000), "결제");
        app.apply(limited(T0 + 60_000), T0 + 6_000);
        assert_eq!(app.loading, 0);
        assert!(
            app.flash_text(T0 + 6_000)
                .is_some_and(|t| t.contains("한도"))
        );
        app.handle(Input::Search, T0 + 6_000);
        type_str(&mut app, "x", T0 + 6_000);
        assert!(app.tick(T0 + 7_000).is_empty(), "다시 멈춘다");
    }

    #[test]
    fn deep_search_limit_keeps_local_results_and_refresh_time() {
        let mut app = started();
        type_str(&mut app, "로그인", T0 + 1_000);
        let before = ids(&app);
        app.handle(Input::Bottom, T0 + 1_000);
        let effects = app.handle(Input::Enter, T0 + 1_000);
        assert!(
            matches!(&effects[..], [Effect::DeepSearch { .. }]),
            "{effects:?}"
        );
        app.apply(Msg::DeepLimited, T0 + 2_000);
        assert_eq!(app.loading, 0);
        assert_eq!(app.flash_text(T0 + 2_000), Some(DEEP_LIMIT_TEXT));
        assert_eq!(app.updated_at, Some(T0), "갱신 시각을 바꾸지 않는다");
        assert_eq!(ids(&app), before, "로컬 결과를 그대로 둔다");
    }

    #[test]
    fn throttle_pauses_auto_search_but_refresh_still_searches() {
        let mut app = started();
        app.apply(Msg::Throttled(T0 + 120_000), T0);
        assert!(app.flash_text(T0).unwrap().contains("2분"));
        assert_eq!(app.loading, 0, "요청을 세지 않는다");
        type_str(&mut app, "결제", T0);
        assert!(app.tick(T0 + 1_000).is_empty(), "자동 검색은 멈춘다");
        app.handle(Input::Esc, T0 + 1_000);
        app.handle(Input::Act(Act::Refresh), T0 + 1_000);
        assert_eq!(app.tick(T0 + 1_000).len(), 1, "직접 새로고침은 보낸다");
    }

    #[test]
    fn onboarding_validates_key() {
        let mut app = App::onboarding(false);
        app.handle(Input::Paste(" lin_api_abc\n".into()), T0);
        assert_eq!(app.key_input, "lin_api_abc");
        assert_eq!(
            app.handle(Input::Enter, T0),
            vec![Effect::ValidateKey("lin_api_abc".into())]
        );
        app.apply(Msg::KeyBad("키가 유효하지 않아요".into()), T0);
        assert_eq!(app.key_error.as_deref(), Some("키가 유효하지 않아요"));
        app.handle(Input::Enter, T0);
        let effects = app.apply(Msg::KeyOk(viewer()), T0);
        assert_eq!(app.mode, Mode::Search);
        assert_eq!(effects[0], Effect::Init);
        assert_eq!(
            app.flash_text(T0),
            Some("김민수님, Acme 워크스페이스에 연결됐어요")
        );
        assert_eq!(app.flash_text(T0 + FLASH_MS), None);
    }

    #[test]
    fn auth_failure_returns_to_onboarding() {
        let mut app = started();
        app.apply(Msg::AuthFailed { env: true }, T0);
        assert_eq!(app.mode, Mode::Onboarding);
        assert!(app.env_key_invalid);
        assert!(app.handle(Input::Char('x'), T0).is_empty());
        assert!(app.key_input.is_empty());
        app.handle(Input::Esc, T0);
        assert!(app.quit);
    }

    #[test]
    fn detail_scroll_is_clamped() {
        let mut app = started();
        app.handle(Input::Enter, T0);
        app.set_detail_max_scroll(3);
        for _ in 0..10 {
            app.handle(Input::Down, T0);
        }
        assert_eq!(app.detail.as_ref().unwrap().scroll, 3);
        app.handle(Input::Top, T0);
        assert_eq!(app.detail.as_ref().unwrap().scroll, 0);
    }

    #[test]
    fn quit_from_a_detail_opened_in_search_mode_closes() {
        let mut app = started();
        app.handle(Input::Enter, T0);
        assert_eq!(app.mode, Mode::Detail);
        app.handle(Input::Act(Act::Quit), T0);
        assert!(app.quit);
        assert_eq!(app.query, "", "검색어에 들어가지 않는다");
    }

    #[test]
    fn warnings_join_and_outlast_flashes() {
        let mut app = App::onboarding(false);
        app.apply(Msg::Warn("설정 경고: A".into()), T0);
        app.apply(Msg::Warn("경고: B".into()), T0 + 1_000);
        app.apply(Msg::Flash("복사됨: ENG-1".into()), T0 + 1_000);
        assert_eq!(
            app.warning_text(T0 + 1_000 + FLASH_MS),
            Some("설정 경고: A · 경고: B")
        );
        assert_eq!(app.warning_text(T0 + 1_000 + WARN_MS), None);
    }

    fn rel(id: &str, identifier: &str, title: &str, state_type: &str) -> RelatedIssue {
        let i = IssueBuilder::new(id, identifier, title)
            .state(state_type, state_type)
            .build();
        RelatedIssue {
            id: i.id,
            identifier: i.identifier,
            title: i.title,
            state: i.state,
        }
    }

    /// ENG-1(상위 ENG-10): 막힘 ENG-20, 하위 ENG-30·ENG-31.
    fn related() -> (Issue, IssueRelations) {
        let issue = IssueBuilder::new("a", "ENG-1", "로그인 버그")
            .parent("p", "ENG-10", "인증 개편", "started")
            .build();
        let relations = IssueRelations {
            blocked_by: vec![rel("b", "ENG-20", "API 스키마", "unstarted")],
            children: vec![
                rel("c1", "ENG-30", "토큰 갱신", "started"),
                rel("c2", "ENG-31", "세션 만료", "completed"),
            ],
            ..IssueRelations::default()
        };
        (issue, relations)
    }

    fn detail_msg(id: &str, issue: Issue, relations: Option<IssueRelations>, fresh: bool) -> Msg {
        Msg::Detail {
            id: id.into(),
            issue,
            comments: Vec::new(),
            more: false,
            relations,
            fresh,
        }
    }

    /// 첫 이슈의 상세를 열고 관계까지 받은 앱.
    fn with_relations() -> App {
        let mut app = started();
        app.handle(Input::Enter, T0);
        let (issue, relations) = related();
        app.apply(detail_msg("a", issue, Some(relations), true), T0);
        app
    }

    fn menu_labels(app: &App) -> Vec<String> {
        app.menu
            .as_ref()
            .unwrap()
            .visible()
            .iter()
            .map(|(l, _)| l.clone())
            .collect()
    }

    #[test]
    fn relations_menu_lists_rows_in_screen_order() {
        let mut app = with_relations();
        app.handle(Input::Act(Act::Relations), T0);
        assert_eq!(app.menu.as_ref().unwrap().title, "관계");
        assert_eq!(
            menu_labels(&app),
            vec![
                "상위 ENG-10 인증 개편",
                "막힘 ENG-20 API 스키마",
                "하위 ENG-30 토큰 갱신",
                "하위 ENG-31 세션 만료",
            ]
        );
    }

    #[test]
    fn opening_a_relation_stacks_the_detail_and_esc_comes_back() {
        let mut app = with_relations();
        app.set_detail_max_scroll(10);
        app.handle(Input::Down, T0);
        app.handle(Input::Down, T0);
        app.handle(Input::Act(Act::Relations), T0);
        type_str(&mut app, "31", T0);
        let effects = app.handle(Input::Enter, T0);
        assert_eq!(effects, vec![Effect::OpenDetail("c2".into())]);
        assert_eq!(app.mode, Mode::Detail);
        let d = app.detail.as_ref().unwrap();
        assert_eq!(d.id, "c2");
        assert!(d.loading && d.issue.is_none());
        assert_eq!(app.loading, 1, "연 관계 이슈 요청 하나");
        app.handle(Input::Esc, T0);
        let d = app.detail.as_ref().unwrap();
        assert_eq!(d.issue.as_ref().unwrap().identifier, "ENG-1", "원래 이슈로");
        assert_eq!(d.scroll, 2, "스크롤 위치 그대로");
        assert_eq!(app.mode, Mode::Detail);
        app.handle(Input::Esc, T0);
        assert_eq!(app.mode, Mode::Search, "처음 이슈에서 Esc는 목록으로");
        assert!(app.detail.is_none());
    }

    #[test]
    fn clicking_a_relation_line_opens_that_issue() {
        let mut app = with_relations();
        assert_eq!(
            app.handle(Input::ClickRelation(1), T0),
            vec![Effect::OpenDetail("b".into())]
        );
        assert_eq!(app.detail.as_ref().unwrap().id, "b");
        assert!(
            app.handle(Input::ClickRelation(99), T0).is_empty(),
            "없는 줄"
        );
    }

    #[test]
    fn response_for_a_stacked_detail_still_lands() {
        let mut app = started();
        app.handle(Input::Enter, T0);
        let (issue, relations) = related();
        // 캐시가 먼저 오고 서버 응답은 아직이다
        app.apply(
            detail_msg("a", issue.clone(), Some(relations.clone()), false),
            T0,
        );
        app.handle(Input::ClickRelation(0), T0);
        assert_eq!(app.detail.as_ref().unwrap().id, "p");
        let mut fresh = issue;
        fresh.title = "새 제목".into();
        app.apply(detail_msg("a", fresh, Some(relations), true), T0);
        assert_eq!(app.detail.as_ref().unwrap().id, "p", "지금 상세는 그대로");
        app.handle(Input::Esc, T0);
        let d = app.detail.as_ref().unwrap();
        assert_eq!(d.issue.as_ref().unwrap().title, "새 제목");
        assert!(!d.loading, "돌아와도 불러오는 중에 멈춰 있지 않다");
        assert_eq!(app.loading, 1, "상위 이슈 요청만 남았다");
    }

    #[test]
    fn failure_clears_waiting_on_stacked_details_too() {
        let mut app = started();
        app.handle(Input::Enter, T0);
        let (issue, relations) = related();
        app.apply(detail_msg("a", issue, Some(relations), false), T0);
        app.handle(Input::ClickRelation(0), T0);
        app.apply(Msg::Failed(ApiError::Offline("x".into())), T0);
        app.handle(Input::Esc, T0);
        assert!(!app.detail.as_ref().unwrap().loading);
    }

    #[test]
    fn relations_menu_title_says_what_we_know() {
        let title = |app: &mut App| {
            app.handle(Input::Act(Act::Relations), T0);
            let t = app.menu.as_ref().unwrap().title;
            app.handle(Input::Esc, T0);
            t
        };
        let mut app = started();
        app.handle(Input::Enter, T0);
        assert_eq!(title(&mut app), "관계 불러오는 중…");
        app.apply(Msg::Failed(ApiError::Offline("x".into())), T0);
        assert_eq!(title(&mut app), "관계를 불러오지 못했어요");
        app.apply(
            detail_msg(
                "a",
                issue("a", "ENG-1", "로그인 버그"),
                Some(IssueRelations::default()),
                true,
            ),
            T0,
        );
        assert_eq!(title(&mut app), "관계 없음");
        assert!(app.menu.is_none());
    }

    #[test]
    fn detail_menu_has_relations_after_links() {
        let mut app = with_relations();
        app.handle(Input::Menu, T0);
        let labels = menu_labels(&app);
        let links = labels.iter().position(|l| l == "링크·이미지 목록").unwrap();
        assert_eq!(labels[links + 1], "관계 이슈");
        app.handle(Input::Esc, T0);
        app.handle(Input::Esc, T0);
        app.handle(Input::Menu, T0);
        assert!(
            !menu_labels(&app).contains(&"관계 이슈".to_string()),
            "목록에서는 없다"
        );
    }

    #[test]
    fn auth_failure_drops_the_detail_stack() {
        let mut app = with_relations();
        app.handle(Input::ClickRelation(0), T0);
        app.apply(Msg::AuthFailed { env: false }, T0);
        assert!(app.detail.is_none() && app.detail_stack.is_empty());
    }

    #[test]
    fn gone_lands_on_stacked_details_too() {
        let mut app = with_relations();
        app.handle(Input::ClickRelation(1), T0); // ENG-20을 연다. ENG-1은 쌓인다
        app.apply(Msg::DetailGone("ENG-1".into()), T0);
        let cur = app.detail.as_ref().unwrap();
        assert!(!cur.gone && cur.loading, "지금 상세는 다른 이슈라 그대로");
        app.handle(Input::Esc, T0);
        let back = app.detail.as_ref().unwrap();
        assert!(back.gone && !back.loading);
    }

    #[test]
    fn response_without_relations_keeps_the_known_ones() {
        let mut app = with_relations();
        let (issue, _) = related();
        app.apply(detail_msg("a", issue, None, true), T0);
        assert!(app.detail.as_ref().unwrap().relations.is_some());
        app.handle(Input::Act(Act::Relations), T0);
        assert_eq!(app.menu.as_ref().unwrap().title, "관계");
    }

    #[test]
    fn unknown_relations_still_list_the_parent() {
        let mut app = started();
        app.handle(Input::Enter, T0);
        let with_parent = IssueBuilder::new("a", "ENG-1", "로그인 버그")
            .parent("p", "ENG-10", "인증 개편", "started")
            .build();
        // 캐시가 먼저 왔고 관계는 아직 모른다
        app.apply(detail_msg("a", with_parent, None, false), T0);
        app.handle(Input::Act(Act::Relations), T0);
        assert_eq!(app.menu.as_ref().unwrap().title, "관계 불러오는 중…");
        assert_eq!(menu_labels(&app), vec!["상위 ENG-10 인증 개편"]);
        app.handle(Input::Esc, T0);
        // 받다가 실패해도 아는 상위는 그대로 보여 준다
        app.apply(Msg::Failed(ApiError::Offline("x".into())), T0);
        app.handle(Input::Act(Act::Relations), T0);
        assert_eq!(app.menu.as_ref().unwrap().title, "관계를 불러오지 못했어요");
        assert_eq!(menu_labels(&app), vec!["상위 ENG-10 인증 개편"]);
        assert_eq!(
            app.handle(Input::Enter, T0),
            vec![Effect::OpenDetail("p".into())],
            "알려진 상위는 관계를 몰라도 열 수 있다"
        );
    }
}
