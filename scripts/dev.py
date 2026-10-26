"""macOS와 Windows에서 같은 명령으로 개발한다. Python 표준 라이브러리만 사용한다."""

import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tomllib
import urllib.error
import urllib.request


ROOT = Path(__file__).resolve().parents[1]
TOOLS = ROOT / ".tools"
SUFFIX = ".exe" if os.name == "nt" else ""
TOOLCHAIN = tomllib.loads((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))[
    "toolchain"
]["channel"]


def development_environment():
    """프로젝트 안에 설치한 Rust를 우선 사용한다. 셸 설정은 변경하지 않는다."""
    environment = os.environ.copy()
    local_bin = TOOLS / "cargo" / "bin"
    if (local_bin / f"rustup{SUFFIX}").exists():
        environment["CARGO_HOME"] = str(TOOLS / "cargo")
        environment["RUSTUP_HOME"] = str(TOOLS / "rustup")
        environment["PATH"] = str(local_bin) + os.pathsep + environment.get("PATH", "")
    environment["CARGO_TARGET_DIR"] = str(ROOT / "target")
    return environment


def run(command, **options):
    print("+ " + " ".join(map(str, command)), file=sys.stderr, flush=True)
    return subprocess.run(
        command, cwd=ROOT, env=development_environment(), check=True, **options
    )


def cargo(*arguments):
    executable = shutil.which("cargo", path=development_environment().get("PATH"))
    if executable is None:
        raise RuntimeError("Rust가 없습니다. 먼저 python3 scripts/dev.py setup을 실행하세요.")
    return run([executable, *arguments])


def setup():
    """공식 rustup 설치기를 내려받아 .tools/ 안에만 설치한다."""
    machine = platform.machine().lower()
    architecture = {
        "arm64": "aarch64",
        "aarch64": "aarch64",
        "amd64": "x86_64",
        "x86_64": "x86_64",
    }.get(machine)
    operating_system = {
        "Darwin": "apple-darwin",
        "Windows": "pc-windows-msvc",
    }.get(platform.system())
    if architecture is None or operating_system is None:
        raise RuntimeError("자동 설치 대상은 macOS와 Windows의 ARM64/x64입니다.")

    local_rustup = TOOLS / "cargo" / "bin" / f"rustup{SUFFIX}"
    if not local_rustup.exists():
        installer = TOOLS / f"rustup-init{SUFFIX}"
        url = f"https://static.rust-lang.org/rustup/dist/{architecture}-{operating_system}/rustup-init{SUFFIX}"
        TOOLS.mkdir(exist_ok=True)
        print(f"공식 설치기 다운로드: {url}", flush=True)
        with urllib.request.urlopen(url, timeout=30) as response:
            with installer.open("wb") as destination:
                shutil.copyfileobj(response, destination)
        installer.chmod(0o755)

        environment = os.environ.copy()
        environment["CARGO_HOME"] = str(TOOLS / "cargo")
        environment["RUSTUP_HOME"] = str(TOOLS / "rustup")
        subprocess.run(
            [str(installer), "-y", "--no-modify-path", "--profile", "minimal",
             "--default-toolchain", TOOLCHAIN],
            cwd=ROOT,
            env=environment,
            check=True,
        )

    run([str(local_rustup), "toolchain", "install", TOOLCHAIN, "--profile", "minimal",
         "--component", "rustfmt", "--component", "clippy"])
    # Cargo.lock은 실제 의존성 해석 결과로 생성한다. 수작업으로 만들지 않는다.
    cargo("fetch")
    print("Rust와 의존성 준비 완료. 이제 doctor와 check를 실행하세요.")


def doctor():
    environment = development_environment()
    print(f"환경: {platform.system()} / {platform.machine()}")
    print(f"Python: {platform.python_version()} / Rust 지정 버전: {TOOLCHAIN}")
    missing = []
    for tool in ["rustup", "rustc", "cargo", "rustfmt", "cargo-clippy"]:
        executable = shutil.which(tool, path=environment.get("PATH"))
        print(f"{tool}: {executable or '설치 필요'}")
        if executable is None:
            missing.append(tool)

    if missing:
        raise RuntimeError("Rust 도구 설치가 필요합니다: python3 scripts/dev.py setup")
    cargo("--version")
    print("도구 경로 확인 완료. 빌드 가능 여부는 check로 별도 확인하세요.")


def smoke_cli():
    """프로세스 경계에서 JSON 계약과 오류 종료 코드를 확인한다."""
    executable = ROOT / "target" / "debug" / f"engine-cli{SUFFIX}"
    cases = [
        (["info"], 0, {"schema_version": 1, "ok": True, "logical_width": 320,
                       "logical_height": 180, "ticks_per_second": 60}),
        (["viewport", "1001", "701"], 0,
         {"schema_version": 1, "ok": True, "x": 20, "y": 80,
          "width": 960, "height": 540, "scale": 3}),
        (["viewport", "319", "180"], 2,
         {"schema_version": 1, "ok": False, "error": "window_too_small"}),
        (["viewport", "-1", "180"], 2,
         {"schema_version": 1, "ok": False, "error": "invalid_window_size"}),
        (["unknown"], 2,
         {"schema_version": 1, "ok": False, "error": "usage_info_or_viewport_width_height"}),
    ]
    for arguments, expected_code, expected_json in cases:
        result = subprocess.run(
            [str(executable), *arguments], cwd=ROOT, capture_output=True, text=True
        )
        if result.returncode != expected_code or json.loads(result.stdout) != expected_json:
            raise RuntimeError(f"CLI 계약 검증 실패: {arguments!r}")
    print(f"CLI 계약 {len(cases)}건 통과")


def main():
    # Windows의 비 UTF-8 콘솔·리다이렉션에서도 한국어 진단을 출력한다.
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=["setup", "doctor", "fmt", "check", "test", "editor", "run", "info", "viewport",
                 "project", "scene", "session", "build"],
    )
    # 하위 CLI의 --connection-file 같은 옵션을 이 파서가 가로채지 않는다.
    parser.add_argument("arguments", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    if arguments.command not in ["viewport", "project", "scene", "session"] and arguments.arguments:
        parser.error("추가 인자는 viewport/project/scene/session에서만 사용합니다.")
    if arguments.command == "viewport" and len(arguments.arguments) != 2:
        parser.error("사용법: viewport WIDTH HEIGHT")
    if arguments.command in ["project", "scene"] and (
        len(arguments.arguments) != 2 or arguments.arguments[0] != "inspect"
    ):
        parser.error(f"사용법: {arguments.command} inspect FILE")
    if arguments.command == "session" and (
        len(arguments.arguments) != 3 or arguments.arguments[:2] != ["inspect", "--connection-file"]
    ):
        parser.error("사용법: session inspect --connection-file FILE")

    if arguments.command == "setup":
        setup()
    elif arguments.command == "doctor":
        doctor()
    elif arguments.command == "fmt":
        cargo("fmt", "--all")
    elif arguments.command == "check":
        cargo("fmt", "--all", "--", "--check")
        cargo("clippy", "--locked", "--workspace", "--all-targets", "--", "-D", "warnings")
        cargo("test", "--locked", "--workspace")
        cargo("build", "--locked", "--workspace")
        smoke_cli()
        run([sys.executable, str(ROOT / "scripts" / "smoke_readonly.py")])
    elif arguments.command == "test":
        cargo("test", "--locked", "--workspace")
    elif arguments.command == "editor":
        cargo("run", "--locked", "-p", "engine-editor")
    elif arguments.command == "run":
        cargo("run", "--locked", "-p", "engine-desktop")
    elif arguments.command in ["info", "viewport", "project", "scene", "session"]:
        cargo(
            "run", "--locked", "--quiet", "-p", "engine-cli", "--",
            arguments.command, *arguments.arguments,
        )
    elif arguments.command == "build":
        cargo("build", "--locked", "--release", "--workspace")


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.CalledProcessError, OSError, urllib.error.URLError) as error:
        print(f"오류: {error}", file=sys.stderr)
        sys.exit(1)
