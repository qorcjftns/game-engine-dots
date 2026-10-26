"""정상 CI desktop에서 실제 eframe 창과 별도 CLI 프로세스를 연결한다.

GUI 명령은 테스트 전용 host가 같은 EditorAction으로 전달한다. 물리 마우스 입력이나
사용자의 DPI 검증을 대신하지 않는다. graphical session이 없으면 실패하며 skip하지 않는다.
"""

import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import time

from smoke_readonly import png_pixel


ROOT = Path(__file__).resolve().parents[1]
SUFFIX = ".exe" if os.name == "nt" else ""
CLI = ROOT / "target/debug" / f"engine-cli{SUFFIX}"
HOST = ROOT / "target/debug/examples" / f"native_session_smoke{SUFFIX}"


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
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
            moved = host_command(1, "observe")
            assert position(moved) == {"x": 60, "y": 38} and moved["is_dirty"]
            assert moved["revision"] == receipt["revision"] > before["revision"]
            host_command(2, "capture")
            wait_for(lambda: (directory / "frame-2.png").exists())
            undone = host_command(3, "undo")
            assert position(undone) == {"x": 40, "y": 28} and not undone["is_dirty"]
            assert inspect() == undone
            redone = host_command(4, "redo")
            assert position(redone) == {"x": 60, "y": 38} and redone["is_dirty"]
            reopened = host_command(5, "save_reopen")
            assert position(reopened) == {"x": 60, "y": 38} and not reopened["is_dirty"]
            assert reopened["session_id"] != before["session_id"]
            assert inspect() == reopened
            saved = cli("project", "inspect", project)["document"]
            assert saved == reopened["project"]["document"]
            host_command(6, "capture")
            wait_for(lambda: (directory / "frame-6.png").exists())
            for number in [2, 6]:
                png = (directory / f"frame-{number}.png").read_bytes()
                assert png[:8] == b"\x89PNG\r\n\x1a\n"
                width, height = struct.unpack(">II", png[16:24])
                assert width >= 640 and height >= 480, (width, height)
            # 마지막 ack와 종료가 경쟁하므로 close는 정상 종료 자체로 확인한다.
            (directory / "host-command.json").write_text(json.dumps({"id": 7, "action": "close"}),
                                                       encoding="utf-8")
            assert process.wait(timeout=min(5, max(0.1, deadline - time.monotonic()))) == 0
            assert not connection.exists(), "Native GUI did not clean its connection descriptor"
        summary = {"result": "passed", "boundary": "native eframe + renderer + external CLI",
                   "physical_ui_input_tested": False, "observations": observations}
        (directory / "native-summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2),
                                                     encoding="utf-8")
        print("Native GUI↔CLI: frame·실편집·GUI undo/redo·저장/재열기·종료 정리 통과")
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            process.wait(timeout=5)


if __name__ == "__main__":
    main()
