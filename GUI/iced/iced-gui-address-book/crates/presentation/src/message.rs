//! GUI 가 처리하는 메시지(이벤트) 정의.

use crate::{i18n::Language,
            theme::ThemeMode};
use application::error::AppError;
use domain::entities::Address;

/// 사용자 입력 및 비동기 작업 결과를 나타내는 메시지.
///
/// `AppError` 가 `Clone` 이므로 비동기 결과를 타입 그대로 운반할 수 있다(에러를
/// 화면에 표시하기 위함).
#[derive(Debug, Clone)]
pub enum Message {
    NameChanged(String),
    PhoneChanged(String),
    EmailChanged(String),
    AddressChanged(String),
    CreateAddress,
    DeleteAddress(i64),
    EditAddress(Address),
    UpdateAddress,
    CancelEdit,
    LoadAddresses,
    AddressesLoaded(Result<Vec<Address>, AppError>),
    TabPressed {
        shift: bool,
    },
    /// 한국어/영어 토글 버튼 선택.
    LanguageSelected(Language),
    /// 시스템/라이트/다크 토글 버튼 선택.
    ThemeSelected(ThemeMode),
}
