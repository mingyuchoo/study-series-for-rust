//! localStorage에 보존되는 토글 선호값(모드·테마·언어)의 공통 구현.
//!
//! 세 가지 토글이 각자 반복하던 "읽기→파싱→폴백 / 저장→`<html>` 속성
//! 반영"을 한곳으로 모은다. 각 enum은 저장 문자열 변환과 기본값만 정의하면
//! `initial()`·`apply()`를 공짜로 얻는다 — 새 토글을 추가할 때 저장소
//! 접근 코드를 다시 쓰지 않는다.

/// 브라우저 localStorage 핸들. SSR·비웹 환경 등에서 없을 수 있다.
fn local_storage() -> Option<web_sys::Storage> { web_sys::window().and_then(|window| window.local_storage().ok().flatten()) }

fn read(key: &str) -> Option<String> { local_storage().and_then(|storage| storage.get_item(key).ok().flatten()) }

fn write(key: &str, value: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(key, value);
    }
}

/// 문서 루트(`<html>`)의 속성을 갱신한다. CSS 변수 팔레트 전환(data-theme)이나
/// 접근성 언어 표시(lang)에 쓴다.
fn set_html_attr(name: &str, value: &str) {
    if let Some(root) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    {
        let _ = root.set_attribute(name, value);
    }
}

/// localStorage에 보존되고 헤더 버튼으로 토글되는 선호값.
///
/// 구현체는 저장 문자열 변환(`as_storage_value`/`from_storage_value`)과
/// 저장값이 없을 때의 기본값(`fallback`)만 정하면 된다.
pub trait PersistedToggle: Copy + Sized {
    /// localStorage 키.
    const STORAGE_KEY: &'static str;
    /// 저장값을 함께 반영할 `<html>` 속성 이름. `None`이면 저장만 한다.
    const HTML_ATTR: Option<&'static str> = None;

    /// 저장 문자열로 변환한다. `HTML_ATTR`가 있으면 그 속성 값으로도 쓰인다.
    fn as_storage_value(self) -> &'static str;

    /// 저장 문자열을 값으로 되돌린다. 알 수 없는 값이면 `None`.
    fn from_storage_value(value: &str) -> Option<Self>;

    /// 저장된 선택이 없을 때의 기본값. OS 설정 등 동적 기본값이 필요하면
    /// 구현체가 재정의한다.
    fn fallback() -> Self;

    /// 시작값: 저장된 사용자 선택 > 기본값.
    fn initial() -> Self {
        read(Self::STORAGE_KEY)
            .and_then(|value| Self::from_storage_value(&value))
            .unwrap_or_else(Self::fallback)
    }

    /// 선택을 저장하고, 해당하면 `<html>` 속성에도 반영한다.
    fn apply(self) {
        if let Some(attr) = Self::HTML_ATTR {
            set_html_attr(attr, self.as_storage_value());
        }
        write(Self::STORAGE_KEY, self.as_storage_value());
    }
}
