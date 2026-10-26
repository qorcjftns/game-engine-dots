//! 실제 TCP 서버와 별도 CLI 프로세스 사이의 계약을 검사합니다.
//! GUI 상태 게시 자체는 engine-editor의 회귀가 담당합니다.

use std::{fs, process::Command, sync::Arc};

use engine_control::QueryServer;
use engine_document::{
    AssetId, EntityId, PngAsset, Position, ProjectDocument, Scene, SpriteEntity,
};
use engine_protocol::{LiveInspection, ProjectInspection};
use serde_json::{Value, json};

fn cli(arguments: &[&str]) -> (i32, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_engine-cli"))
        .args(arguments)
        .output()
        .expect("CLI 프로세스를 실행해야 합니다.");
    let response = serde_json::from_slice(&output.stdout)
        .expect("stdout 전체가 JSON 응답 한 건이어야 합니다.");
    assert!(
        output.stderr.is_empty(),
        "정상 계약 경로에서 진단이 출력됐습니다."
    );
    (output.status.code().unwrap(), response)
}

#[test]
fn tcp_cli_reads_published_state_instead_of_disk_and_reports_closed_server() {
    let directory = tempfile::tempdir().unwrap();
    let connection = directory.path().join("연결 정보.json");
    let project = directory.path().join("한글 프로젝트.json");
    // 디스크와 게시 상태를 의도적으로 다르게 두어 live/disk 경로 혼동을 잡는다.
    let disk_bytes = b"saved bytes must remain untouched";
    fs::write(&project, disk_bytes).unwrap();
    let server = QueryServer::start(&connection).unwrap();
    let snapshot = LiveInspection {
        session_id: "process-contract-session".into(),
        revision: 23,
        busy: true,
        runtime: None,
        is_dirty: true,
        can_undo: true,
        can_redo: false,
        project: Some(Arc::new(ProjectInspection {
            project_file: project.display().to_string(),
            document: ProjectDocument {
                behaviors: vec![],
                schema_version: 2,
                name: "저장 전 문서".into(),
                logical_width: 320,
                logical_height: 180,
                scene: Scene {
                    entities: vec![SpriteEntity {
                        behavior_id: None,
                        id: EntityId(73),
                        asset_id: AssetId(19),
                        name: "게시된 개체".into(),
                        position: Position { x: -12, y: 38 },
                    }],
                },
                assets: vec![PngAsset {
                    id: AssetId(19),
                    path: "assets/도트 그림.png".into(),
                    width: 16,
                    height: 16,
                }],
            },
        })),
    };
    server.publisher().publish(snapshot.clone()).unwrap();
    let connection_bytes = fs::read(&connection).unwrap();
    let arguments = [
        "session",
        "inspect",
        "--connection-file",
        connection.to_str().unwrap(),
    ];
    let (code, response) = cli(&arguments);
    assert_eq!(code, 0);
    assert_eq!(
        response,
        json!({
            "protocol_version": 1, "source": "live", "status": "ok", "data": snapshot,
        })
    );
    assert_eq!(fs::read(&project).unwrap(), disk_bytes);
    assert_eq!(fs::read(&connection).unwrap(), connection_bytes);

    drop(server);
    assert!(!connection.exists());
    // stale 연결 정보를 가진 호출은 새 세션/디스크를 조회하는 대신 실패해야 한다.
    fs::write(&connection, &connection_bytes).unwrap();
    let (code, response) = cli(&arguments);
    assert_eq!(code, 2);
    assert_eq!(response["source"], "live");
    assert_eq!(response["status"], "error");
    assert_eq!(response["error"]["code"], "control_io");
    assert!(response.get("data").is_none());
    assert_eq!(fs::read(&project).unwrap(), disk_bytes);
}

#[test]
fn live_cli_rejects_missing_arguments_without_reading_a_project() {
    let (code, response) = cli(&["session", "inspect"]);
    assert_eq!(code, 2);
    assert_eq!(response["source"], "live");
    assert_eq!(response["error"]["code"], "invalid_arguments");
    assert!(response.get("data").is_none());
}

#[test]
fn live_cli_rejects_missing_and_malformed_connection_files_without_rewriting_them() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("없는 연결.json");
    let malformed = directory.path().join("잘못된 연결.json");
    fs::write(&malformed, b"{").unwrap();
    for path in [&missing, &malformed] {
        let (code, response) = cli(&[
            "session",
            "inspect",
            "--connection-file",
            path.to_str().unwrap(),
        ]);
        assert_eq!(code, 2);
        assert_eq!(response["source"], "live");
        assert_eq!(response["error"]["code"], "invalid_connection_file");
        assert!(response.get("data").is_none());
    }
    assert!(!missing.exists());
    assert_eq!(fs::read(&malformed).unwrap(), b"{");
}
