#![allow(non_snake_case)]

use crate::{i18n::{Lang,
                   use_lang},
            models::deadline::local_today};
use chrono::{Datelike,
             Duration,
             NaiveDate};
use dioxus::prelude::*;
use std::collections::HashMap;

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
    this_week_days: usize,
}

/// UTC로 저장된 측정 시각을 로컬 날짜로 바꾼다. `offset_minutes`는
/// JS `getTimezoneOffset()`(UTC − 로컬, 분)이다.
fn local_date(timestamp: &str, offset_minutes: i64) -> Option<NaiveDate> {
    let utc = chrono::DateTime::parse_from_rfc3339(timestamp).ok()?;
    Some((utc.with_timezone(&chrono::Utc) - Duration::minutes(offset_minutes)).date_naive())
}

/// GitHub 잔디처럼 하루 한 칸, 농도는 그날의 기록 건수. 측정값 크기가
/// 아니라 "기록하는 행위의 꾸준함"을 보여 주는 것이 목적이다.
fn build_grass(timestamps: &[String], t: Lang) -> Option<GrassData> {
    let today = local_today()?;
    let offset_minutes = js_sys::Date::new_0().get_timezone_offset() as i64;
    // 마지막 열이 이번 주(일요일 시작)가 되도록 시작일을 맞춘다.
    let week_start = today - Duration::days(today.weekday().num_days_from_sunday() as i64);
    let start = week_start - Duration::days(7 * (GRASS_WEEKS - 1));

    let mut counts: HashMap<NaiveDate, u32> = HashMap::new();
    for timestamp in timestamps {
        if let Some(date) = local_date(timestamp, offset_minutes)
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

    let this_week_days = counts.keys().filter(|date| **date >= week_start).count();

    let columns = (0 .. GRASS_WEEKS)
        .map(|week| {
            (0 .. 7)
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
                    let date_text = date.to_string();
                    let title = if count == 0 {
                        t.grass_no_record(&date_text)
                    } else {
                        t.grass_count(&date_text, count)
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
        this_week_days,
    })
}

#[derive(Props, Clone, PartialEq)]
pub struct RecordGrassProps {
    /// RFC3339 측정 시각 목록. 어느 Key Performance Indicator의 기록인지는
    /// 구분하지 않는다 — 잔디는 기록하는 행위 자체를 본다.
    pub timestamps: Vec<String>,
    /// 머리글의 범위 설명(예: "전체 Key Performance Indicator"). 없으면 기간만
    /// 보여 준다.
    #[props(default)]
    pub scope: Option<String>,
}

/// 기록의 꾸준함을 보여 주는 잔디 그래프. Key Performance Indicator 상세 패널과
/// 대시보드가 같은 구현을 공유한다.
pub fn RecordGrass(props: RecordGrassProps) -> Element {
    let t = *use_lang().read();
    let Some(grass) = build_grass(&props.timestamps, t) else {
        return rsx! {};
    };
    let range = t.grass_range(props.scope.as_deref(), GRASS_WEEKS);

    rsx! {
        div { class: "measurement-grass",
            div { class: "grass-heading",
                span { class: "grass-title",
                    {t.grass_title()}
                    " "
                    span { class: "grass-range", "{range}" }
                }
                span { class: "grass-stats",
                    {t.grass_stats(grass.recorded_days, grass.longest_streak, grass.this_week_days)}
                }
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
                {t.grass_less()}
                span { class: "grass-cell l0" }
                span { class: "grass-cell l1" }
                span { class: "grass-cell l2" }
                span { class: "grass-cell l3" }
                span { class: "grass-cell l4" }
                {t.grass_more()}
            }
        }
    }
}
