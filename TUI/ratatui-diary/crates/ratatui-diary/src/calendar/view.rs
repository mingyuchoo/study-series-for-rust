//! 달력 화면 렌더링.

use crate::{app::Model,
            calendar::state::CalendarState};
use ratatui::{Frame,
              layout::{Alignment,
                       Constraint,
                       Direction,
                       Layout,
                       Rect},
              style::{Color,
                      Modifier,
                      Style},
              text::{Line,
                     Span},
              widgets::{Block,
                        Borders,
                        Paragraph,
                        Wrap}};

/// 달력 화면의 키바인딩 도움말 텍스트 생성
pub fn build_calendar_keybindings(_state: &CalendarState) -> String { "C-b/f/p/n:이동 | Enter:편집 | M-n/p:월 | M-]/[:년 | C-q:종료".to_string() }

pub fn render(f: &mut Frame, model: &Model) {
    // 메인 레이아웃: 수평 분할 (50:50)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // 왼쪽: 달력
            Constraint::Percentage(50), // 오른쪽: 미리보기
        ])
        .split(f.area());

    // 왼쪽: 달력 영역 (기존 레이아웃)
    let calendar_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // 헤더
            Constraint::Min(0),    // 달력
            Constraint::Length(2), // 상태바
        ])
        .split(main_chunks[0]);

    // 헤더
    let header = Paragraph::new(format!("{}년 {}월", model.calendar_state.current_year, model.calendar_state.current_month))
        .alignment(Alignment::Center)
        .style(Style::default().add_modifier(Modifier::BOLD));
    f.render_widget(header, calendar_chunks[0]);

    // 달력 그리드
    render_calendar_grid(f, calendar_chunks[1], model);

    // 상태바 - 동적 키바인딩
    let keybindings = build_calendar_keybindings(&model.calendar_state);
    let statusbar = Paragraph::new(keybindings).alignment(Alignment::Center);
    f.render_widget(statusbar, calendar_chunks[2]);

    // 오른쪽: 미리보기 영역
    let selected_date = model.calendar_state.selected_date;
    let preview_content = match model.storage.load(selected_date) {
        | Ok(content) => content,
        | Err(_) => "📝 작성된 다이어리가 없습니다.\n\nEnter를 눌러 새로 작성하세요.".to_string(),
    };

    render_preview_pane(f, main_chunks[1], &preview_content, &format!("다이어리: {}", selected_date));
}

fn render_preview_pane(f: &mut Frame, area: Rect, content: &str, title: &str) {
    let text = Paragraph::new(content)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap {
            trim: false,
        })
        .style(Style::default());

    f.render_widget(text, area);
}

fn render_calendar_grid(f: &mut Frame, area: Rect, model: &Model) {
    use chrono::{Datelike,
                 NaiveDate};

    let year = model.calendar_state.current_year;
    let month = model.calendar_state.current_month;

    // 요일 헤더
    let weekdays = ["일", "월", "화", "수", "목", "금", "토"];
    let mut lines = vec![Line::from(
        weekdays
            .iter()
            .map(|&day| Span::styled(format!("{:^4}", day), Style::default()))
            .collect::<Vec<_>>(),
    )];

    // 월의 첫날
    let first_day = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let weekday = first_day.weekday().num_days_from_sunday() as usize;

    // 달력 생성
    let days_in_month = first_day
        .with_month(month + 1)
        .unwrap_or_else(|| first_day.with_year(year + 1).unwrap().with_month(1).unwrap())
        .pred_opt()
        .unwrap()
        .day();

    let mut week = vec![Span::raw("    "); 7];
    let mut day = 1;

    // 첫 주 빈 칸 채우기
    for slot in week.iter_mut().take(7).skip(weekday) {
        let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
        *slot = format_day(day, date, model);
        day += 1;
    }
    lines.push(Line::from(week.clone()));

    // 나머지 주
    while day <= days_in_month {
        week = vec![Span::raw("    "); 7];
        for slot in week.iter_mut().take(7) {
            if day <= days_in_month {
                let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
                *slot = format_day(day, date, model);
                day += 1;
            }
        }
        lines.push(Line::from(week.clone()));
    }

    let calendar = Paragraph::new(lines).block(Block::default().borders(Borders::NONE));
    f.render_widget(calendar, area);
}

fn format_day(day: u32, date: chrono::NaiveDate, model: &Model) -> Span<'static> {
    let has_entry = model.diary_entries.entries.contains(&date);
    let is_selected = date == model.calendar_state.selected_date;
    let is_today = date == chrono::Local::now().date_naive();

    let mut style = Style::default();

    if has_entry {
        style = style.fg(Color::Green).add_modifier(Modifier::BOLD);
    }
    if is_selected {
        style = style.bg(Color::Blue);
    }
    if is_today {
        style = style.add_modifier(Modifier::UNDERLINED);
    }

    let marker = if has_entry { "●" } else { " " };
    Span::styled(format!("{:>2}{} ", day, marker), style)
}
