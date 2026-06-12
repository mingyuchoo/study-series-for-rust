//! 퀵 기록 흐름: 측정값 한 건을 기록하고 토스트(+실행 취소)로 알린다.
//! busy 가드·스토어 호출·피드백이 한 곳에 모여 있어, 어느 화면이든
//! "기록한다"는 같은 경로를 쓴다.

use super::record_toast::{RecordToastHost,
                          use_record_toast};
use crate::{i18n::{Lang,
                   use_lang},
            models::{IkikItem,
                     KpiAggregation,
                     RecordKpiMeasurementRequest,
                     format_value},
            store::IkikStore};
use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct QuickRecord {
    pub toast: RecordToastHost,
    busy: Signal<bool>,
    store: IkikStore,
    lang: Signal<Lang>,
}

pub fn use_quick_record() -> QuickRecord {
    QuickRecord {
        toast: use_record_toast(),
        busy: use_signal(|| false),
        store: use_context::<IkikStore>(),
        lang: use_lang(),
    }
}

impl QuickRecord {
    pub fn is_busy(&self) -> bool { *self.busy.read() }

    /// 측정값 한 건을 기록한다. 성공하면 실행 취소가 달린 토스트를
    /// 띄우고 true, 실패하면 오류 토스트를 띄우고 false를 돌려준다.
    pub async fn record(mut self, item: &IkikItem, value: f64) -> bool {
        if *self.busy.peek() {
            return false;
        }

        self.busy.set(true);
        let request = RecordKpiMeasurementRequest {
            kpi_id: item.id.clone(),
            value,
            note: None,
        };
        let result = self.store.record_measurement(request).await;
        self.busy.set(false);

        match result {
            | Ok(measurement) => {
                let unit = item.unit.clone().unwrap_or_default();
                let amount = if item.aggregation == KpiAggregation::Sum {
                    format!("+{}", format_value(value))
                } else {
                    format_value(value)
                };
                let message = self.lang.peek().recorded_toast(&item.title, &amount, &unit);
                self.toast.show(message, Some((item.id.clone(), measurement.id)));
                true
            },
            | Err(e) => {
                self.toast.show(self.lang.peek().err_record(&e), None);
                false
            },
        }
    }

    /// 실행 취소: 방금 추가한 측정값을 지우고 목록을 새로고침한다.
    pub fn undo(self, (kpi_id, measurement_id): (String, String)) {
        spawn(async move {
            if let Err(e) = self.store.delete_measurement(kpi_id, measurement_id).await {
                self.toast.show(self.lang.peek().err_undo(&e), None);
            }
        });
    }
}
