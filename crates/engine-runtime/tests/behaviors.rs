use engine_document::{EditCommand, EditorSession, Position};
use engine_runtime::{ActionInput, PlaySession, RuntimeWorld};
fn fixture(source: &str) -> (tempfile::TempDir, EditorSession) {
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
            source: source.into(),
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
    (dir, s)
}
#[test]
fn same_snapshot_ticks_inputs_have_same_positions_pixels_and_logs() {
    let (_dir, mut s) =
        fixture("if right { move(1,0); } set_state(state+1); if primary_pressed { log(\"tap\"); }");
    let before = s.document().clone();
    let saved = std::fs::read(s.project_file()).unwrap();
    let mut a = PlaySession::new(RuntimeWorld::snapshot(&s).unwrap(), 1);
    let mut b = PlaySession::new(RuntimeWorld::snapshot(&s).unwrap(), 1);
    let hash = a.status().frame_hash.clone();
    a.pause(true);
    b.pause(true);
    let input = ActionInput {
        right: true,
        primary: true,
        ..Default::default()
    };
    a.step_with_input(12, input).unwrap();
    b.step_with_input(12, input).unwrap();
    assert_eq!(a.status(), b.status());
    assert_ne!(hash, a.status().frame_hash);
    assert_eq!(a.status().entities[0].position.x, 12);
    assert_eq!(a.status().entities[0].state, 12);
    assert_eq!(a.status().logs.len(), 1);
    assert_eq!(a.status().logs[0].line, 1);
    assert_eq!(s.document(), &before);
    assert_eq!(std::fs::read(s.project_file()).unwrap(), saved);
    s.execute(EditCommand::UpdateBehavior {
        id: before.behaviors[0].id,
        file: "move.rhai".into(),
        source: "move(-5,0);".into(),
    })
    .unwrap();
    a.step_with_input(1, input).unwrap();
    assert_eq!(a.status().entities[0].position.x, 13);
    assert!(a.step(121).is_err());
}
#[test]
fn failures_rollback_complete_tick_pause_and_preserve_source() {
    for source in [
        "move(1,0);\nlet z=0;\nset_position(x,1/z);",
        "move(1,0);\nloop {}",
        "move(1,0);\nlet s=\"a\"; loop {s+=s;}",
        "move(1,0);\nfn rec(){rec()} rec()",
        "move(1,0);\nread_file(\"secret\");",
    ] {
        let (_dir, s) = fixture(source);
        let before = s.document().clone();
        let mut run = PlaySession::new(RuntimeWorld::snapshot(&s).unwrap(), 1);
        let hash = run.status().frame_hash.clone();
        run.pause(true);
        assert!(run.step(1).is_err());
        let error = run.status().error.as_ref().unwrap();
        assert_eq!(error.file, "move.rhai");
        assert_eq!(error.entity_id, Some(before.scene.entities[0].id));
        assert!(error.line >= 2, "{error:?}");
        assert_eq!(run.status().tick, 0);
        assert_eq!(run.status().frame_hash, hash);
        assert_eq!(run.world.document(), &before);
        assert_eq!(s.document(), &before);
        run.pause(false);
        assert!(run.status().paused);
        assert!(run.step(1).is_err());
    }
}
#[test]
fn syntax_errors_and_disabled_collections_are_not_executed() {
    for source in ["x = ;", "let a=[1];", "let a=#{};", "import \"p\" as p;"] {
        let (_dir, s) = fixture(source);
        let before = s.document().clone();
        assert!(!engine_runtime::validate_behaviors(s.document()).is_empty());
        assert!(RuntimeWorld::snapshot(&s).is_err());
        assert_eq!(s.document(), &before);
    }
}
#[test]
fn later_binding_failure_rolls_back_prior_binding_state_and_logs() {
    let (_dir, mut s) = fixture("move(2,0); set_state(state+1); log(\"must rollback\");");
    let asset_id = s.document().assets[0].id;
    let id = s
        .execute(EditCommand::AddSprite {
            asset_id,
            name: "second".into(),
            position: Position::default(),
        })
        .unwrap()
        .entity_id
        .unwrap();
    let behavior_id = s
        .execute(EditCommand::AddBehavior {
            file: "fail.rhai".into(),
            source: "throw \"failure\";".into(),
        })
        .unwrap()
        .behavior_id
        .unwrap();
    s.execute(EditCommand::BindBehavior {
        id,
        behavior_id: Some(behavior_id),
    })
    .unwrap();
    let before = s.document().clone();
    let mut run = PlaySession::new(RuntimeWorld::snapshot(&s).unwrap(), 1);
    run.pause(true);
    assert!(run.step(1).is_err());
    assert_eq!(run.world.document(), &before);
    assert_eq!(run.status().entities[0].state, 0);
    assert!(run.status().logs.is_empty());
}

#[test]
fn unsafe_native_string_functions_are_not_registered() {
    for source in [
        "let text=\"a\"; pad(text,2,\"\");",
        "repeat(\"a\",9223372036854775807);",
    ] {
        let (_dir, s) = fixture(source);
        let mut run = PlaySession::new(RuntimeWorld::snapshot(&s).unwrap(), 1);
        run.pause(true);
        assert!(run.step(1).is_err());
        assert!(
            run.status()
                .error
                .as_ref()
                .unwrap()
                .message
                .contains("not found")
        );
    }
}

#[test]
fn typed_output_errors_keep_actual_source_lines_and_calls_accumulate() {
    for source in [
        "// header\nset_state(state+1);\nx = true;",
        "// header\nset_state(state+1);\nset_position(true,0);",
        "// header\nset_state(state+1);\nset_position(2147483648,0);",
    ] {
        let (_dir, s) = fixture(source);
        let mut run = PlaySession::new(RuntimeWorld::snapshot(&s).unwrap(), 1);
        run.pause(true);
        assert!(run.step(1).is_err());
        assert_eq!(run.status().error.as_ref().unwrap().line, 3);
        assert_eq!(run.status().entities[0].state, 0);
    }
    let (_dir, s) = fixture(
        "move(2,1); move(3,2); set_position(10,20); move(-1,1); set_state(2); set_state(state+1);",
    );
    let mut run = PlaySession::new(RuntimeWorld::snapshot(&s).unwrap(), 1);
    run.pause(true);
    run.step(1).unwrap();
    assert_eq!(run.status().entities[0].position, Position { x: 9, y: 21 });
    assert_eq!(run.status().entities[0].state, 1);
}
