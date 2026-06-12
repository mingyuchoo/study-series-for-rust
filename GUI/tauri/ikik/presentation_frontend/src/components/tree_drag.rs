//! 트리 행 드래그앤드롭 상태 머신. 어떤 행이 끌리는 중이고 어디에
//! 놓을 수 있는지를 한 곳에서 판정한다 — 트리 컴포넌트는 행 클래스와
//! 이벤트 위임만 그린다.

use crate::models::{IkikItem,
                    tree::is_valid_drop};
use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub struct TreeDrag {
    /// 드래그 중인 항목.
    source: Signal<Option<IkikItem>>,
    /// 지금 드롭 대상으로 가리키는 행의 id.
    target: Signal<Option<String>>,
}

pub fn use_tree_drag() -> TreeDrag {
    TreeDrag {
        source: use_signal(|| None),
        target: use_signal(|| None),
    }
}

impl TreeDrag {
    /// 드래그 상태에 따라 자신·유효 대상·무효 대상을 시각적으로 구분한다.
    pub fn row_class(&self, item: &IkikItem) -> &'static str {
        match self.source.read().as_ref() {
            | Some(dragged) if dragged.id == item.id => "tree-row dragging",
            | Some(dragged) if is_valid_drop(dragged, item) =>
                if self.target.read().as_deref() == Some(item.id.as_str()) {
                    "tree-row drop-ok drop-hover"
                } else {
                    "tree-row drop-ok"
                },
            | Some(_) => "tree-row drop-dim",
            | None => "tree-row",
        }
    }

    pub fn start(mut self, item: IkikItem) { self.source.set(Some(item)); }

    pub fn reset(mut self) {
        self.source.set(None);
        self.target.set(None);
    }

    /// dragover: 유효한 드롭 대상이면 prevent_default로 수락해야
    /// 브라우저가 이 행을 드롭 대상으로 받아들인다.
    pub fn hover(mut self, evt: &DragEvent, item: &IkikItem) {
        let Some(dragged) = self.source.peek().clone() else {
            return;
        };
        if is_valid_drop(&dragged, item) {
            evt.prevent_default();
            if self.target.peek().as_deref() != Some(item.id.as_str()) {
                self.target.set(Some(item.id.clone()));
            }
        }
    }

    pub fn leave(mut self, item_id: &str) {
        if self.target.peek().as_deref() == Some(item_id) {
            self.target.set(None);
        }
    }

    /// drop: 유효한 드롭이면 (끌린 항목, 새 상위 항목)을 돌려주고
    /// 상태를 비운다.
    pub fn drop_on(self, evt: &DragEvent, target: &IkikItem) -> Option<(IkikItem, IkikItem)> {
        evt.prevent_default();
        let result = match self.source.peek().clone() {
            | Some(dragged) if is_valid_drop(&dragged, target) => Some((dragged, target.clone())),
            | _ => None,
        };
        self.reset();
        result
    }
}
