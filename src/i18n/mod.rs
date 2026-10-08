//! 화면·CLI·알림·로그 문구의 언어. 기본은 영어이고 `config.toml`의 `language`로 바꾼다.
//! 문구는 언어마다 `Texts` 하나에 모여 있다: `t().tab_mine`, `(t().minutes_ago)(5)`.

mod de;
mod en;
mod ja;
mod ko;
mod texts;
mod zh_cn;

use std::cell::Cell;
use std::sync::atomic::{AtomicU8, Ordering};

pub use texts::Texts;

/// 지원하는 언어. 값은 `ALL`의 순서와 같다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Lang {
    En = 0,
    Ko = 1,
    Ja = 2,
    ZhCn = 3,
    De = 4,
}

impl Lang {
    pub const ALL: [Lang; 5] = [Lang::En, Lang::Ko, Lang::Ja, Lang::ZhCn, Lang::De];

    /// 실제 실행의 기본 언어.
    #[cfg(not(test))]
    const DEFAULT: Lang = Lang::En;
    /// 단위 테스트는 한국어로 돈다. 한국어 문구를 확인하는 기존 테스트가 한국어 카탈로그의 회귀 검사가 된다.
    #[cfg(test)]
    const DEFAULT: Lang = Lang::Ko;

    /// 언어 코드. 스크린샷 디렉터리 이름과 SVG의 `xml:lang`에 쓴다.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ko => "ko",
            Lang::Ja => "ja",
            Lang::ZhCn => "zh-CN",
            Lang::De => "de",
        }
    }

    /// 설정 값을 언어로 바꾼다. 대소문자·앞뒤 공백·`_`/`-`는 가리지 않고,
    /// POSIX 로캘의 꼬리(`.UTF-8`, `@euro`)는 떼고,
    /// 지역 태그(`de-DE`, `ja_JP`, `zh-Hans-CN`)는 앞부분으로 본다.
    /// 번체 중국어(`zh-TW`, `zh-HK`, `zh-Hant`, `zh_TW.UTF-8` 등)와 모르는 값은 `None`이다.
    pub fn parse(value: &str) -> Option<Lang> {
        let lowered = value.trim().to_lowercase().replace('_', "-");
        // `zh_TW.UTF-8`의 `.UTF-8`이 지역 태그 `tw`에 붙어 번체를 놓치지 않게, 별칭·분할 전에 자른다
        let v = lowered.split(['.', '@']).next().unwrap_or("");
        match v {
            "english" => return Some(Lang::En),
            "korean" | "한국어" => return Some(Lang::Ko),
            "japanese" | "日本語" => return Some(Lang::Ja),
            "chinese" | "中文" | "简体中文" => return Some(Lang::ZhCn),
            "german" | "deutsch" => return Some(Lang::De),
            _ => {}
        }
        let mut parts = v.split('-');
        let rest: Vec<&str> = parts.clone().skip(1).collect();
        match parts.next().unwrap_or("") {
            "en" => Some(Lang::En),
            "ko" | "kr" => Some(Lang::Ko),
            "ja" | "jp" => Some(Lang::Ja),
            "de" => Some(Lang::De),
            "zh" => {
                let simplified = rest.contains(&"hans");
                let traditional = rest
                    .iter()
                    .any(|p| matches!(*p, "hant" | "tw" | "hk" | "mo"));
                (simplified || !traditional).then_some(Lang::ZhCn)
            }
            _ => None,
        }
    }

    fn from_u8(n: u8) -> Lang {
        Lang::ALL
            .get(usize::from(n))
            .copied()
            .unwrap_or(Lang::DEFAULT)
    }
}

/// 프로세스 전역 언어. `main`이 명령을 해석하기 전에 정한다.
static GLOBAL: AtomicU8 = AtomicU8::new(Lang::DEFAULT as u8);

thread_local! {
    /// 이 스레드만의 언어 (`with_lang`, 단위 테스트의 `set_lang`).
    static OVERRIDE: Cell<Option<Lang>> = const { Cell::new(None) };
}

/// 지금 언어: 이 스레드의 덮어쓰기가 있으면 그것, 없으면 프로세스 전역 값.
pub fn lang() -> Lang {
    OVERRIDE
        .with(Cell::get)
        .unwrap_or_else(|| Lang::from_u8(GLOBAL.load(Ordering::Relaxed)))
}

/// 프로세스의 언어를 정한다.
#[cfg(not(test))]
pub fn set_lang(lang: Lang) {
    GLOBAL.store(lang as u8, Ordering::Relaxed);
}

/// 단위 테스트에서는 이 스레드의 언어만 바꾼다. 병렬로 도는 다른 테스트에 번지지 않게 한다.
#[cfg(test)]
pub fn set_lang(lang: Lang) {
    OVERRIDE.with(|o| o.set(Some(lang)));
}

/// `f`를 도는 동안만 이 스레드의 언어를 `lang`으로 둔다. 끝나면(패닉이어도) 되돌린다.
pub fn with_lang<R>(lang: Lang, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Lang>);
    impl Drop for Restore {
        fn drop(&mut self) {
            OVERRIDE.with(|o| o.set(self.0));
        }
    }
    let _restore = Restore(OVERRIDE.with(|o| o.replace(Some(lang))));
    f()
}

static EN: Texts = en::EN;
static KO: Texts = ko::KO;
static JA: Texts = ja::JA;
static ZH_CN: Texts = zh_cn::ZH_CN;
static DE: Texts = de::DE;

/// 그 언어의 문구.
pub fn texts_for(lang: Lang) -> &'static Texts {
    match lang {
        Lang::En => &EN,
        Lang::Ko => &KO,
        Lang::Ja => &JA,
        Lang::ZhCn => &ZH_CN,
        Lang::De => &DE,
    }
}

/// 지금 언어의 문구.
pub fn t() -> &'static Texts {
    texts_for(lang())
}

/// 한글(음절·자모)이 들어 있는지. 영어 등 다른 언어 화면에 한국어가 남았는지 볼 때 쓴다.
#[cfg(test)]
pub(crate) fn has_hangul(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(c, '\u{AC00}'..='\u{D7A3}' | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_aliases_and_region_tags() {
        for (value, want) in [
            ("en", Lang::En),
            ("English", Lang::En),
            ("en-US", Lang::En),
            ("ko", Lang::Ko),
            ("KR", Lang::Ko),
            ("korean", Lang::Ko),
            ("한국어", Lang::Ko),
            ("ko_KR", Lang::Ko),
            ("ja", Lang::Ja),
            ("jp", Lang::Ja),
            ("Japanese", Lang::Ja),
            ("日本語", Lang::Ja),
            ("ja_JP", Lang::Ja),
            ("zh", Lang::ZhCn),
            ("zh-CN", Lang::ZhCn),
            ("ZH-cn", Lang::ZhCn),
            ("zh-Hans", Lang::ZhCn),
            ("zh-Hans-CN", Lang::ZhCn),
            ("zh-Hans-TW", Lang::ZhCn),
            ("zh-SG", Lang::ZhCn),
            ("chinese", Lang::ZhCn),
            ("中文", Lang::ZhCn),
            ("简体中文", Lang::ZhCn),
            ("de", Lang::De),
            ("German", Lang::De),
            ("Deutsch", Lang::De),
            ("de-DE", Lang::De),
            ("de_AT", Lang::De),
            ("  ko  ", Lang::Ko),
            // POSIX 로캘의 꼬리(`.인코딩`, `@수식어`)는 무시한다
            ("ko_KR.UTF-8", Lang::Ko),
            ("de_DE@euro", Lang::De),
            ("ja_JP.eucJP", Lang::Ja),
            ("zh_CN.UTF-8", Lang::ZhCn),
        ] {
            assert_eq!(Lang::parse(value), Some(want), "{value}");
        }
    }

    #[test]
    fn rejects_traditional_chinese_and_unknown_values() {
        for value in [
            "zh-TW",
            "zh_HK",
            "zh-Hant",
            "zh-Hant-TW",
            "fr",
            "",
            "xx-YY",
            "english-ish",
            // 로캘의 꼬리가 붙어도 번체는 번체다
            "zh_TW.UTF-8",
            "zh_HK.UTF-8",
        ] {
            assert_eq!(Lang::parse(value), None, "{value}");
        }
    }

    #[test]
    fn codes_parse_back() {
        for l in Lang::ALL {
            assert_eq!(Lang::parse(l.code()), Some(l));
        }
    }

    #[test]
    fn discriminants_round_trip_through_all_and_from_u8() {
        // `GLOBAL`에는 번호가 저장된다: 번호 ↔ `ALL`의 순서 ↔ `from_u8`이 어긋나면 엉뚱한 언어가 된다
        for l in Lang::ALL {
            assert_eq!(Lang::from_u8(l as u8), l, "{l:?}");
        }
    }

    #[test]
    fn unit_tests_default_to_korean() {
        assert_eq!(lang(), Lang::Ko);
    }

    #[test]
    fn with_lang_overrides_and_restores() {
        let inner = with_lang(Lang::En, || {
            assert_eq!(lang(), Lang::En);
            with_lang(Lang::De, lang)
        });
        assert_eq!(inner, Lang::De);
        assert_eq!(lang(), Lang::Ko);
    }

    #[test]
    fn with_lang_restores_after_a_panic() {
        let r = std::panic::catch_unwind(|| with_lang(Lang::Ja, || panic!("boom")));
        assert!(r.is_err());
        assert_eq!(lang(), Lang::Ko);
    }

    #[test]
    fn set_lang_in_unit_tests_changes_only_this_thread() {
        set_lang(Lang::En);
        assert_eq!(lang(), Lang::En);
        assert_eq!(std::thread::spawn(lang).join().unwrap(), Lang::Ko);
    }

    #[test]
    fn t_follows_the_language() {
        assert_eq!(t().cli_about, "herdr에서 Linear를 빠르게 조회");
        assert_eq!(
            with_lang(Lang::En, || t().cli_about),
            "Fast Linear lookup in herdr"
        );
        for l in Lang::ALL {
            assert!(!texts_for(l).cli_about.is_empty(), "{l:?}");
            assert!(!texts_for(l).no_api_key.is_empty(), "{l:?}");
        }
    }

    #[test]
    fn catalogs_other_than_korean_have_no_hangul() {
        for (name, src) in [
            ("en.rs", include_str!("en.rs")),
            ("ja.rs", include_str!("ja.rs")),
            ("zh_cn.rs", include_str!("zh_cn.rs")),
            ("de.rs", include_str!("de.rs")),
        ] {
            for (n, line) in src.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                assert!(!has_hangul(line), "{name}:{}: {line}", n + 1);
            }
        }
    }

    #[test]
    fn japanese_catalog_is_translated() {
        let (ja, en) = (texts_for(Lang::Ja), texts_for(Lang::En));
        for (a, b) in [
            (ja.cli_about, en.cli_about),
            (ja.no_api_key, en.no_api_key),
            (ja.tab_mine, en.tab_mine),
            (ja.menu_close, en.menu_close),
            (ja.rel_blocked_by, en.rel_blocked_by),
            (ja.hints_list, en.hints_list),
            (ja.deep_limit, en.deep_limit),
            (ja.herdr_busy, en.herdr_busy),
        ] {
            assert_ne!(a, b);
        }
        assert_ne!((ja.minutes_ago)(5), (en.minutes_ago)(5));
        assert!(!include_str!("ja.rs").contains("..EN"));
    }

    #[test]
    fn simplified_chinese_catalog_is_translated() {
        let (zh, en) = (texts_for(Lang::ZhCn), texts_for(Lang::En));
        for (a, b) in [
            (zh.cli_about, en.cli_about),
            (zh.no_api_key, en.no_api_key),
            (zh.tab_mine, en.tab_mine),
            (zh.menu_close, en.menu_close),
            (zh.rel_blocked_by, en.rel_blocked_by),
            (zh.hints_list, en.hints_list),
            (zh.deep_limit, en.deep_limit),
            (zh.herdr_busy, en.herdr_busy),
        ] {
            assert_ne!(a, b);
        }
        assert_ne!((zh.minutes_ago)(5), (en.minutes_ago)(5));
        assert!(!include_str!("zh_cn.rs").contains("..EN"));
    }

    #[test]
    fn german_catalog_is_translated() {
        let (de, en) = (texts_for(Lang::De), texts_for(Lang::En));
        for (a, b) in [
            (de.cli_about, en.cli_about),
            (de.no_api_key, en.no_api_key),
            (de.tab_mine, en.tab_mine),
            (de.menu_close, en.menu_close),
            (de.rel_blocked_by, en.rel_blocked_by),
            (de.hints_list, en.hints_list),
            (de.deep_limit, en.deep_limit),
            (de.herdr_busy, en.herdr_busy),
        ] {
            assert_ne!(a, b);
        }
        assert_ne!((de.minutes_ago)(5), (en.minutes_ago)(5));
        assert_eq!((de.whoami_scope)(1), "Suchbereich: 1 Team");
        assert_eq!((de.whoami_scope)(2), "Suchbereich: 2 Teams");
        assert!(!include_str!("de.rs").contains("..EN"));
    }

    #[test]
    fn german_counted_words_use_the_singular_for_one() {
        let de = texts_for(Lang::De);
        // `minuten` 도우미를 쓰는 두 문구: 1분은 단수("1 Minute"), 그 밖에는 복수
        for f in [de.rate_limited_retry_in, de.throttled] {
            let (one, five) = (f(1), f(5));
            assert!(
                one.contains("1 Minute") && !one.contains("Minuten"),
                "{one}"
            );
            assert!(five.contains("5 Minuten"), "{five}");
        }
        // 하위가 하나뿐이면 "alle"을 붙이지 않는다
        assert_eq!((de.children_all_done)(1), "1 erledigt");
        assert_eq!((de.children_all_done)(3), "alle 3 erledigt");
    }
}
