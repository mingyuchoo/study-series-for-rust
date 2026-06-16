//! Emacs 스타일 키바인딩을 위한 메시지 타입
//!
//! 모드리스(modeless) 에디터로, Ctrl+/Alt+ 조합으로 명령을 실행하며
//! 별도의 모드 전환 없이 항상 문자 입력이 가능합니다.

use chrono::NaiveDate;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    Quit,
    DismissError,

    // 에디터 - 네비게이션
    EditorMoveLeft,
    EditorMoveRight,
    EditorMoveUp,
    EditorMoveDown,
    EditorWordNext,
    EditorWordPrev,

    // 에디터 - 점프
    EditorGotoDocStart,
    EditorGotoDocEnd,
    EditorGotoLineStart,
    EditorGotoLineEnd,
    EditorExitSubMode,

    // 에디터 - 문자 입력 (항상 활성)
    EditorInsertChar(char),
    EditorBackspace,
    EditorNewLine,
    EditorOpenLine,

    // 에디터 - Selection
    EditorToggleSelection,
    EditorSelectLine,

    // 에디터 - 편집
    EditorDelete,
    EditorDeleteForward,
    EditorKillLine,
    EditorYank,
    EditorPaste,

    // 에디터 - Undo/Redo
    EditorUndo,
    EditorRedo,

    // 에디터 - 검색
    EditorEnterSearchMode,
    EditorSearchChar(char),
    EditorSearchBackspace,
    EditorSearchNext,
    EditorSearchPrev,
    EditorExecuteSearch,

    // 에디터 - Ctrl+X 프리픽스
    EditorEnterCtrlXMode,
    EditorCtrlXSave,
    EditorCtrlXBack,

    // 달력
    CalendarMoveLeft,
    CalendarMoveRight,
    CalendarMoveUp,
    CalendarMoveDown,
    CalendarSelectDate,
    CalendarNextMonth,
    CalendarPrevMonth,
    CalendarNextYear,
    CalendarPrevYear,

    // 파일 I/O 결과
    LoadDiarySuccess(NaiveDate, String),
    LoadDiaryFailed(String),
    SaveDiarySuccess,
    SaveDiaryFailed(String),
    DeleteDiarySuccess(NaiveDate),
    RefreshIndex(HashSet<NaiveDate>),
}
