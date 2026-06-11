#![allow(non_snake_case)]

use crate::{models::{KpiMeasurement,
                     RecordKpiMeasurementRequest,
                     aggregation_label,
                     format_timestamp,
                     format_value},
            services::VvkikService,
            store::VvkikStore};
use chrono::{Datelike,
             Duration,
             NaiveDate};
use dioxus::prelude::*;
use std::collections::HashMap;

#[derive(Props, Clone, PartialEq)]
pub struct KpiMeasurementPanelProps {
    pub kpi_id: String,
    pub aggregation: crate::models::KpiAggregation,
    pub unit: Option<String>,
    /// 기록 유무를 폼과 공유해 현재값 입력을 잠근다.
    pub has_measurements: Signal<bool>,
    /// 집계된 현재값을 폼의 현재값 입력에도 반영한다.
    pub current_value: Signal<String>,
}

/// 기록 잔디로 보여 줄 기간(주 단위).
const GRASS_WEEKS: i64 = 20;

struct GrassCell {
    level: u8,
    title: String,
    /// 이번 주의 아직 오지 않은 날. 자리만 차지하고 그리지 않는다.
    future: bool,
}

struct GrassData {
    columns: Vec<Vec<GrassCell>>,
    recorded_days: usize,
    longest_streak: i64,
}

/// 브라우저 로컬 시간대 기준 오늘 날짜.
fn local_today() -> Option<NaiveDate> {
    let now = js_sys::Date::new_0();
    NaiveDate::from_ymd_opt(now.get_full_year() as i32, now.get_month() + 1, now.get_date())
}

/// UTC로 저장된 측정 시각을 로컬 날짜로 바꾼다. `offset_minutes`는
/// JS `getTimezoneOffset()`(UTC − 로컬, 분)이다.
fn local_date(measured_at: &str, offset_minutes: i64) -> Option<NaiveDate> {
    let utc = chrono::DateTime::parse_from_rfc3339(measured_at).ok()?;
    Some((utc.with_timezone(&chrono::Utc) - Duration::minutes(offset_minutes)).date_naive())
}

/// GitHub 잔디처럼 하루 한 칸, 농도는 그날의 기록 건수. 측정값 크기가
/// 아니라 "기록하는 행위의 꾸준함"을 보여 주는 것이 목적이다.
fn build_grass(measurements: &[KpiMeasurement]) -> Option<GrassData> {
    let today = local_today()?;
    let offset_minutes = js_sys::Date::new_0().get_timezone_offset() as i64;
    // 마지막 열이 이번 주(일요일 시작)가 되도록 시작일을 맞춘다.
    let start = today - Duration::days(today.weekday().num_days_from_sunday() as i64 + 7 * (GRASS_WEEKS - 1));

    let mut counts: HashMap<NaiveDate, u32> = HashMap::new();
    for measurement in measurements {
        if let Some(date) = local_date(&measurement.measured_at, offset_minutes)
            && date >= start
            && date <= today
        {
            *counts.entry(date).or_insert(0) += 1;
        }
    }

    let mut longest_streak = 0i64;
    let mut run = 0i64;
    let mut day = start;
    while day <= today {
        if counts.contains_key(&day) {
            run += 1;
            longest_streak = longest_streak.max(run);
        } else {
            run = 0;
        }
        day += Duration::days(1);
    }

    let columns = (0..GRASS_WEEKS)
        .map(|week| {
            (0..7)
                .map(|weekday| {
                    let date = start + Duration::days(week * 7 + weekday);
                    if date > today {
                        return GrassCell {
                            level: 0,
                            title: String::new(),
                            future: true,
                        };
                    }
                    let count = counts.get(&date).copied().unwrap_or(0);
                    let title = if count == 0 {
                        format!("{date} · 기록 없음")
                    } else {
                        format!("{date} · {count}건 기록")
                    };
                    GrassCell {
                        level: count.min(4) as u8,
                        title,
                        future: false,
                    }
                })
                .collect()
        })
        .collect();

    Some(GrassData {
        columns,
        recorded_days: counts.len(),
        longest_streak,
    })
}

/// KPI 상세 화면의 "실적 기록" 패널. 값과 함께 그날의 느낌·감상을
/// 일기처럼 남기면 백엔드가 집계 방식대로 현재값을 다시 계산하고,
/// 기록의 꾸준함은 잔디 그래프로 쌓인다.
pub fn KpiMeasurementPanel(props: KpiMeasurementPanelProps) -> Element {
    let kpi_id = use_signal(|| props.kpi_id.clone());
    let aggregation = props.aggregation;
    // 측정값 추가·삭제는 스토어를 거쳐 목록(현재값·진행률)까지 함께
    // 새로고침한다.
    let store = use_context::<VvkikStore>();
    let unit = props.unit.clone().unwrap_or_default();

    let mut measurements = use_signal(Vec::<KpiMeasurement>::new);
    let mut value_input = use_signal(String::new);
    let mut note_input = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut panel_error = use_signal(|| None::<String>);
    let mut has_measurements = props.has_measurements;
    let mut current_value = props.current_value;

    // 목록을 받아 패널 상태와 폼의 현재값 표시를 한꺼번에 갱신한다.
    let mut apply = move |list: Vec<KpiMeasurement>| {
        has_measurements.set(!list.is_empty());
        let values: Vec<f64> = list.iter().map(|measurement| measurement.value).collect();
        if let Some(aggregated) = aggregation.aggregate(&values) {
            current_value.set(format_value(aggregated));
        }
        measurements.set(list);
    };

    use_effect(move || {
        spawn(async move {
            match VvkikService::list_kpi_measurements(kpi_id.read().clone()).await {
                | Ok(list) => apply(list),
                | Err(e) => panel_error.set(Some(format!("실적 기록을 불러오지 못했습니다: {e}"))),
            }
        });
    });

    // 버튼 클릭과 메모 입력의 ⌘+Enter가 같은 경로를 쓴다.
    let mut submit = move || {
        if *busy.read() {
            return;
        }

        let raw = value_input.read().trim().to_string();
        let Ok(value) = raw.parse::<f64>() else {
            panel_error.set(Some("측정값은 숫자로 입력하세요.".to_string()));
            return;
        };
        let note = note_input.read().trim().to_string();

        spawn(async move {
            busy.set(true);
            let request = RecordKpiMeasurementRequest {
                kpi_id: kpi_id.read().clone(),
                value,
                note: (!note.is_empty()).then_some(note),
            };
            match store.record_measurement(request).await {
                | Ok(_) => {
                    value_input.set(String::new());
                    note_input.set(String::new());
                    panel_error.set(None);
                    match VvkikService::list_kpi_measurements(kpi_id.read().clone()).await {
                        | Ok(list) => apply(list),
                        | Err(e) => panel_error.set(Some(e)),
                    }
                },
                | Err(e) => panel_error.set(Some(format!("실적 기록 추가에 실패했습니다: {e}"))),
            }
            busy.set(false);
        });
    };

    let handle_delete = move |measurement_id: String| {
        if *busy.read() {
            return;
        }

        spawn(async move {
            busy.set(true);
            match store.delete_measurement(kpi_id.read().clone(), measurement_id).await {
                | Ok(()) => {
                    panel_error.set(None);
                    match VvkikService::list_kpi_measurements(kpi_id.read().clone()).await {
                        | Ok(list) => {
                            // 마지막 기록을 지우면 집계값이 없어지므로 표시도 비운다.
                            if list.is_empty() {
                                current_value.set(String::new());
                            }
                            apply(list);
                        },
                        | Err(e) => panel_error.set(Some(e)),
                    }
                },
                | Err(e) => panel_error.set(Some(format!("실적 기록 삭제에 실패했습니다: {e}"))),
            }
            busy.set(false);
        });
    };

    let grass = build_grass(&measurements.read());

    rsx! {
        div { class: "measurement-panel",
            div { class: "measurement-heading",
                label { "실적 기록" }
                span { class: "measurement-hint", "{aggregation_label(aggregation)}(으)로 현재값에 자동 집계됩니다." }
            }

            if let Some(error) = panel_error.read().clone() {
                div { class: "form-error", "{error}" }
            }

            div { class: "measurement-add",
                div { class: "measurement-add-value",
                    input {
                        r#type: "number",
                        step: "any",
                        class: "measurement-value-input",
                        placeholder: "측정값",
                        value: "{value_input}",
                        oninput: move |evt| value_input.set(evt.value())
                    }
                    if !unit.is_empty() {
                        span { class: "measurement-unit", "{unit}" }
                    }
                }
                textarea {
                    rows: "2",
                    class: "measurement-note-input",
                    placeholder: "오늘의 메모 (선택) — 느낌, 배운 점, 컨디션 등을 일기처럼 남겨 보세요.",
                    value: "{note_input}",
                    oninput: move |evt| note_input.set(evt.value()),
                    onkeydown: move |evt| {
                        let modifiers = evt.modifiers();
                        if evt.key() == Key::Enter && (modifiers.meta() || modifiers.ctrl()) {
                            submit();
                        }
                    }
                }
                div { class: "measurement-add-actions",
                    span { class: "measurement-submit-hint", "⌘+Enter로도 기록" }
                    button {
                        r#type: "button",
                        class: "btn btn-secondary",
                        disabled: *busy.read(),
                        onclick: move |_| submit(),
                        "기록"
                    }
                }
            }

            if let Some(grass) = grass {
                div { class: "measurement-grass",
                    div { class: "grass-heading",
                        span { class: "grass-title",
                            "기록 잔디 "
                            span { class: "grass-range", "최근 {GRASS_WEEKS}주" }
                        }
                        span { class: "grass-stats", "{grass.recorded_days}일 기록 · 최장 연속 {grass.longest_streak}일" }
                    }
                    div { class: "grass-grid",
                        for column in grass.columns {
                            div { class: "grass-col",
                                for cell in column {
                                    if cell.future {
                                        span { class: "grass-cell future" }
                                    } else {
                                        span { class: "grass-cell l{cell.level}", title: "{cell.title}" }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "grass-legend",
                        "적음"
                        span { class: "grass-cell l0" }
                        span { class: "grass-cell l1" }
                        span { class: "grass-cell l2" }
                        span { class: "grass-cell l3" }
                        span { class: "grass-cell l4" }
                        "많음"
                    }
                }
            }

            if measurements.read().is_empty() {
                p { class: "measurement-empty", "아직 기록이 없습니다. 첫 실적을 기록해 보세요." }
            } else {
                ul { class: "measurement-list",
                    for measurement in measurements.read().iter() {
                        {
                            let measurement_id = measurement.id.clone();
                            let value_text = format_value(measurement.value);
                            let measured_at = format_timestamp(&measurement.measured_at);
                            let note = measurement.note.clone().unwrap_or_default();
                            rsx! {
                                li { class: "measurement-row",
                                    div { class: "measurement-row-head",
                                        span { class: "measurement-value", "{value_text} {unit}" }
                                        span { class: "measurement-meta",
                                            span { class: "measurement-date", title: "{measurement.measured_at}", "{measured_at}" }
                                            button {
                                                r#type: "button",
                                                class: "btn row-btn",
                                                disabled: *busy.read(),
                                                onclick: move |_| handle_delete(measurement_id.clone()),
                                                "삭제"
                                            }
                                        }
                                    }
                                    if !note.is_empty() {
                                        p { class: "measurement-note", "{note}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
