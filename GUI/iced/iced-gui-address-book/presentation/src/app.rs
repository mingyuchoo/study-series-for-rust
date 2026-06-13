//! 애플리케이션 상태와 합성 루트(composition root).

use crate::message::Message;
use application::usecases::AddressUseCases;
use domain::entities::Address;
use iced::Task;
use infrastructure::database::SqliteAddressRepository;
use std::sync::Arc;

/// SQLite 데이터베이스 파일 경로.
const DB_PATH: &str = "addresses.db";

/// 주소록 GUI 의 전체 상태.
pub struct AddressBook {
    /// 주입된 use case 모음(저장소 추상화 뒤에 있음).
    pub(crate) usecases: Arc<AddressUseCases>,
    /// 현재 화면에 표시 중인 주소 목록.
    pub(crate) addresses: Vec<Address>,
    pub(crate) name_input: String,
    pub(crate) phone_input: String,
    pub(crate) email_input: String,
    pub(crate) address_input: String,
    /// 편집 중인 주소의 `id`. `None` 이면 "추가" 모드.
    pub(crate) editing_id: Option<i64>,
    /// 마지막으로 발생한 에러 메시지(있으면 화면에 배너로 표시).
    pub(crate) error_message: Option<String>,
}

impl AddressBook {
    /// 의존성 그래프를 구성하고 초기 목록 로드 태스크와 함께 상태를 만든다.
    ///
    /// 저장소 초기화 실패는 스토리지 없는 주소록이 무의미하므로 명확한 메시지와
    /// 함께 fail-fast 한다(합성 루트의 정당한 종료 지점).
    pub(crate) fn new() -> (Self, Task<Message>) {
        let repository = Arc::new(SqliteAddressRepository::new(DB_PATH).expect("failed to initialize database at addresses.db"));
        let usecases = Arc::new(AddressUseCases::new(repository));

        (
            Self {
                usecases,
                addresses: Vec::new(),
                name_input: String::new(),
                phone_input: String::new(),
                email_input: String::new(),
                address_input: String::new(),
                editing_id: None,
                error_message: None,
            },
            Task::done(Message::LoadAddresses),
        )
    }

    /// 입력 폼의 네 필드를 모두 비운다.
    ///
    /// 생성/수정/취소 경로에서 공통으로 쓰여 중복을 제거한다.
    pub(crate) fn clear_inputs(&mut self) {
        self.name_input.clear();
        self.phone_input.clear();
        self.email_input.clear();
        self.address_input.clear();
    }
}
