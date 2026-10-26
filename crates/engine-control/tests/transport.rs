use engine_control::{QueryServer, SessionTracker, inspect_connection_file};
use engine_protocol::{ApiResponse, ErrorCode, LiveInspection};
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::{Duration, Instant},
};

fn address(path: &std::path::Path) -> (String, String) {
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    (
        value["address"].as_str().unwrap().into(),
        value["token"].as_str().unwrap().into(),
    )
}
fn raw(path: &std::path::Path, bytes: &[u8]) -> ApiResponse<LiveInspection> {
    let (address, _) = address(path);
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(4)))
        .unwrap();
    stream.write_all(bytes).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    serde_json::from_str(&response).unwrap()
}
fn code(response: ApiResponse<LiveInspection>) -> ErrorCode {
    match response {
        ApiResponse::Error { error, .. } => error.code,
        _ => panic!("expected error"),
    }
}
#[test]
fn tcp_inspection_auth_validation_and_owned_file_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("한글 연결.json");
    let server = QueryServer::start(&path).unwrap();
    let mut tracker = SessionTracker::new().unwrap();
    let snapshot = tracker.observe(0, None, true).unwrap();
    server.publisher().publish(snapshot.clone()).unwrap();
    assert_eq!(inspect_connection_file(&path).unwrap(), snapshot);
    assert!(QueryServer::start(&path).is_err());
    assert_eq!(code(raw(&path, b"{}\n")), ErrorCode::InvalidRequest);
    assert_eq!(
        code(raw(
            &path,
            b"{\"protocol_version\":1,\"token\":\"wrong\",\"operation\":\"inspect\"}\n"
        )),
        ErrorCode::Unauthorized
    );
    let (_, token) = address(&path);
    let request =
        format!("{{\"protocol_version\":99,\"token\":\"{token}\",\"operation\":\"inspect\"}}\n");
    assert_eq!(
        code(raw(&path, request.as_bytes())),
        ErrorCode::UnsupportedProtocol
    );
    assert_eq!(
        code(raw(&path, &vec![b'x'; 8193])),
        ErrorCode::RequestTooLarge
    );
    drop(server);
    assert!(!path.exists());
}
#[test]
fn tcp_partial_frame_has_total_deadline_and_shutdown_discards_queue() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("connection.json");
    let server = QueryServer::start(&path).unwrap();
    let (address, _) = address(&path);
    let mut stream = TcpStream::connect(&address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(4)))
        .unwrap();
    let started = Instant::now();
    // 수신과 별도로 계속 송신하여 총 기한이 각 read마다 연장되는 회귀를 잡습니다.
    let sending = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let sender_flag = sending.clone();
    let mut writer = stream.try_clone().unwrap();
    let writer_thread = std::thread::spawn(move || {
        while sender_flag.load(std::sync::atomic::Ordering::Acquire) {
            if writer.write_all(b" ").is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    });
    let mut response = String::new();
    let result = stream.read_to_string(&mut response);
    let elapsed = started.elapsed();
    sending.store(false, std::sync::atomic::Ordering::Release);
    writer_thread.join().unwrap();
    assert!(
        elapsed >= Duration::from_millis(1800),
        "frame 기한 전에 닫혔습니다: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(4),
        "slow-drip이 기한을 연장했습니다: {elapsed:?}"
    );
    match result {
        Ok(_) => assert_eq!(
            code(serde_json::from_str(&response).unwrap()),
            ErrorCode::ControlIo
        ),
        Err(error) => assert!(
            matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
            ),
            "{error}"
        ),
    }
    // 추가 송신 없는 불완전 frame은 작은 구조화 기한 오류로 응답해야 합니다.
    assert_eq!(code(raw(&path, b" ")), ErrorCode::ControlIo);
    assert!(inspect_connection_file(&path).is_ok());
    let clients: Vec<_> = (0..12)
        .map(|_| TcpStream::connect(&address).unwrap())
        .collect();
    std::thread::sleep(Duration::from_millis(50));
    let started = Instant::now();
    drop(server);
    assert!(started.elapsed() < Duration::from_secs(1));
    drop(clients);
}
#[test]
fn tcp_modified_connection_file_is_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("connection.json");
    let server = QueryServer::start(&path).unwrap();
    std::fs::write(&path, "user replacement").unwrap();
    drop(server);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "user replacement");
}
#[test]
fn tcp_oversized_snapshot_returns_small_structured_error() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("connection.json");
    let server = QueryServer::start(&path).unwrap();
    let session = engine_document::EditorSession::create(
        directory.path().join("project.json"),
        "검증",
        320,
        180,
    )
    .unwrap();
    let mut snapshot = SessionTracker::new()
        .unwrap()
        .observe(1, Some(&session), false)
        .unwrap();
    std::sync::Arc::make_mut(snapshot.project.as_mut().unwrap())
        .document
        .name = "x".repeat(engine_control::MAX_RESPONSE_BYTES);
    server.publisher().publish(snapshot).unwrap();
    assert_eq!(
        inspect_connection_file(&path).unwrap_err().code,
        ErrorCode::ResponseTooLarge
    );
}
