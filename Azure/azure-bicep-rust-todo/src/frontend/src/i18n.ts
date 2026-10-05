export type Language = "ko" | "en";

const korean = {
  Language: "언어",
  Theme: "테마",
  System: "시스템",
  Light: "라이트",
  Dark: "다크",
  "Skip to tasks": "할 일로 건너뛰기",
  "Main navigation": "주 메뉴",
  "Todo home": "Todo 홈",
  "My workspace": "내 작업 공간",
  "New task": "새 할 일",
  "A little structure. A lot of clarity.": "조금 더 정돈된, 한결 가벼운 하루.",
  "Make room for": "소중한 일에",
  "what matters.": "집중할 여유.",
  "Big plans start with small steps. Keep your tasks in one place and move through your day with a clear mind.":
    "큰 계획도 작은 실천에서 시작됩니다. 할 일을 한곳에 모으고, 가벼운 마음으로 하루를 보내세요.",
  "Let’s get things done": "하나씩 시작해 볼까요",
  "Your workspace": "나의 작업 공간",
  "A clearer picture.": "한눈에 보는 나의 하루.",
  "Every small step adds up.": "작은 실천이 모여 변화를 만듭니다.",
  "Total tasks": "전체 할 일",
  "All tasks": "전체",
  Active: "진행 중",
  Completed: "완료",
  "Overall progress": "전체 진행률",
  "Task completion": "할 일 완료율",
  "ONE THING AT A TIME": "한 번에 하나씩",
  "Your tasks, simplified.": "할 일 관리, 더 간편하게.",
  "A place for everything on your mind.": "머릿속 할 일을 모두 담아 두세요.",
  "Your tasks couldn’t be loaded. Please try refreshing.":
    "할 일을 불러오지 못했습니다. 새로고침해 주세요.",
  "That change couldn’t be saved. Your input is still here — please try again.":
    "변경 사항을 저장하지 못했습니다. 입력한 내용은 유지되니 다시 시도해 주세요.",
  "Task added. A little more clarity for your day.":
    "할 일을 추가했습니다. 하루가 조금 더 정돈되었네요.",
  "Task updated.": "할 일을 수정했습니다.",
  "Task marked active.": "할 일을 진행 중으로 변경했습니다.",
  "Task completed. Nicely done.": "할 일을 완료했습니다. 수고하셨어요!",
  "Task deleted.": "할 일을 삭제했습니다.",
  "Refresh tasks": "할 일 새로고침",
  "Add a new task": "새 할 일 추가",
  "Get it out of your head and onto your list.":
    "기억하는 대신 목록에 적어 두세요.",
  "Task title": "할 일 제목",
  "What needs to get done?": "어떤 일을 해야 하나요?",
  Description: "설명",
  "(optional)": "(선택 사항)",
  "Add a little more detail…": "자세한 내용을 적어 주세요…",
  "Saving…": "저장 중…",
  "Add task": "할 일 추가",
  "One small step is a great start.": "작은 한 걸음이면 충분한 시작입니다.",
  "Filter tasks": "할 일 필터",
  "Search tasks": "할 일 검색",
  "Search your tasks…": "할 일을 검색하세요…",
  "Loading your tasks…": "할 일을 불러오는 중…",
  "Making a little room for clarity.": "하루를 정돈할 준비를 하고 있어요.",
  "Let’s try that again.": "다시 시도해 볼까요?",
  "Refresh your tasks to reconnect to your workspace.":
    "할 일을 새로고침하여 작업 공간에 다시 연결하세요.",
  "No matching tasks.": "검색 결과가 없습니다.",
  "A fresh start.": "새롭게 시작해 보세요.",
  "All caught up.": "모두 끝냈어요!",
  "Small steps start here.": "작은 실천은 여기서 시작됩니다.",
  "Try a different search or clear your filters.":
    "다른 검색어를 입력하거나 필터를 초기화하세요.",
  "Add your first task and make space for what matters.":
    "첫 할 일을 추가하고 소중한 일에 집중해 보세요.",
  "No active tasks. Take a moment to enjoy it.":
    "진행 중인 할 일이 없습니다. 잠시 여유를 즐기세요.",
  "Completed tasks will appear here.": "완료한 할 일이 여기에 표시됩니다.",
  "Clear filters": "필터 초기화",
  "Mark {title} incomplete": "{title}: 진행 중으로 변경",
  "Mark {title} complete": "{title}: 완료로 변경",
  "Added {date}": "추가일 {date}",
  "Edit {title}": "{title} 수정",
  "Delete {title}": "{title} 삭제",
  "Edit task": "할 일 수정",
  "Delete task": "할 일 삭제",
  "Save changes": "변경 사항 저장",
  Cancel: "취소",
  "{visible} of {total} tasks": "전체 {total}개 중 {visible}개",
  "Your personal task list": "나만의 할 일 목록",
  "Less clutter. More focus.": "더 간결하게, 더 집중해서.",
  "A little progress, every day.": "매일 조금씩, 앞으로.",
  "You don’t have to do it all at once. Just take the next step.":
    "한 번에 다 하지 않아도 괜찮아요. 다음 한 걸음만 내디뎌 보세요.",
  "Take the next step": "다음 한 걸음 시작하기",
  "A simpler space to get things done.": "할 일을 위한 더 간편한 공간.",
  "Back to your tasks": "할 일로 돌아가기",
  "Make time for what matters.": "소중한 일을 위한 시간을 만드세요.",
  "Your personal workspace": "나만의 작업 공간",
  "todo. — Your personal workspace": "todo. — 나만의 작업 공간",
  "A simpler space to organize your tasks, clear your mind, and make room for what matters.":
    "할 일을 정리하고, 마음을 가볍게 하고, 소중한 일에 집중할 수 있는 공간.",
};

export type Message = keyof typeof korean;

export function translate(
  language: Language,
  message: Message,
  values: Record<string, string | number> = {},
) {
  const template = language === "ko" ? korean[message] : message;
  return template.replace(/\{(\w+)\}/g, (match, key: string) =>
    String(values[key] ?? match),
  );
}
