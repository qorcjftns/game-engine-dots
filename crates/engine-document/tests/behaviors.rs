use engine_document::{BehaviorId, EditCommand, EditorSession, Position};
#[test]
fn migration_is_in_memory_and_behavior_history_is_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.json");
    let old = r#"{"schema_version":1,"name":"v1","logical_width":32,"logical_height":18,"assets":[],"scene":{"entities":[]}}"#;
    std::fs::write(&path, old).unwrap();
    let mut s = EditorSession::open(&path).unwrap();
    assert_eq!(s.document().schema_version, 2);
    assert!(!s.is_dirty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), old);
    let id = s
        .execute(EditCommand::AddBehavior {
            file: "이동.rhai".into(),
            source: "x+=1;".into(),
        })
        .unwrap()
        .behavior_id
        .unwrap();
    s.save().unwrap();
    let saved = s.document().clone();
    s.execute(EditCommand::UpdateBehavior {
        id,
        file: "이동.rhai".into(),
        source: "syntax invalid".into(),
    })
    .unwrap();
    assert!(s.is_dirty());
    s.undo();
    assert_eq!(s.document(), &saved);
    assert!(!s.is_dirty());
    s.redo();
    s.save().unwrap();
    assert_eq!(EditorSession::open(&path).unwrap().document(), s.document());
    let before = s.document().clone();
    assert!(
        s.execute(EditCommand::AddBehavior {
            file: "../escape.rhai".into(),
            source: "".into()
        })
        .is_err()
    );
    assert!(
        s.execute(EditCommand::AddBehavior {
            file: "large.rhai".into(),
            source: " ".repeat(8193)
        })
        .is_err()
    );
    assert_eq!(s.document(), &before);
}
#[test]
fn binding_undo_and_missing_resources_preserve_document() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("p.png");
    image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 255, 255]))
        .save(&png)
        .unwrap();
    let mut s = EditorSession::create(dir.path().join("p.json"), "p", 32, 18).unwrap();
    let asset_id = s.import_png(png).unwrap();
    let id = s
        .execute(EditCommand::AddSprite {
            asset_id,
            name: "p".into(),
            position: Position::default(),
        })
        .unwrap()
        .entity_id
        .unwrap();
    let behavior_id = s
        .execute(EditCommand::AddBehavior {
            file: "move.rhai".into(),
            source: "x+=1;".into(),
        })
        .unwrap()
        .behavior_id
        .unwrap();
    s.execute(EditCommand::BindBehavior {
        id,
        behavior_id: Some(behavior_id),
    })
    .unwrap();
    s.save().unwrap();
    let before = s.document().clone();
    assert!(
        s.execute(EditCommand::RemoveBehavior { id: behavior_id })
            .is_err()
    );
    assert!(
        s.execute(EditCommand::BindBehavior {
            id,
            behavior_id: Some(BehaviorId(999))
        })
        .is_err()
    );
    assert_eq!(s.document(), &before);
    s.undo();
    assert_eq!(s.document().scene.entities[0].behavior_id, None);
    s.redo();
    assert_eq!(s.document(), &before);
    assert_eq!(
        EditorSession::open(s.project_file()).unwrap().document(),
        &before
    );
}
