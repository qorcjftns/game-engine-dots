//! 실제 공식 SDK 서버 프로세스에 initialize → tools/list → tools/call을 보냅니다.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

struct McpProcess {
    child: Child,
    input: Option<ChildStdin>,
    output: mpsc::Receiver<Value>,
}
impl McpProcess {
    fn start(connection: &std::path::Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_engine-mcp"))
            .args(["--connection-file"])
            .arg(connection)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let line = line.unwrap();
                let value: Value =
                    serde_json::from_str(&line).expect("stdout에는 SDK JSON wire만 있어야 합니다.");
                if sender.send(value).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            input: Some(input),
            output: receiver,
        }
    }
    fn send(&mut self, message: Value) {
        writeln!(self.input.as_mut().unwrap(), "{message}").unwrap();
    }
    fn call(&mut self, id: u64, method: &str, params: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        let response = self
            .output
            .recv_timeout(Duration::from_secs(8))
            .expect("SDK 응답 제한 시간을 초과했습니다.");
        assert_eq!(response["id"], id);
        response
    }
    fn initialize(&mut self) {
        self.initialize_version(rmcp::model::ProtocolVersion::LATEST_WITH_INITIALIZE.as_str());
    }
    fn initialize_version(&mut self, version: &str) {
        let response = self.call(
            1,
            "initialize",
            json!({
                "protocolVersion": version,
                "capabilities": {

                },
                "clientInfo": {
                    "name": "engine-mcp-regression",
                    "version": "1"
                }
            }),
        );
        assert!(
            response["result"]["capabilities"]["tools"].is_object(),
            "{response}"
        );
        assert_eq!(response["result"]["serverInfo"]["name"], "engine-mcp");
        assert_eq!(response["result"]["protocolVersion"], version);
        self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    }
}
impl Drop for McpProcess {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn official_sdk_stdio_negotiates_lists_typed_tools_and_structures_control_errors() {
    let directory = tempfile::tempdir().unwrap();
    let mut process = McpProcess::start(&directory.path().join("없는 연결.json"));
    process.initialize();
    let list = process.call(2, "tools/list", json!({}));
    let tools = list["result"]["tools"].as_array().unwrap();
    let mut names: Vec<_> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec!["session_inspect", "session_mutate", "session_result"]
    );
    let mutate = tools
        .iter()
        .find(|tool| tool["name"] == "session_mutate")
        .unwrap();
    assert!(mutate["inputSchema"]["properties"]["command"].is_object());
    assert!(
        mutate["description"]
            .as_str()
            .unwrap()
            .contains("queued/running")
    );
    let result = process.call(
        3,
        "tools/call",
        json!({"name":"session_inspect","arguments":{}}),
    );
    assert_eq!(result["result"]["isError"], true);
    assert_eq!(result["result"]["structuredContent"]["source"], "live");
    assert_eq!(
        result["result"]["structuredContent"]["error"]["code"],
        "invalid_connection_file"
    );
    let invalid = process.call(
        4,
        "tools/call",
        json!({
            "name": "session_mutate",
            "arguments": {
                "expected_session_id": "unused",
                "expected_revision": 0,
                "request_id": "bad-command",
                "timeout_ms": 5000,
                "command": {
                    "type": "shell",
                    "code": "anything"
                }
            }
        }),
    );
    assert!(
        invalid.get("error").is_some() || invalid["result"]["isError"] == true,
        "{invalid}"
    );
    for (index, command) in ["undo", "redo", "save"].into_iter().enumerate() {
        let extended = process.call(
            5 + index as u64,
            "tools/call",
            json!({
                "name": "session_mutate",
                "arguments": {
                    "expected_session_id": "unused",
                    "expected_revision": 0,
                    "request_id": "extended-unit",
                    "timeout_ms": 5000,
                    "command": {
                        "type": command,
                        "extra": 1
                    }
                }
            }),
        );
        assert!(
            extended.get("error").is_some() || extended["result"]["isError"] == true,
            "{extended}"
        );
        assert_ne!(
            extended["result"]["structuredContent"]["error"]["code"],
            "invalid_connection_file"
        );
    }
}

#[test]
fn tcp_official_sdk_mutates_and_retrieves_the_same_common_ledger_receipt() {
    use engine_control::{QueryServer, SessionTracker, apply_command};
    use engine_document::EditorSession;
    use engine_protocol::MutationState;
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project.json");
    std::fs::write(
        &project,
        r#"{
  "schema_version": 1,
  "name": "검증",
  "logical_width": 320,
  "logical_height": 180,
  "scene": {
    "entities": [
      {
        "id": 73,
        "asset_id": 19,
        "name": "원본",
        "position": {
          "x": 1,
          "y": 2
        }
      }
    ]
  },
  "assets": [
    {
      "id": 19,
      "path": "assets/test.png",
      "width": 1,
      "height": 1
    }
  ]
}"#,
    )
    .unwrap();
    let mut session = EditorSession::open(project).unwrap();
    let mut tracker = SessionTracker::new().unwrap();
    let initial = tracker.observe(1, Some(&session), false).unwrap();
    let connection = directory.path().join("connection.json");
    let server = QueryServer::start_with_snapshot(&connection, initial.clone()).unwrap();
    let mut inbox = server.enable_mutations(|| {}).unwrap();
    let mut process = McpProcess::start(&connection);
    process.initialize();
    let request = json!({
        "expected_session_id": initial.session_id,
        "expected_revision": 0,
        "request_id": "mcp-once",
        "timeout_ms": 5000,
        "command": {
            "type": "move_entity",
            "id": 73,
            "position": {
                "x": 60,
                "y": 38
            }
        }
    });
    let queued = process.call(
        2,
        "tools/call",
        json!({"name":"session_mutate","arguments":request}),
    );
    assert_eq!(
        queued["result"]["structuredContent"]["data"]["state"],
        "queued"
    );
    let mut pending = inbox.try_recv().unwrap();
    let command = pending.begin(&initial).unwrap();
    let outcome = apply_command(&command, &mut session);
    let changed = tracker.observe(1, Some(&session), false).unwrap();
    server.publisher().publish(changed.clone()).unwrap();
    let receipt = pending.finish(outcome, &changed).unwrap();
    assert!(matches!(receipt.state, MutationState::Applied { .. }));
    let looked_up = process.call(
        3,
        "tools/call",
        json!({
            "name": "session_result",
            "arguments": {
                "session_id": initial.session_id,
                "request_id": "mcp-once"
            }
        }),
    );
    assert_eq!(
        looked_up["result"]["structuredContent"]["data"],
        serde_json::to_value(&receipt).unwrap()
    );
    let replay = process.call(
        4,
        "tools/call",
        json!({"name":"session_mutate","arguments":request}),
    );
    assert_eq!(
        replay["result"]["structuredContent"]["data"],
        serde_json::to_value(receipt).unwrap()
    );
    assert!(inbox.try_recv().is_none());
    let inspect = process.call(
        5,
        "tools/call",
        json!({"name":"session_inspect","arguments":{}}),
    );
    assert_eq!(
        inspect["result"]["structuredContent"]["data"]["project"]["document"]["scene"]["entities"]
            [0]["position"],
        json!({"x":60,"y":38})
    );
}

#[test]
fn official_sdk_preserves_legacy_initialize_compatibility() {
    let directory = tempfile::tempdir().unwrap();
    let mut process = McpProcess::start(&directory.path().join("missing.json"));
    process.initialize_version("2024-11-05");
}

#[test]
fn official_sdk_latest_no_initialize_routes_list_and_call_from_request_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let mut process = McpProcess::start(&directory.path().join("missing.json"));
    let metadata = json!({
        "io.modelcontextprotocol/protocolVersion": rmcp::model::ProtocolVersion::LATEST,
        "io.modelcontextprotocol/clientInfo": {
            "name": "engine-latest-regression",
            "version": "1"
        },
        "io.modelcontextprotocol/clientCapabilities": {

        }
    });
    let listed = process.call(1, "tools/list", json!({"_meta":metadata}));
    assert_eq!(
        listed["result"]["tools"].as_array().unwrap().len(),
        3,
        "{listed}"
    );
    let called = process.call(
        2,
        "tools/call",
        json!({"name":"session_inspect","arguments":{},"_meta":metadata}),
    );
    assert_eq!(
        called["result"]["structuredContent"]["error"]["code"], "invalid_connection_file",
        "{called}"
    );
    assert_eq!(called["result"]["isError"], true);
}
