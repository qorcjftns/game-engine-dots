use engine_control::SessionTracker;
use engine_document::{EditCommand, EditorSession, EntityId};

#[test]
fn revision_tracks_committed_changes_and_not_busy_save_or_noop() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("프로젝트.json");
    std::fs::write(&path, r#"{"schema_version":1,"name":"검증","logical_width":320,"logical_height":180,"scene":{"entities":[{"id":73,"asset_id":19,"name":"원본","position":{"x":1,"y":2}}]},"assets":[{"id":19,"path":"assets/test.png","width":1,"height":1}]}"#).unwrap();
    let mut session = EditorSession::open(&path).unwrap();
    let mut tracker = SessionTracker::new().unwrap();
    let initial = tracker.observe(1, Some(&session), false).unwrap();
    let busy = tracker.observe(1, Some(&session), true).unwrap();
    assert_eq!(initial.revision, busy.revision);
    assert!(std::sync::Arc::ptr_eq(
        initial.project.as_ref().unwrap(),
        busy.project.as_ref().unwrap()
    ));
    session
        .execute(EditCommand::RenameEntity {
            id: EntityId(73),
            name: "수정".into(),
        })
        .unwrap();
    let edited = tracker.observe(1, Some(&session), false).unwrap();
    assert_eq!(edited.revision, 1);
    assert!(edited.is_dirty && edited.can_undo);
    assert!(session.undo());
    let undone = tracker.observe(1, Some(&session), false).unwrap();
    assert_eq!(undone.revision, 2);
    assert!(!undone.is_dirty && undone.can_redo);
    assert!(session.redo());
    assert_eq!(
        tracker.observe(1, Some(&session), false).unwrap().revision,
        3
    );
    session.save().unwrap();
    let saved = tracker.observe(1, Some(&session), false).unwrap();
    assert_eq!(saved.revision, 3);
    assert!(!saved.is_dirty);
    let reopened = tracker.observe(2, Some(&session), false).unwrap();
    assert_ne!(initial.session_id, reopened.session_id);
    assert_eq!(reopened.revision, 0);
    assert!(tracker.observe(3, None, false).unwrap().project.is_none());
}
