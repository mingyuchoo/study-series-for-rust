//! 화면 컴포넌트와 Tauri 호출 사이의 데이터 오케스트레이션.
//!
//! 컴포넌트는 시그널을 읽어 그리기만 하고, 목록 새로고침·로딩·오류
//! 처리는 전부 여기서 담당한다. 시그널은 모두 Copy라 스토어 자체를
//! 값으로 들고 다닐 수 있다.

use crate::{models::{CreateItemRequest,
                     ItemKind,
                     UpdateItemRequest,
                     VvkikItem},
            services::VvkikService};
use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct VvkikStore {
    pub items: Signal<Vec<VvkikItem>>,
    pub loading: Signal<bool>,
    pub error: Signal<Option<String>>,
    pub search_query: Signal<String>,
}

/// 스토어를 만들고 첫 목록을 불러온다. `App` 최상단에서 한 번 호출한다.
pub fn use_vvkik_store() -> VvkikStore {
    let store = VvkikStore {
        items: use_signal(Vec::new),
        loading: use_signal(|| false),
        error: use_signal(|| None),
        search_query: use_signal(String::new),
    };

    use_effect(move || {
        spawn(async move {
            store.reload("VVKIK 항목을 불러오지 못했습니다").await;
        });
    });

    store
}

impl VvkikStore {
    pub fn is_busy(&self) -> bool { *self.loading.read() }

    pub fn clear_error(mut self) { self.error.set(None); }

    pub fn set_error(mut self, message: String) { self.error.set(Some(message)); }

    /// 같은 단계·같은 상위 항목을 가진 형제들 뒤에 붙도록 정렬값을
    /// 자동 부여한다.
    pub fn next_position(&self, kind: ItemKind, parent_id: Option<&str>) -> i64 {
        self.items
            .read()
            .iter()
            .filter(|item| item.kind == kind && item.parent_id.as_deref() == parent_id)
            .map(|item| item.position)
            .max()
            .map_or(0, |max| max + 1)
    }

    async fn fetch(query: String) -> Result<Vec<VvkikItem>, String> {
        let query = query.trim().to_string();

        if query.is_empty() {
            VvkikService::list_items().await
        } else {
            VvkikService::search_items(query).await
        }
    }

    /// 현재 검색어 기준으로 목록을 다시 불러온다.
    async fn reload(mut self, failure_label: &str) -> bool {
        self.loading.set(true);
        let query = self.search_query.read().clone();
        let succeeded = match Self::fetch(query).await {
            | Ok(items) => {
                self.items.set(items);
                self.error.set(None);
                true
            },
            | Err(e) => {
                self.error.set(Some(format!("{failure_label}: {e}")));
                false
            },
        };
        self.loading.set(false);
        succeeded
    }

    pub async fn search(self) { self.reload("검색에 실패했습니다").await; }

    /// 폼 바깥에서 데이터가 바뀌었을 때(측정 기록 추가·삭제 등) 목록을
    /// 현재 검색어 기준으로 다시 불러온다.
    pub async fn refresh(self) { self.reload("VVKIK 항목을 불러오지 못했습니다").await; }

    pub async fn clear_search(mut self) {
        self.search_query.set(String::new());
        self.reload("VVKIK 항목을 불러오지 못했습니다").await;
    }

    /// 성공하면 true를 돌려주어 호출한 화면이 닫기 등 후속 동작을
    /// 결정할 수 있게 한다.
    pub async fn create(mut self, request: CreateItemRequest) -> bool {
        if self.is_busy() {
            return false;
        }

        self.loading.set(true);
        let result = VvkikService::create_item(request).await;
        self.loading.set(false);

        match result {
            | Ok(_) => {
                self.reload("목록을 새로고침하지 못했습니다").await;
                true
            },
            | Err(e) => {
                self.error.set(Some(format!("항목 추가에 실패했습니다: {e}")));
                false
            },
        }
    }

    pub async fn update(mut self, request: UpdateItemRequest) -> bool {
        if self.is_busy() {
            return false;
        }

        self.loading.set(true);
        let result = VvkikService::update_item(request).await;
        self.loading.set(false);

        match result {
            | Ok(_) => {
                self.reload("목록을 새로고침하지 못했습니다").await;
                true
            },
            | Err(e) => {
                self.error.set(Some(format!("항목 수정에 실패했습니다: {e}")));
                false
            },
        }
    }

    pub async fn delete(mut self, id: String) {
        if self.is_busy() {
            return;
        }

        self.loading.set(true);
        let result = VvkikService::delete_item(id).await;
        self.loading.set(false);

        match result {
            | Ok(_) => {
                self.reload("목록을 새로고침하지 못했습니다").await;
            },
            | Err(e) => {
                self.error.set(Some(format!("항목 삭제에 실패했습니다: {e}")));
            },
        }
    }
}
