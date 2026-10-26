"""정상 CI desktop에서 실제 eframe 창과 별도 CLI 프로세스를 연결한다.

GUI 명령은 테스트 전용 host가 같은 EditorAction으로 전달한다. 물리 마우스 입력이나
사용자의 DPI 검증을 대신하지 않는다. graphical session이 없으면 실패하며 skip하지 않는다.
"""

import json
import os
from pathlib import Path
import queue
import struct
import subprocess
import sys
import threading
import time

from smoke_readonly import png_pixel


ROOT = Path(__file__).resolve().parents[1]
SUFFIX = ".exe" if os.name == "nt" else ""
CLI = ROOT / "target/debug" / f"engine-cli{SUFFIX}"
HOST = ROOT / "target/debug/examples" / f"native_session_smoke{SUFFIX}"
MCP = ROOT / "target/debug" / f"engine-mcp{SUFFIX}"


class McpClient:
    """공식 SDK 서버에 실제 JSON-RPC stdio를 보내는 검증 전용 클라이언트."""

    def __init__(self, connection, log, deadline):
        self.deadline = deadline
        self.number = 0
        self.output = queue.Queue()
        self.process = subprocess.Popen([str(MCP), "--connection-file", str(connection)],
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=log, encoding="utf-8", bufsize=1)

        def read():
            try:
                for line in self.process.stdout:
                    self.output.put(json.loads(line))
                self.output.put(RuntimeError("MCP stdout closed before response"))
            except Exception as error:
                self.output.put(error)
        threading.Thread(target=read, daemon=True).start()

    def send(self, value):
        self.process.stdin.write(json.dumps(value, ensure_ascii=False) + "\n")
        self.process.stdin.flush()

    def call(self, method, params):
        self.number += 1
        self.send({"jsonrpc": "2.0", "id": self.number, "method": method, "params": params})
        response = self.output.get(timeout=max(0.1, min(5, self.deadline - time.monotonic())))
        if isinstance(response, Exception):
            raise response
        assert response.get("id") == self.number and "error" not in response, response
        return response["result"]

    def tool(self, name, arguments):
        result = self.call("tools/call", {"name": name, "arguments": arguments})
        assert not result.get("isError", False), result
        value = result["structuredContent"]
        assert value["status"] == "ok" and value["source"] == "live", value
        return value["data"]

    def initialize(self):
        result = self.call("initialize", {"protocolVersion": "2025-11-25", "capabilities": {},
                           "clientInfo": {"name": "engine-native-cross-smoke", "version": "1"}})
        assert "tools" in result["capabilities"]
        self.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
        names = {tool["name"] for tool in self.call("tools/list", {})["tools"]}
        assert names == {"session_inspect", "session_mutate", "session_result"}, names

    def close(self):
        if self.process.stdin and not self.process.stdin.closed:
            self.process.stdin.close()
        try:
            self.process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            self.process.wait(timeout=2)


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    mcp_enabled = sys.argv[1:] == ["--mcp"]
    assert not sys.argv[1:] or mcp_enabled, "Usage: native_session_smoke.py [--mcp]"
    directory = ROOT / "artifacts/native-session"
    directory.mkdir(parents=True, exist_ok=False)
    project_dir = directory / "project"
    (project_dir / "assets").mkdir(parents=True)
    (project_dir / "assets/도트 그림.png").write_bytes(png_pixel())
    document = {
        "schema_version": 1, "name": "Native 공동 편집 검토",
        "logical_width": 320, "logical_height": 180,
        "scene": {"entities": [{"id": 73, "asset_id": 19, "name": "처음 개체",
                  "position": {"x": 40, "y": 28}}]},
        "assets": [{"id": 19, "path": "assets/도트 그림.png", "width": 1, "height": 1}],
    }
    project = project_dir / "project.json"
    project.write_text(json.dumps(document, ensure_ascii=False), encoding="utf-8")
    connection = directory / "connection.json"
    observations = []
    deadline = time.monotonic() + 28
    process = None
    mcp = None
    mcp_log = None

    def wait_for(check):
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise RuntimeError(f"Native host stopped before completion: {process.returncode}")
            result = check()
            if result:
                return result
            time.sleep(0.05)
        raise TimeoutError("Native graphical session/frame or shared command exceeded deadline")

    def read_json(path):
        try:
            return json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            return None

    def cli(*arguments):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError("Native smoke deadline")
        result = subprocess.run([str(CLI), *map(str, arguments)], capture_output=True,
                                encoding="utf-8", timeout=min(5, remaining), check=False)
        value = json.loads(result.stdout)
        assert result.returncode == 0 and value["status"] == "ok", value
        assert not result.stderr, "CLI stdout/stderr contract changed"
        return value["data"]

    def inspect():
        return cli("session", "inspect", "--connection-file", connection)

    def host_command(number, action):
        (directory / "host-command.json").write_text(json.dumps({"id": number, "action": action}),
                                                   encoding="utf-8")
        def acknowledged():
            value = read_json(directory / "host-observation.json")
            return value if value and value["id"] == number else None
        observation = wait_for(acknowledged)
        assert observation["frames"] >= 3
        observations.append({"action": action, **observation})
        return observation["snapshot"]

    def position(snapshot):
        return snapshot["project"]["document"]["scene"]["entities"][0]["position"]

    try:
        with (directory / "native-host.log").open("wb") as log:
            process = subprocess.Popen([str(HOST), str(directory)], stdout=log, stderr=log)
            wait_for(lambda: connection.exists() and read_json(directory / "host-observation.json"))
            before = inspect()
            assert position(before) == {"x": 40, "y": 28} and not before["is_dirty"]
            request = {
                "expected_session_id": before["session_id"], "expected_revision": before["revision"],
                "request_id": "native-cli-move", "timeout_ms": 5000,
                "command": {"type": "move_entity", "id": 73, "position": {"x": 60, "y": 38}},
            }
            request_file = directory / "mutation.json"
            request_file.write_text(json.dumps(request), encoding="utf-8")
            receipt = cli("session", "mutate", "--connection-file", connection,
                          "--request-file", request_file)
            def applied():
                result = cli("session", "result", "--connection-file", connection,
                             "--session-id", before["session_id"], "--request-id", request["request_id"])
                assert result["state"] in ["queued", "running", "applied"], result
                return result if result["state"] == "applied" else None
            receipt = wait_for(applied)
            if mcp_enabled:
                mcp_log = (directory / "mcp-host.log").open("wb")
                mcp = McpClient(connection, mcp_log, deadline)
                mcp.initialize()
                current = inspect()
                assert mcp.tool("session_inspect", {}) == current
                host_command(1, "minimize")
                def minimized_observation():
                    value = read_json(directory / "host-observation.json")
                    return value if value and value.get("minimized") is True else None
                wait_for(minimized_observation)
                mcp_request = {
                    "expected_session_id": current["session_id"],
                    "expected_revision": current["revision"],
                    "request_id": "native-mcp-rename", "timeout_ms": 5000,
                    "command": {"type": "rename_entity", "id": 73, "name": "MCP 공동 편집"},
                }
                mcp.tool("session_mutate", mcp_request)
                # 결과는 CLI로 조회해 MCP와 CLI가 같은 원장을 사용함을 검증한다.
                def mcp_applied():
                    result = cli("session", "result", "--connection-file", connection,
                                 "--session-id", current["session_id"], "--request-id", "native-mcp-rename")
                    assert result["state"] in ["queued", "running", "applied"], result
                    return result if result["state"] == "applied" else None
                receipt = wait_for(mcp_applied)
                assert mcp.tool("session_result", {"session_id": current["session_id"],
                                                   "request_id": "native-mcp-rename"}) == receipt
                assert mcp.tool("session_mutate", mcp_request) == receipt
                def minimized_change():
                    value = minimized_observation()
                    if value and value["snapshot"]["project"]["document"]["scene"]["entities"][0]["name"] == "MCP 공동 편집":
                        return value
                    return None
                minimized = wait_for(minimized_change)
                observations.append({"action": "minimized_mutation", **minimized})
                host_command(2, "restore")
                def restored():
                    value = read_json(directory / "host-observation.json")
                    return value if value and value.get("minimized") is False else None
                wait_for(restored)
            offset = 2 if mcp_enabled else 0
            moved = host_command(1 + offset, "observe")
            assert position(moved) == {"x": 60, "y": 38} and moved["is_dirty"]
            assert moved["revision"] == receipt["revision"] > before["revision"]
            host_command(2 + offset, "capture")
            wait_for(lambda: (directory / f"frame-{2 + offset}.png").exists())
            undone = host_command(3 + offset, "undo")
            if mcp_enabled:
                assert position(undone) == {"x": 60, "y": 38} and undone["is_dirty"]
                assert undone["project"]["document"]["scene"]["entities"][0]["name"] == "처음 개체"
            else:
                assert position(undone) == {"x": 40, "y": 28} and not undone["is_dirty"]
            assert inspect() == undone
            redone = host_command(4 + offset, "redo")
            assert position(redone) == {"x": 60, "y": 38} and redone["is_dirty"]
            if mcp_enabled:
                assert redone["project"]["document"]["scene"]["entities"][0]["name"] == "MCP 공동 편집"
            reopened = host_command(5 + offset, "save_reopen")
            assert position(reopened) == {"x": 60, "y": 38} and not reopened["is_dirty"]
            assert reopened["session_id"] != before["session_id"]
            assert inspect() == reopened
            if mcp_enabled:
                assert mcp.tool("session_inspect", {}) == reopened
            saved = cli("project", "inspect", project)["document"]
            assert saved == reopened["project"]["document"]
            host_command(6 + offset, "capture")
            wait_for(lambda: (directory / f"frame-{6 + offset}.png").exists())
            for number in [2 + offset, 6 + offset]:
                png = (directory / f"frame-{number}.png").read_bytes()
                assert png[:8] == b"\x89PNG\r\n\x1a\n"
                width, height = struct.unpack(">II", png[16:24])
                assert width >= 640 and height >= 480, (width, height)
            # 마지막 ack와 종료가 경쟁하므로 close는 정상 종료 자체로 확인한다.
            (directory / "host-command.json").write_text(json.dumps({"id": 7 + offset, "action": "close"}),
                                                       encoding="utf-8")
            assert process.wait(timeout=min(5, max(0.1, deadline - time.monotonic()))) == 0
            assert not connection.exists(), "Native GUI did not clean its connection descriptor"
        summary = {"result": "passed", "boundary": "native eframe + renderer + external CLI",
                   "official_mcp_cross_tested": mcp_enabled,
                   "actual_minimized_mutation_tested": mcp_enabled,
                   "physical_ui_input_tested": False, "observations": observations}
        (directory / "native-summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2),
                                                     encoding="utf-8")
        print("Native GUI↔CLI: frame·실편집·GUI undo/redo·저장/재열기·종료 정리 통과")
    finally:
        if mcp is not None:
            mcp.close()
        if mcp_log is not None:
            mcp_log.close()
        if process is not None and process.poll() is None:
            process.terminate()
            process.wait(timeout=5)


if __name__ == "__main__":
    main()
