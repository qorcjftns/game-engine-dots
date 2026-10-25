use std::{fs, path::Path};

use engine_document::{AssetId, EditCommand, EditorSession, EntityId, Position, ProjectDocument};
use tempfile::TempDir;

fn project() -> (TempDir, EditorSession) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("한글 프로젝트/project.json");
    let session = EditorSession::create(path, "내 도트 게임", 320, 180).unwrap();
    (directory, session)
}

fn png(path: &Path) {
    image::RgbaImage::from_pixel(2, 3, image::Rgba([255, 120, 40, 255]))
        .save_with_format(path, image::ImageFormat::Png)
        .unwrap();
}

fn sprite(session: &mut EditorSession, asset_id: AssetId, name: &str) -> EntityId {
    session
        .execute(EditCommand::AddSprite {
            asset_id,
            name: name.into(),
            position: Position::default(),
        })
        .unwrap()
        .entity_id
        .unwrap()
}

#[test]
fn korean_project_round_trips_with_imported_pixels_and_edits() {
    let (directory, mut session) = project();
    let source = directory.path().join("주인공.png");
    png(&source);
    let asset_id = session.import_png(&source).unwrap();
    let id = sprite(&mut session, asset_id, "임시 이름");
    session
        .execute(EditCommand::RenameEntity {
            id,
            name: "주인공".into(),
        })
        .unwrap();
    session
        .execute(EditCommand::MoveEntity {
            id,
            position: Position { x: 31, y: -2 },
        })
        .unwrap();
    assert!(session.is_dirty());
    session.save().unwrap();
    assert!(!session.is_dirty());

    let reopened = EditorSession::open(session.project_file()).unwrap();
    assert_eq!(reopened.document(), session.document());
    assert_eq!(reopened.document().scene.entities[0].id, id);
    assert_eq!(reopened.read_png_asset(asset_id).unwrap().rgba.len(), 24);
    assert!(reopened.document().assets[0].path.starts_with("assets/"));
    assert!(!reopened.can_undo());
    let json = fs::read_to_string(reopened.project_file()).unwrap();
    assert!(json.contains("내 도트 게임"));
    assert!(json.contains("schema_version"));
}

#[test]
fn create_never_overwrites_an_existing_project() {
    let (_directory, session) = project();
    let original = fs::read(session.project_file()).unwrap();
    assert!(EditorSession::create(session.project_file(), "다른 이름", 640, 360).is_err());
    assert_eq!(fs::read(session.project_file()).unwrap(), original);
}

#[test]
fn commands_undo_to_the_original_document_and_restore_deleted_order() {
    let (directory, mut session) = project();
    let source = directory.path().join("sprite.png");
    png(&source);
    let asset = session.import_png(&source).unwrap();
    let first = sprite(&mut session, asset, "첫 번째");
    let second = sprite(&mut session, asset, "두 번째");
    let original = session.document().clone();
    session
        .execute(EditCommand::RemoveEntity { id: first })
        .unwrap();
    assert_eq!(session.document().scene.entities[0].id, second);
    assert!(session.undo());
    assert_eq!(session.document(), &original);
    assert!(session.redo());
    assert_eq!(session.document().scene.entities[0].id, second);
    assert!(session.undo());
    session
        .execute(EditCommand::MoveEntity {
            id: first,
            position: Position { x: 7, y: 8 },
        })
        .unwrap();
    assert!(!session.can_redo());
    assert!(session.undo());
    assert_eq!(session.document(), &original);
}

#[test]
fn failed_and_no_op_commands_preserve_document_and_redo() {
    let (directory, mut session) = project();
    let source = directory.path().join("sprite.png");
    png(&source);
    let asset = session.import_png(&source).unwrap();
    let id = sprite(&mut session, asset, "주인공");
    session
        .execute(EditCommand::MoveEntity {
            id,
            position: Position { x: 5, y: 6 },
        })
        .unwrap();
    session.undo();
    let original = session.document().clone();
    assert!(
        session
            .execute(EditCommand::RenameEntity {
                id,
                name: "  ".into()
            })
            .is_err()
    );
    assert!(
        session
            .execute(EditCommand::AddSprite {
                asset_id: AssetId(999),
                name: "없는 리소스".into(),
                position: Position::default(),
            })
            .is_err()
    );
    session
        .execute(EditCommand::MoveEntity {
            id,
            position: Position::default(),
        })
        .unwrap();
    assert_eq!(session.document(), &original);
    assert!(session.can_redo());
    session.redo();
    assert_eq!(
        session.document().scene.entities[0].position,
        Position { x: 5, y: 6 }
    );
}

#[test]
fn dirty_state_follows_the_saved_document_after_undo_and_redo() {
    let (directory, mut session) = project();
    let source = directory.path().join("sprite.png");
    png(&source);
    let asset = session.import_png(&source).unwrap();
    let id = sprite(&mut session, asset, "주인공");
    session.save().unwrap();
    session
        .execute(EditCommand::MoveEntity {
            id,
            position: Position { x: 8, y: 9 },
        })
        .unwrap();
    assert!(session.is_dirty());
    session.undo();
    assert!(!session.is_dirty());
    session.redo();
    assert!(session.is_dirty());
}

#[test]
fn undoing_an_import_keeps_the_file_and_future_imports_do_not_overwrite_it() {
    let (directory, mut session) = project();
    let source = directory.path().join("sprite.png");
    png(&source);
    let first = session.import_png(&source).unwrap();
    let path = session.asset_path(first).unwrap();
    let original_bytes = fs::read(&path).unwrap();
    assert!(session.undo());
    assert!(session.document().assets.is_empty());
    assert!(path.exists());
    assert!(!session.is_dirty());

    let other_source = directory.path().join("other.png");
    image::RgbaImage::from_pixel(2, 3, image::Rgba([4, 5, 6, 255]))
        .save_with_format(&other_source, image::ImageFormat::Png)
        .unwrap();
    let second = session.import_png(&other_source).unwrap();
    assert_ne!(first, second);
    assert_eq!(fs::read(&path).unwrap(), original_bytes);
    assert_eq!(
        session.read_png_asset(second).unwrap().rgba[..4],
        [4, 5, 6, 255]
    );
    assert!(session.undo());
    assert!(!session.is_dirty());

    session.save().unwrap();
    let mut reopened = EditorSession::open(session.project_file()).unwrap();
    let third = reopened.import_png(&source).unwrap();
    assert_ne!(first, third);
    assert_ne!(second, third);
    assert_eq!(fs::read(&path).unwrap(), original_bytes);
    assert!(path.exists());
}

#[test]
fn external_file_changes_are_preserved_when_save_fails() {
    let (directory, mut session) = project();
    let source = directory.path().join("sprite.png");
    png(&source);
    session.import_png(&source).unwrap();
    let original = session.document().clone();
    fs::write(session.project_file(), b"external content").unwrap();
    assert!(session.save().is_err());
    assert_eq!(
        fs::read(session.project_file()).unwrap(),
        b"external content"
    );
    assert_eq!(session.document(), &original);
    assert!(session.is_dirty());
    assert!(session.can_undo());
}

#[test]
fn import_does_not_reuse_a_missing_file_path_reserved_by_another_asset() {
    let (directory, mut session) = project();
    let source = directory.path().join("sprite.png");
    png(&source);
    let first = session.import_png(&source).unwrap();
    let project_file = session.project_file().to_path_buf();
    let mut document = session.document().clone();
    document.assets[0].path = "assets/asset-2.png".into();
    fs::write(&project_file, serde_json::to_vec_pretty(&document).unwrap()).unwrap();

    let mut reopened = EditorSession::open(project_file).unwrap();
    let second = reopened.import_png(source).unwrap();
    assert_eq!(second, AssetId(3));
    assert_eq!(reopened.document().assets[0].path, "assets/asset-2.png");
    assert!(reopened.asset_path(first).is_err());
    assert!(reopened.read_png_asset(second).is_ok());
}

#[test]
fn missing_project_file_does_not_clear_dirty_state_or_history() {
    let (directory, mut session) = project();
    let source = directory.path().join("sprite.png");
    png(&source);
    session.import_png(&source).unwrap();
    let original = session.document().clone();
    fs::remove_file(session.project_file()).unwrap();
    assert!(session.save().is_err());
    assert_eq!(session.document(), &original);
    assert!(session.is_dirty());
    assert!(session.undo());
}

#[test]
fn malformed_and_unsupported_projects_do_not_replace_a_working_session() {
    let (directory, session) = project();
    let invalid = directory.path().join("broken.json");
    fs::write(&invalid, b"not JSON").unwrap();
    assert!(EditorSession::open(&invalid).is_err());
    let mut document = session.document().clone();
    document.schema_version = 99;
    fs::write(&invalid, serde_json::to_vec(&document).unwrap()).unwrap();
    assert!(EditorSession::open(&invalid).is_err());
    assert_eq!(session.document().name, "내 도트 게임");
    assert!(!session.is_dirty());
}

#[test]
fn duplicate_ids_missing_references_and_unsafe_paths_are_rejected() {
    let (directory, mut session) = project();
    let source = directory.path().join("sprite.png");
    png(&source);
    let asset = session.import_png(&source).unwrap();
    sprite(&mut session, asset, "주인공");
    let original = session.document().clone();
    let mut document = original.clone();
    document
        .scene
        .entities
        .push(document.scene.entities[0].clone());
    assert!(document.validate().is_err());
    document = original.clone();
    document.assets.push(document.assets[0].clone());
    assert!(document.validate().is_err());
    document = original.clone();
    document.scene.entities[0].asset_id = AssetId(999);
    assert!(document.validate().is_err());
    for path in [
        "../outside.png",
        "/outside.png",
        "assets/../outside.png",
        "assets\\outside.png",
        "assets/C:outside.png",
    ] {
        document = original.clone();
        document.assets[0].path = path.into();
        assert!(document.validate().is_err(), "accepted {path}");
    }
    let invalid = directory.path().join("duplicate.json");
    let mut document: ProjectDocument = original;
    document
        .scene
        .entities
        .push(document.scene.entities[0].clone());
    fs::write(&invalid, serde_json::to_vec(&document).unwrap()).unwrap();
    assert!(EditorSession::open(invalid).is_err());
}

#[test]
fn invalid_png_does_not_create_an_asset_or_change_the_document() {
    let (directory, mut session) = project();
    let source = directory.path().join("fake.png");
    fs::write(&source, b"\x89PNG\r\n\x1a\ntruncated").unwrap();
    let original = session.document().clone();
    assert!(session.import_png(source).is_err());
    assert_eq!(session.document(), &original);
    assert!(!session.can_undo());
    assert!(!session.project_directory().join("assets").exists());
}

#[test]
fn a_png_with_dimensions_but_truncated_pixel_data_is_rejected() {
    let (directory, mut session) = project();
    let source = directory.path().join("truncated.png");
    png(&source);
    let bytes = fs::read(&source).unwrap();
    // 서명과 IHDR은 유지하지만 픽셀 데이터는 잘라냅니다.
    fs::write(&source, &bytes[..40]).unwrap();
    assert!(session.import_png(source).is_err());
    assert!(session.document().assets.is_empty());
    assert!(!session.is_dirty());
}

#[cfg(unix)]
#[test]
fn asset_symlinks_cannot_escape_the_project_directory() {
    let (directory, mut session) = project();
    let source = directory.path().join("outside.png");
    png(&source);
    let asset = session.import_png(&source).unwrap();
    let imported = session.asset_path(asset).unwrap();
    fs::remove_file(&imported).unwrap();
    std::os::unix::fs::symlink(&source, &imported).unwrap();
    assert!(session.asset_path(asset).is_err());
    assert!(session.read_png_asset(asset).is_err());
}
