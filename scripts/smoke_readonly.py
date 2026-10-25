"""실제 CLI 프로세스의 읽기 계약과 입력 파일 보존을 확인한다."""

import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import zlib


ROOT = Path(__file__).resolve().parents[1]
EXECUTABLE = ROOT / "target" / "debug" / ("engine-cli.exe" if os.name == "nt" else "engine-cli")


def invoke(*arguments, error=None):
    result = subprocess.run(
        [str(EXECUTABLE), *map(str, arguments)], capture_output=True,
        encoding="utf-8", check=False, timeout=10,
    )
    response = json.loads(result.stdout)
    assert response["protocol_version"] == 1, response
    assert response["source"] == "disk", response
    if error is None:
        assert result.returncode == 0 and response["status"] == "ok", response
        assert "error" not in response, response
        return response["data"]
    assert result.returncode == 2 and response["status"] == "error", response
    assert response["error"]["code"] == error, response
    assert response["error"]["message"] and "data" not in response, response
    return None


def png_pixel():
    """의존성 없이 유효한 1×1 RGBA PNG fixture를 만든다."""
    def chunk(kind, content):
        return (struct.pack(">I", len(content)) + kind + content
                + struct.pack(">I", zlib.crc32(kind + content)))
    return (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(b"\x00\xff\x00\xff\xff"))
            + chunk(b"IEND", b""))


def hashes(directory):
    return {str(path.relative_to(directory)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in directory.rglob("*") if path.is_file()}


def main():
    with tempfile.TemporaryDirectory(prefix="engine-readonly-") as temporary:
        directory = Path(temporary) / "한글 경로 with spaces"
        (directory / "assets").mkdir(parents=True)
        (directory / "assets" / "도트 그림.png").write_bytes(png_pixel())
        document = {
            "schema_version": 1, "name": '검토용 "프로젝트"',
            "logical_width": 320, "logical_height": 180,
            "scene": {"entities": [{"id": 73, "asset_id": 19,
                      "name": '주인공 "별"', "position": {"x": -12, "y": 38}}]},
            "assets": [{"id": 19, "path": "assets/도트 그림.png", "width": 1, "height": 1}],
        }
        project = directory / "내 프로젝트.json"
        project.write_text(json.dumps(document, ensure_ascii=False), encoding="utf-8")
        malformed = directory / "잘못된 JSON.json"
        malformed.write_text("{", encoding="utf-8")
        future = directory / "미래 버전.json"
        future.write_text(json.dumps({**document, "schema_version": 999}), encoding="utf-8")
        invalid = directory / "잘못된 참조.json"
        invalid.write_text(json.dumps({**document, "assets": []}), encoding="utf-8")
        before = hashes(directory)

        data = invoke("project", "inspect", project)
        # Windows의 canonical 경로는 확장 길이 접두사(\\?\)를 포함할 수 있다.
        assert os.path.samefile(data.pop("project_file"), project), data
        assert data == {"document": document}, data
        scene = invoke("scene", "inspect", project)
        assert os.path.samefile(scene.pop("project_file"), project), scene
        assert scene == {
            "project_name": document["name"],
            "logical_width": 320, "logical_height": 180,
            "scene": document["scene"], "assets": document["assets"],
        }, scene
        for path in [malformed, future, invalid]:
            invoke("project", "inspect", path, error="invalid_project")
            invoke("scene", "inspect", path, error="invalid_project")
        invoke("project", "inspect", directory / "없는 파일.json", error="project_not_found")
        invoke("scene", "inspect", error="invalid_arguments")
        assert hashes(directory) == before, "읽기 CLI가 입력 파일이나 디렉터리를 변경했습니다."
    print("읽기 CLI 계약 10건 통과: 한글/공백·ID/좌표/자산·오류·파일 hash 보존")


if __name__ == "__main__":
    main()
