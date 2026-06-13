//! 한국어/영어 UI 문자열. 언어는 `Signal<Lang>` 컨텍스트로 내려가고,
//! 토글한 선택은 localStorage와 `<html lang>` 속성에 보존된다.
//! 모든 문자열이 `Lang` 메서드라서 번역 누락이 컴파일 에러로 잡힌다.
//! 백엔드가 만드는 에러 메시지는 1차 범위 밖이라 한국어로 내려온다.
//!
//! 문구는 화면 영역별 서브모듈에 나뉘어 산다. 단계·상태·집계 방식의
//! *데이터 표시 레이블*(kind_label 등)은 여기가 아니라 models에 있다 —
//! i18n은 화면 문구, models는 도메인 값의 표기를 책임진다.

use crate::preferences::PersistedToggle;
use dioxus::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ko,
    En,
}

/// `App`이 제공한 언어 시그널. 읽는 컴포넌트는 언어가 바뀌면 다시 그린다.
pub fn use_lang() -> Signal<Lang> { use_context::<Signal<Lang>>() }

/// 시작 언어: 저장된 사용자 선택 > 한국어.
pub fn initial_lang() -> Lang { Lang::initial() }

/// `<html lang>`을 갱신하고 선택을 저장한다.
pub fn apply_lang(lang: Lang) { lang.apply(); }

pub(crate) fn pick(lang: Lang, ko: &'static str, en: &'static str) -> &'static str {
    match lang {
        | Lang::Ko => ko,
        | Lang::En => en,
    }
}

impl Lang {
    pub fn toggled(self) -> Self {
        match self {
            | Self::Ko => Self::En,
            | Self::En => Self::Ko,
        }
    }
}

impl PersistedToggle for Lang {
    const HTML_ATTR: Option<&'static str> = Some("lang");
    const STORAGE_KEY: &'static str = "lang";

    fn as_storage_value(self) -> &'static str { pick(self, "ko", "en") }

    fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            | "en" => Some(Self::En),
            | "ko" => Some(Self::Ko),
            | _ => None,
        }
    }

    /// 기본 언어: 한국어.
    fn fallback() -> Self { Self::Ko }
}

mod common;
mod dashboard;
mod detail;
mod form;
mod records;
mod tree;
