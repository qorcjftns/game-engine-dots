//! 선택과 입력 중인 값은 GUI가 소유한다. 문서 변경은 확정한 명령으로만 전달한다.

use engine_document::{EntityId, Position, ProjectDocument};

#[derive(Clone)]
pub struct EntityDraft {
    pub id: EntityId,
    pub name: String,
    pub position: Position,
}

#[derive(Clone, Copy)]
pub struct DragPreview {
    pub id: EntityId,
    pub original_position: Position,
    pub position: Position,
}

#[derive(Default)]
pub struct EditingState {
    pub selected: Option<EntityId>,
    pub draft: Option<EntityDraft>,
    pub drag: Option<DragPreview>,
    pub drag_origin: Option<eframe::egui::Pos2>,
}

impl EditingState {
    pub fn synchronize(&mut self, document: &ProjectDocument) {
        self.draft = self.selected.and_then(|id| {
            document
                .scene
                .entities
                .iter()
                .find(|entity| entity.id == id)
                .map(|entity| EntityDraft {
                    id,
                    name: entity.name.clone(),
                    position: entity.position,
                })
        });
        if self.draft.is_none() {
            self.selected = None;
        }
        self.drag = None;
        self.drag_origin = None;
    }

    pub fn has_pending_changes(&self, document: &ProjectDocument) -> bool {
        let changed_draft = self.draft.as_ref().is_some_and(|draft| {
            document
                .scene
                .entities
                .iter()
                .find(|entity| entity.id == draft.id)
                .is_some_and(|entity| {
                    entity.name != draft.name || entity.position != draft.position
                })
        });
        changed_draft
            || self
                .drag
                .is_some_and(|drag| drag.position != drag.original_position)
    }

    pub fn preview_position(&self, document: &ProjectDocument) -> Option<(EntityId, Position)> {
        if let Some(drag) = self.drag {
            return Some((drag.id, drag.position));
        }
        self.draft.as_ref().and_then(|draft| {
            document
                .scene
                .entities
                .iter()
                .find(|entity| entity.id == draft.id)
                .filter(|entity| entity.position != draft.position)
                .map(|_| (draft.id, draft.position))
        })
    }
}
