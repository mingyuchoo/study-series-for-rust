//! 최상위 렌더링 디스패처 + 공용 오버레이(에러 팝업).

use crate::{app::{Model,
                  Screen},
            calendar,
            editor};
use ratatui::{Frame,
              layout::{Constraint,
                       Direction,
                       Layout,
                       Rect},
              style::{Color,
                      Style},
              widgets::{Block,
                        Borders,
                        Clear,
                        Paragraph,
                        Wrap}};

pub fn view(f: &mut Frame, model: &Model) {
    match model.screen {
        | Screen::Calendar => calendar::view::render(f, model),
        | Screen::Editor => editor::view::render(f, model),
    }

    // 에러 팝업
    if model.show_error_popup {
        render_error_popup(f, model);
    }
}

fn render_error_popup(f: &mut Frame, model: &Model) {
    let area = centered_rect(60, 20, f.size());

    let error_msg = model.error_message.as_deref().unwrap_or("알 수 없는 에러");
    let popup = Paragraph::new(error_msg)
        .block(
            Block::default()
                .title("Error")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        )
        .style(Style::default().bg(Color::Black))
        .wrap(Wrap {
            trim: true,
        });

    f.render_widget(Clear, area);
    f.render_widget(popup, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
