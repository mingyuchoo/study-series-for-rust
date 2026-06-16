//! 에디터 화면의 키 입력 → 메시지 매핑 (Emacs 스타일).

use crate::{editor::state::{EditorState,
                            EditorSubMode},
            message::Msg};
use crossterm::event::{KeyCode,
                       KeyEvent,
                       KeyModifiers};

pub fn handle_key(key: KeyEvent, state: &EditorState) -> Option<Msg> {
    // 서브모드 처리 우선
    match &state.submode {
        | Some(EditorSubMode::CtrlX) => {
            return match key.code {
                // Ctrl+S: 저장
                | KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::EditorCtrlXSave),
                // Ctrl+C: 뒤로 (달력)
                | KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::EditorCtrlXBack),
                // Esc: 취소
                | KeyCode::Esc => Some(Msg::EditorExitSubMode),
                | _ => Some(Msg::EditorExitSubMode),
            };
        },
        | Some(EditorSubMode::Search) => {
            return match key.code {
                | KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::EditorSearchNext),
                | KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::EditorSearchPrev),
                | KeyCode::Char(c) => Some(Msg::EditorSearchChar(c)),
                | KeyCode::Enter => Some(Msg::EditorExecuteSearch),
                | KeyCode::Esc => Some(Msg::EditorExitSubMode),
                | KeyCode::Backspace => Some(Msg::EditorSearchBackspace),
                | _ => None,
            };
        },
        | None => {},
    }

    let mods = key.modifiers;

    match key.code {
        // === Ctrl 조합 ===
        // Ctrl+F/B/N/P: 커서 이동
        | KeyCode::Char('f') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::EditorMoveRight),
        | KeyCode::Char('b') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::EditorMoveLeft),
        | KeyCode::Char('n') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::EditorMoveDown),
        | KeyCode::Char('p') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::EditorMoveUp),

        // Ctrl+A / Ctrl+E: 줄 시작/끝
        | KeyCode::Char('a') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::EditorGotoLineStart),
        | KeyCode::Char('e') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::EditorGotoLineEnd),

        // Ctrl+H: backspace
        | KeyCode::Char('h') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::EditorBackspace),

        // Ctrl+O: 커서 위치에 새 줄 열기 (open-line)
        | KeyCode::Char('o') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::EditorOpenLine),

        // Ctrl+D: 커서 앞 문자 삭제
        | KeyCode::Char('d') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::EditorDeleteForward),

        // Ctrl+K: kill-line (줄 끝까지 삭제)
        | KeyCode::Char('k') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::EditorKillLine),

        // Ctrl+Space: 마크 설정 (선택 토글)
        | KeyCode::Char(' ') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::EditorToggleSelection),

        // Ctrl+W: 영역 잘라내기
        | KeyCode::Char('w') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::EditorDelete),

        // Ctrl+Y: 붙여넣기 (yank)
        | KeyCode::Char('y') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::EditorPaste),

        // Ctrl+Z: 실행취소
        | KeyCode::Char('z') if mods == KeyModifiers::CONTROL => Some(Msg::EditorUndo),

        // Ctrl+Shift+Z: 다시실행
        | KeyCode::Char('Z') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::EditorRedo),

        // Ctrl+S: 검색 (서브모드 진입)
        | KeyCode::Char('s') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::EditorEnterSearchMode),

        // Ctrl+X: 프리픽스 모드 진입
        | KeyCode::Char('x') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::EditorEnterCtrlXMode),

        // Ctrl+Q: 종료
        | KeyCode::Char('q') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::Quit),

        // === Alt 조합 ===
        // Alt+F / Alt+B: 단어 이동
        | KeyCode::Char('f') if mods.contains(KeyModifiers::ALT) => Some(Msg::EditorWordNext),
        | KeyCode::Char('b') if mods.contains(KeyModifiers::ALT) => Some(Msg::EditorWordPrev),

        // Alt+< / Alt+>: 문서 시작/끝
        | KeyCode::Char('<') if mods.contains(KeyModifiers::ALT) => Some(Msg::EditorGotoDocStart),
        | KeyCode::Char('>') if mods.contains(KeyModifiers::ALT) => Some(Msg::EditorGotoDocEnd),

        // Alt+W: 영역 복사
        | KeyCode::Char('w') if mods.contains(KeyModifiers::ALT) => Some(Msg::EditorYank),

        // === 일반 키 ===
        | KeyCode::Backspace => Some(Msg::EditorBackspace),
        | KeyCode::Enter => Some(Msg::EditorNewLine),
        | KeyCode::Char(c) => Some(Msg::EditorInsertChar(c)),

        | _ => None,
    }
}
