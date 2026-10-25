//! 파일과 명령 경계에서 저장·전환·드래그의 회귀를 확인한다. 네이티브 대화상자는 열지 않는다.

use std::fs;

use eframe::egui;
use engine_document::{EditCommand, EditorSession, EntityId, Position};

use crate::{
    app::EditorApp,
    editor_actions::{Confirmation, EditorAction},
};

const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 16, 0, 0, 0, 16, 8, 6,
    0, 0, 0, 31, 243, 255, 97, 0, 0, 0, 33, 73, 68, 65, 84, 120, 156, 99, 96, 24, 62, 224, 117,
    158, 249, 127, 124, 216, 104, 223, 157, 6, 100, 60, 106, 192, 168, 1, 180, 49, 96, 232, 2, 0,
    29, 253, 40, 160, 35, 56, 112, 14, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn fixture() -> (tempfile::TempDir, EditorApp, EntityId) {
    let directory = tempfile::tempdir().unwrap();
    let image = directory.path().join("source.png");
    fs::write(&image, PNG).unwrap();
    let mut session =
        EditorSession::create(directory.path().join("project.json"), "편집 검사", 160, 144)
            .unwrap();
    let asset_id = session.import_png(&image).unwrap();
    let id = session
        .execute(EditCommand::AddSprite {
            asset_id,
            name: "원래 이름".into(),
            position: Position { x: 20, y: 30 },
        })
        .unwrap()
        .entity_id
        .unwrap();
    session.save().unwrap();
    let mut app = EditorApp::default();
    app.workspace.session = Some(session);
    app.editing.selected = Some(id);
    app.synchronize_selection();
    (directory, app, id)
}

#[test]
fn save_commits_the_visible_name_draft_before_writing_the_file() {
    let (_directory, mut app, _) = fixture();
    app.editing.draft.as_mut().unwrap().name = "저장할 이름".into();
    app.process_action(&egui::Context::default(), EditorAction::Save);
    let session = app.workspace.session.as_ref().unwrap();
    let reopened = EditorSession::open(session.project_file()).unwrap();
    assert_eq!(reopened.document().scene.entities[0].name, "저장할 이름");
    assert!(!app.has_unsaved_changes());
}

#[test]
fn failed_save_preserves_the_pending_close_and_current_work() {
    let (_directory, mut app, _) = fixture();
    let context = egui::Context::default();
    app.editing.draft.as_mut().unwrap().name = "보존할 이름".into();
    app.process_action(&context, EditorAction::Quit);
    let path = app
        .workspace
        .session
        .as_ref()
        .unwrap()
        .project_file()
        .to_owned();
    fs::write(&path, "외부 변경").unwrap();
    app.confirm_transition(&context, Confirmation::Save);
    assert!(app.pending_transition.is_some());
    assert!(!app.close_approved);
    assert!(app.workspace.session.as_ref().unwrap().is_dirty());
    assert_eq!(app.editing.draft.as_ref().unwrap().name, "보존할 이름");
    assert_eq!(fs::read_to_string(path).unwrap(), "외부 변경");
}

#[test]
fn discarding_a_transition_does_not_erase_work_when_the_new_project_form_is_cancelled() {
    let (_directory, mut app, id) = fixture();
    let context = egui::Context::default();
    app.execute(EditCommand::MoveEntity {
        id,
        position: Position { x: 40, y: 50 },
    });
    app.synchronize_selection();
    app.editing.draft.as_mut().unwrap().name = "아직 입력 중".into();
    app.process_action(&context, EditorAction::NewProject);
    app.confirm_transition(&context, Confirmation::Discard);
    assert!(app.workspace.form_is_open());
    app.workspace.cancel_form();
    assert!(app.workspace.session.as_ref().unwrap().is_dirty());
    assert_eq!(app.editing.draft.as_ref().unwrap().name, "아직 입력 중");
    assert_eq!(
        app.workspace
            .session
            .as_ref()
            .unwrap()
            .document()
            .scene
            .entities[0]
            .position
            .x,
        40
    );
}

#[test]
fn cancelling_close_preserves_even_an_invalid_uncommitted_draft() {
    let (_directory, mut app, _) = fixture();
    let context = egui::Context::default();
    app.editing.draft.as_mut().unwrap().name.clear();
    app.process_action(&context, EditorAction::Quit);
    assert!(app.pending_transition.is_some());
    app.confirm_transition(&context, Confirmation::Cancel);
    assert!(app.pending_transition.is_none());
    assert!(!app.close_approved);
    assert!(app.editing.draft.as_ref().unwrap().name.is_empty());
    assert!(!app.workspace.session.as_ref().unwrap().is_dirty());
}

#[test]
fn multiple_drag_updates_make_one_undo_step_and_escape_does_not_change_the_document() {
    let (_directory, mut app, id) = fixture();
    let context = egui::Context::default();
    assert!(app.begin_drag(id));
    for step in 1..=8 {
        app.update_drag(egui::Vec2::new(step as f32 * 4.0, -12.0), 2.0);
    }
    assert_eq!(
        app.workspace
            .session
            .as_ref()
            .unwrap()
            .document()
            .scene
            .entities[0]
            .position,
        Position { x: 20, y: 30 }
    );
    assert!(app.commit_edits());
    assert_eq!(
        app.workspace
            .session
            .as_ref()
            .unwrap()
            .document()
            .scene
            .entities[0]
            .position,
        Position { x: 36, y: 24 }
    );
    app.process_action(&context, EditorAction::Undo);
    assert_eq!(
        app.workspace
            .session
            .as_ref()
            .unwrap()
            .document()
            .scene
            .entities[0]
            .position,
        Position { x: 20, y: 30 }
    );
    assert!(app.begin_drag(id));
    app.update_drag(egui::Vec2::splat(500.0), 2.0);
    app.cancel_drag();
    assert!(!app.has_unsaved_changes());
}

#[test]
fn undoing_creation_clears_selection_and_asset_history_refreshes_the_cache() {
    let (_directory, mut app, _) = fixture();
    let context = egui::Context::default();
    app.process_action(&context, EditorAction::Undo);
    assert!(app.editing.selected.is_none());
    let generation = app.workspace.textures.generation;
    app.process_action(&context, EditorAction::Undo);
    assert!(app.workspace.textures.generation > generation);
    app.process_action(&context, EditorAction::Redo);
    let id = app.workspace.session.as_ref().unwrap().document().assets[0].id;
    assert!(app.workspace.textures.get(id).is_some());
}

#[test]
fn text_input_keeps_undo_and_delete_shortcuts_out_of_the_document() {
    let (_directory, mut app, _) = fixture();
    let context = egui::Context::default();
    let mut text = "text input".to_owned();
    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
        ui.text_edit_singleline(&mut text).request_focus();
    });
    output.textures_delta.clear();
    for key in [egui::Key::Z, egui::Key::Delete] {
        let modifiers = if key == egui::Key::Z {
            egui::Modifiers::COMMAND
        } else {
            egui::Modifiers::NONE
        };
        let input = egui::RawInput {
            events: vec![
                egui::Event::ModifiersChanged(modifiers),
                egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers,
                },
            ],
            ..Default::default()
        };
        let mut output = context.run_ui(input, |ui| {
            ui.text_edit_singleline(&mut text);
            app.keyboard_shortcuts(ui.ctx());
        });
        output.textures_delta.clear();
        assert!(app.queued_action.is_none());
    }
    assert_eq!(
        app.workspace
            .session
            .as_ref()
            .unwrap()
            .document()
            .scene
            .entities
            .len(),
        1
    );
}

#[test]
fn canvas_focus_allows_document_undo_and_clean_quit_sends_a_close_command() {
    let (_directory, mut app, id) = fixture();
    let context = egui::Context::default();
    app.execute(EditCommand::MoveEntity {
        id,
        position: Position { x: 44, y: 55 },
    });
    app.synchronize_selection();
    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
        ui.allocate_response(egui::vec2(100.0, 100.0), egui::Sense::click_and_drag())
            .request_focus();
    });
    output.textures_delta.clear();
    let input = egui::RawInput {
        events: vec![
            egui::Event::ModifiersChanged(egui::Modifiers::COMMAND),
            egui::Event::Key {
                key: egui::Key::Z,
                physical_key: Some(egui::Key::Z),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            },
        ],
        ..Default::default()
    };
    let mut output = context.run_ui(input, |ui| {
        ui.allocate_response(egui::vec2(100.0, 100.0), egui::Sense::click_and_drag());
        app.keyboard_shortcuts(ui.ctx());
    });
    output.textures_delta.clear();
    assert!(matches!(app.queued_action.take(), Some(EditorAction::Undo)));
    app.process_action(&context, EditorAction::Undo);
    assert!(!app.has_unsaved_changes());
    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
        app.process_action(ui.ctx(), EditorAction::Quit);
    });
    assert!(app.close_approved);
    assert!(app.workspace.session.is_some());
    assert!(output.viewport_output.values().any(|viewport| {
        viewport
            .commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Close))
    }));
    output.textures_delta.clear();
}

#[test]
fn drag_coordinates_saturate_at_the_document_integer_limits() {
    let (_directory, mut app, id) = fixture();
    app.execute(EditCommand::MoveEntity {
        id,
        position: Position {
            x: i32::MAX,
            y: i32::MIN,
        },
    });
    app.synchronize_selection();
    assert!(app.begin_drag(id));
    app.update_drag(egui::vec2(500.0, -500.0), 1.0);
    let drag = app.editing.drag.unwrap();
    assert_eq!(
        drag.position,
        Position {
            x: i32::MAX,
            y: i32::MIN
        }
    );
}

#[test]
fn command_q_in_a_text_field_requests_confirmation_without_egui_closing_the_window() {
    let (_directory, mut app, _) = fixture();
    let context = egui::Context::default();
    crate::app::configure_context(&context);
    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
        ui.text_edit_singleline(&mut app.editing.draft.as_mut().unwrap().name)
            .request_focus();
    });
    output.textures_delta.clear();
    let mut output = context.run_ui(
        egui::RawInput {
            events: vec![egui::Event::Text(" 입력 중".into())],
            ..Default::default()
        },
        |ui| {
            ui.text_edit_singleline(&mut app.editing.draft.as_mut().unwrap().name);
        },
    );
    output.textures_delta.clear();
    let draft = app.editing.draft.as_ref().unwrap().name.clone();
    assert!(app.has_unsaved_changes());
    let mut output = context.run_ui(
        egui::RawInput {
            events: vec![
                egui::Event::ModifiersChanged(egui::Modifiers::COMMAND),
                egui::Event::Key {
                    key: egui::Key::Q,
                    physical_key: Some(egui::Key::Q),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                },
            ],
            ..Default::default()
        },
        |ui| {
            ui.text_edit_singleline(&mut app.editing.draft.as_mut().unwrap().name);
            app.quit_shortcut(ui.ctx());
            if let Some(action) = app.queued_action.take() {
                app.process_action(ui.ctx(), action);
            }
        },
    );
    assert!(app.pending_transition.is_some());
    assert!(!app.close_approved);
    assert!(!output.viewport_output.values().any(|viewport| {
        viewport
            .commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Close))
    }));
    output.textures_delta.clear();
    app.confirm_transition(&context, Confirmation::Cancel);
    assert_eq!(app.editing.draft.as_ref().unwrap().name, draft);
}

#[test]
fn real_pointer_frames_commit_the_full_drag_once_and_undo_redo_restore_it() {
    let (_directory, mut app, _) = fixture();
    let context = egui::Context::default();
    crate::app::configure_context(&context);
    let start = egui::pos2(180.0, 150.0);
    let end = egui::pos2(224.0, 174.0);
    let frames = vec![
        vec![egui::Event::PointerMoved(start)],
        vec![egui::Event::PointerButton {
            pos: start,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
        vec![egui::Event::PointerMoved(egui::pos2(200.0, 160.0))],
        vec![egui::Event::PointerMoved(end)],
        vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    ];
    for (index, events) in frames.into_iter().enumerate() {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                time: Some(index as f64 * 0.1),
                events,
                ..Default::default()
            },
            |ui| {
                let session = app.workspace.session.as_ref().unwrap();
                app.canvas_interaction = Some(app.workspace.preview.show(
                    ui,
                    session.document(),
                    &app.workspace.textures,
                    app.editing.selected,
                    app.editing.preview_position(session.document()),
                ));
                app.handle_canvas(ui.ctx());
            },
        );
        output.textures_delta.clear();
    }
    assert_eq!(
        app.workspace
            .session
            .as_ref()
            .unwrap()
            .document()
            .scene
            .entities[0]
            .position,
        Position { x: 31, y: 36 }
    );
    app.process_action(&context, EditorAction::Undo);
    assert_eq!(
        app.workspace
            .session
            .as_ref()
            .unwrap()
            .document()
            .scene
            .entities[0]
            .position,
        Position { x: 20, y: 30 }
    );
    app.process_action(&context, EditorAction::Redo);
    assert_eq!(
        app.workspace
            .session
            .as_ref()
            .unwrap()
            .document()
            .scene
            .entities[0]
            .position,
        Position { x: 31, y: 36 }
    );
}

#[test]
fn logic_only_native_close_protects_drafts_and_clean_documents_can_close() {
    let (_directory, mut app, _) = fixture();
    let context = egui::Context::default();
    let mut input = egui::RawInput::default();
    let viewport = input.viewports.entry(egui::ViewportId::ROOT).or_default();
    viewport.events.push(egui::ViewportEvent::Close);
    viewport.minimized = Some(true);
    app.editing.draft.as_mut().unwrap().name = "가려진 창의 입력".into();
    let output = context.run_logic(&input, |context| app.handle_native_close(context));
    assert!(
        output
            .viewport_commands
            .get(&egui::ViewportId::ROOT)
            .unwrap()
            .contains(&egui::ViewportCommand::CancelClose)
    );
    assert!(app.pending_transition.is_some());
    assert!(!app.close_approved);
    app.confirm_transition(&context, Confirmation::Cancel);
    assert_eq!(app.editing.draft.as_ref().unwrap().name, "가려진 창의 입력");
    app.synchronize_selection();
    let output = context.run_logic(&input, |context| app.handle_native_close(context));
    assert!(
        !output
            .viewport_commands
            .get(&egui::ViewportId::ROOT)
            .is_some_and(|commands| commands.contains(&egui::ViewportCommand::CancelClose))
    );
    assert!(app.close_approved);
}
