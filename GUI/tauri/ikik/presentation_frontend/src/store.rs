//! 화면 컴포넌트와 Tauri 호출 사이의 데이터 오케스트레이션.
//!
//! 컴포넌트는 시그널을 읽어 그리기만 하고, 목록 새로고침·로딩·오류
//! 처리는 전부 여기서 담당한다. 시그널은 모두 Copy라 스토어 자체를
//! 값으로 들고 다닐 수 있다.
//!
//! 컴포넌트는 `IkikService`를 직접 부르지 않는다 — 화면-국소 데이터
//! (측정 기록·변경 이력·잔디)도 아래의 load_* 메서드를 거쳐, 데이터
//! 접근 경로가 스토어 하나로 모인다.

use crate::{i18n::{Lang,
                   use_lang},
            models::{CreateItemRequest,
                     IkikItem,
                     ItemKind,
                     ItemRevision,
                     KpiMeasurement,
                     RecordKpiMeasurementRequest,
                     UpdateItemRequest},
            services::IkikService};
use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct IkikStore {
    pub items: Signal<Vec<IkikItem>>,
    pub loading: Signal<bool>,
    pub error: Signal<Option<String>>,
    pub search_query: Signal<String>,
    /// 오류 메시지를 현재 언어로 만들기 위해 들고 다닌다.
    lang: Signal<Lang>,
}

/// 스토어를 만들고 첫 목록을 불러온다. `App` 최상단에서 언어 컨텍스트
/// 제공 후 한 번 호출한다. 컨텍스트로도 제공하므로 하위 컴포넌트는
/// prop 없이 `use_context::<IkikStore>()`로 데이터 갱신을 요청할 수 있다.
pub fn use_ikik_store() -> IkikStore {
    let store = IkikStore {
        items: use_signal(Vec::new),
        loading: use_signal(|| false),
        error: use_signal(|| None),
        search_query: use_signal(String::new),
        lang: use_lang(),
    };
    use_context_provider(|| store);

    use_effect(move || {
        spawn(async move {
            let label = store.lang().err_load_items();
            store.reload(label).await;
        });
    });

    store
}

impl IkikStore {
    fn lang(&self) -> Lang { *self.lang.peek() }

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

    async fn fetch(query: String) -> Result<Vec<IkikItem>, String> {
        let query = query.trim().to_string();

        if query.is_empty() {
            IkikService::list_items().await
        } else {
            IkikService::search_items(query).await
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

    pub async fn search(self) { self.reload(self.lang().err_search()).await; }

    /// 폼 바깥에서 데이터가 바뀌었을 때(측정 기록 추가·삭제 등) 목록을
    /// 현재 검색어 기준으로 다시 불러온다.
    pub async fn refresh(self) { self.reload(self.lang().err_load_items()).await; }

    pub async fn clear_search(mut self) {
        self.search_query.set(String::new());
        self.reload(self.lang().err_load_items()).await;
    }

    /// 변이 한 건의 공통 경로: busy 가드 → 호출 → 성공 시 목록
    /// 새로고침 / 실패 시 오류 표시. 변이 후에는 반드시 목록을 다시
    /// 불러온다는 규칙이 여기 한 곳에 산다. 성공하면 true를 돌려주어
    /// 호출한 화면이 닫기 등 후속 동작을 결정할 수 있게 한다.
    async fn mutate<T>(mut self, call: impl Future<Output = Result<T, String>>, error_message: impl FnOnce(Lang, &str) -> String) -> bool {
        if self.is_busy() {
            return false;
        }

        self.loading.set(true);
        let result = call.await;
        self.loading.set(false);

        match result {
            | Ok(_) => {
                self.reload(self.lang().err_refresh_list()).await;
                true
            },
            | Err(e) => {
                self.error.set(Some(error_message(self.lang(), &e)));
                false
            },
        }
    }

    pub async fn create(self, request: CreateItemRequest) -> bool { self.mutate(IkikService::create_item(request), |lang, e| lang.err_create_item(e)).await }

    pub async fn update(self, request: UpdateItemRequest) -> bool { self.mutate(IkikService::update_item(request), |lang, e| lang.err_update_item(e)).await }

    /// 트리 드래그: 항목을 새 상위 항목 아래 맨 뒤로 옮긴다. 요청
    /// 조립(어떤 필드를 비우는지)이 화면이 아니라 여기서 결정된다.
    pub async fn reparent(self, item: IkikItem, new_parent: IkikItem) -> bool {
        let position = self.next_position(item.kind, Some(new_parent.id.as_str()));
        let request = UpdateItemRequest {
            id: item.id,
            kind: None,
            parent_id: Some(Some(new_parent.id)),
            title: None,
            description: None,
            target_value: None,
            current_value: None,
            unit: None,
            position: Some(position),
            status: None,
            aggregation: None,
            due_date: None,
        };
        self.update(request).await
    }

    /// Key Performance Indicator 하나의 측정 기록(최신순). 화면-국소
    /// 데이터라 시그널에 담지 않고 호출자에게 돌려준다.
    pub async fn load_measurements(&self, kpi_id: String) -> Result<Vec<KpiMeasurement>, String> { IkikService::list_kpi_measurements(kpi_id).await }

    /// 모든 측정 기록(최신순). 대시보드의 기록 잔디가 쓴다.
    pub async fn load_all_measurements(&self) -> Result<Vec<KpiMeasurement>, String> { IkikService::list_all_kpi_measurements().await }

    /// 항목 정의 변경 이력(최신순).
    pub async fn load_revisions(&self, item_id: String) -> Result<Vec<ItemRevision>, String> { IkikService::list_item_revisions(item_id).await }

    /// 측정값을 기록하고 목록(현재값·진행률)을 새로고침한다. 오류
    /// 표시는 호출한 화면이 맡으므로 스토어 오류를 건드리지 않는다.
    pub async fn record_measurement(self, request: RecordKpiMeasurementRequest) -> Result<KpiMeasurement, String> {
        let measurement = IkikService::record_kpi_measurement(request).await?;
        self.refresh().await;
        Ok(measurement)
    }

    /// 측정값 하나를 지우고 목록을 새로고침한다.
    pub async fn delete_measurement(self, kpi_id: String, measurement_id: String) -> Result<(), String> {
        IkikService::delete_kpi_measurement(kpi_id, measurement_id).await?;
        self.refresh().await;
        Ok(())
    }

    pub async fn delete(self, id: String) { self.mutate(IkikService::delete_item(id), |lang, e| lang.err_delete_item(e)).await; }
}
