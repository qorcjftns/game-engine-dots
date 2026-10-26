//! 요청 파일의 확장 필드가 연결 전에 거부되는 실제 CLI 경로를 검사합니다.
use serde_json::json;
use std::process::Command;
#[test]
fn cli_unit_commands_reject_extra_fields_before_contacting_a_session() {
    let directory = tempfile::tempdir().unwrap();
    let request = directory.path().join("요청.json");
    let connection = directory.path().join("없는 연결.json");
    for command in ["undo", "redo", "save"] {
        let input = json!({
            "expected_session_id": "session",
            "expected_revision": 0,
            "request_id": "strict-input",
            "timeout_ms": 5000,
            "command": {
                "type": command,
                "extra": 1
            }
        });
        let bytes = serde_json::to_vec(&input).unwrap();
        std::fs::write(&request, &bytes).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_engine-cli"))
            .args(["session", "mutate", "--connection-file"])
            .arg(&connection)
            .arg("--request-file")
            .arg(&request)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stderr.is_empty());
        let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(response["error"]["code"], "invalid_request");
        assert_eq!(std::fs::read(&request).unwrap(), bytes);
        assert!(!connection.exists());
    }
}
