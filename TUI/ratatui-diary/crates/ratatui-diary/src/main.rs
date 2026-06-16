use crossterm::{event::{self,
                        Event},
                execute,
                terminal::{EnterAlternateScreen,
                           LeaveAlternateScreen,
                           disable_raw_mode,
                           enable_raw_mode}};
use ratatui::{Terminal,
              backend::CrosstermBackend,
              prelude::*};
use ratatui_diary::{Model,
                    Msg,
                    app,
                    storage::Storage};
use std::{io,
          time::Duration};

fn main() -> std::io::Result<()> {
    // Storage 초기화
    let storage = Storage::new()?;
    let entries = storage.scan_entries()?;

    // Model 초기화
    let mut model = Model::new(entries, storage);

    // Terminal 초기화
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // 이벤트 루프
    let result = run_app(&mut terminal, &mut model);

    // Terminal 복원
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, model: &mut Model) -> std::io::Result<()> {
    loop {
        // 렌더링
        terminal.draw(|f| app::view(f, model))?;

        // 이벤트 처리
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && let Some(msg) = app::handle_key(key, model)
        {
            // Quit 메시지 처리
            if matches!(msg, Msg::Quit) {
                break;
            }

            // Update 호출
            if let Some(cmd) = app::update(model, msg) {
                app::execute_command(cmd, model)?;
            }
        }
    }

    Ok(())
}
