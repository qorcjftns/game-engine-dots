# 개발 기반과 계획 검증 기록

기준일: 2026-10-25. macOS ARM64 환경입니다.
소스 작성·도구 확인·기존 바이너리 실행·현재 소스 빌드·화면 확인을 구분합니다.

## 초기 환경 확인 이력

초기에는 Rust 도구가 없었고 공식 다운로드 서버의 DNS 조회가 실패했습니다.
당시 설치 오류는 `nodename nor servname provided, or not known`, curl 오류는
`Could not resolve host: static.rust-lang.org`였습니다.
Python 3.14.8, Apple clang 21.0.0과 설정 파일 문법·개발 명령의 기본 처리를 확인했습니다.
이 기록은 과거 상태이며 현재 도구 설치 상태를 뜻하지 않습니다.

## 현재 기반 검증 결과

원본 작업 폴더의 사용자 파일을 보존하고, 원격 main의 `f7d2a2fc` 기준 격리 작업 폴더에서 확인했습니다.
Rust 1.99.0 도구 체인과 기존에 다운로드된 의존성을 사용했습니다.

| 항목 | 현재 확인 범위 |
| --- | --- |
| 로컬 Rust 도구 | Rust 1.99.0 도구 체인과 rustc·cargo·rustfmt·Clippy 구성 요소 존재 |
| doctor | 통과 |
| 기존 target/debug CLI | info·viewport 정상 JSON 응답 |
| 현재 소스 재빌드 | workspace 빌드 통과 |
| 현재 소스 전체 테스트 | 코어 테스트 8개 통과 |
| Clippy·포맷 | 모든 workspace 대상 Clippy 경고 없음, rustfmt 검사 통과 |
| 새 빌드 CLI 계약 | 정상 응답·잘못된 입력 처리 5개 통과 |
| GUI 에디터 | 아직 구현 전 |
| macOS 실제 렌더링·입력 | 이번 확인에서 검증하지 않음 |
| Windows 빌드·테스트 | windows-2025 원격 CI 통과. 실제 창 출력은 아직 미확인 |
| Cargo.lock | 원본 미추적 파일을 보존하고 동일 내용을 복사. `--locked` 검사 중 변경 없음 |

기존 바이너리의 응답 확인과 별도로, 이번에는 현재 소스를 빌드한 CLI의 계약 검사까지 실행했습니다.
실제 데스크톱 창의 렌더링·입력과 Windows 동작은 이 결과로 대신하지 않습니다.

## GUI 계획 문서 이력

GUI 에디터를 제품의 중심으로 재정의하고, GUI 셸 → 문서/명령·저장/열기 → PNG 배치 →
선택·속성·드래그 → undo/redo·미저장 보호 순서로 정리합니다.
GUI는 OS의 DPI 배율을 따르고, 게임 뷰포트의 콘텐츠만 논리 해상도를 사용합니다.
위 계획 PR은 문서만 변경했습니다. 이후 기반 PR에서 lockfile과 검사·CI 설정을 추가합니다.

## 기반 PR 검사 명령

```sh
python3 scripts/dev.py doctor
CARGO_NET_OFFLINE=true python3 scripts/dev.py check
```

macOS ARM64에서 두 명령이 통과했습니다. 기존 캐시만으로 검사하기 위해 offline 모드를 사용했습니다.
검사·테스트·실행·릴리스 빌드 명령은 `--locked`를 사용하며, lockfile이 맞지 않으면 실패합니다.
원본과 복사본의 Cargo.lock SHA-256은 모두 다음 값입니다.

```text
2f4eb81b8707a6fff39870316a125ac42ec0919df73d202f742dac160a219717
```

GitHub Actions는 `macos-15` ARM64와 `windows-2025` x64에서 도구 준비와 같은 검사를 실행합니다.
runner 라벨은 [GitHub 공식 지원 목록](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)을 확인했습니다.
checkout v7.0.1과 setup-python v7.1.0은 공식 태그의 commit SHA로 고정했습니다.
로컬에서 CI 설정을 작성한 것과 원격 CI가 실제 통과한 것은 별도로 기록합니다.

최초 원격 CI에서 macOS 검사는 통과했고, Windows는 Python의 cp1252 출력 인코딩으로
한국어 설치 안내를 출력하다 실패했습니다. 개발 스크립트의 stdout·stderr를 UTF-8로 고정해
수정하고, 로컬에서 `PYTHONIOENCODING=cp1252` 상태의 doctor·도움말·파이프·파일 리다이렉션·오류 출력을 확인했습니다.
수정 커밋 `e7a54aa`의 [원격 CI](https://github.com/qorcjftns/game-engine-dots/actions/runs/40181094238)는
macOS ARM64와 Windows x64에서 모두 통과했습니다.

## 다음 단계의 확인 항목

실제 GUI가 추가되면 운영체제·화면 크기·DPI·입력·저장/재열기 결과와 캡처를 기록합니다.
Windows에서는 `py -3`를 사용합니다. MCP는 구현 이후 초기화·목록·호출을 따로 검증합니다.

## GUI 셸 검증

`engine-editor`에 eframe 0.37.0/wgpu 기본 창과 패널을 추가했습니다.
로컬 crates.io DNS 조회가 제한돼, 정상 GitHub Actions에서 공식 Cargo로 의존성을 해석하고 빌드했습니다.
[초기 bootstrap](https://github.com/qorcjftns/game-engine-dots/actions/runs/40181487281)의
Mac 검사·빌드가 통과했고, [일반 CI](https://github.com/qorcjftns/game-engine-dots/actions/runs/40181487335)는
Mac·Windows 모두 코드 검사·빌드는 통과했으나 저장소의 기존 lockfile 차이를 검출했습니다.

원격에서 실제 생성한 Cargo.lock을 반영하고, 공개 Cargo registry 데이터만 내려받아
절대 경로·상위 경로 이동·링크가 없는지 검사한 뒤 격리 작업 폴더의 캐시에 복원했습니다.
인증 파일이나 전체 Cargo 홈은 업로드·복원하지 않았으며 원본 작업 폴더는 보존했습니다.
이후 GUI를 포함한 로컬 `CARGO_NET_OFFLINE=true python3 scripts/dev.py check`가 통과했습니다.
임시 bootstrap workflow는 제거하고 일반 `--locked` CI를 유지합니다.

GUI 실행 명령은 `python3 scripts/dev.py editor`, 기존 출력 데모는 `run`입니다.
exec 경로의 실행에서는 hiServices/WindowServer 연결 오류가 있었습니다.
이후 별도 네이티브 UI 경로에서 실제 macOS/Retina 200% 배율의 한글 메뉴·장면/속성/리소스 패널·
빈 뷰포트 안내가 잘림이나 깨짐 없이 표시되는 것을 화면으로 확인했습니다.
이 확인은 초기 셸에 대한 결과이며, Windows 네이티브 실행·DPI 검증이나 후속 편집 동작의 검증은 아닙니다.

## 프로젝트 문서 코어 검증

`engine-document`를 workspace에 연결하고 실제 공개 registry 캐시로 lockfile을 갱신했습니다.
기존 의존성 버전은 유지하며 문서 크레이트와 필요한 JSON 패키지 항목만 추가됐습니다.
macOS ARM64에서 전체 offline check가 통과했습니다: 기존 코어 8개, 문서 IO·편집 회귀 14개,
문서 예제 doctest 1개, Clippy, 포맷, workspace 빌드, CLI 계약 5개입니다.

문서 테스트는 한글 이름·PNG 픽셀 보존·편집/undo/redo·dirty 상태·저장/재열기,
중복 ID·없는 참조·안전하지 않은 경로·잘린 PNG 거부, 외부 수정과 저장 실패 시 기존 파일 보호를 확인합니다.
GUI 타입이나 MCP 전송에 의존하지 않는 API이며, 이 단계에서는 GUI 버튼에 아직 연결하지 않았습니다.

문서 코어 커밋 `b1005e7`의 [원격 CI](https://github.com/qorcjftns/game-engine-dots/actions/runs/40182614965)는
macOS·Windows 모두 통과했습니다.

## 프로젝트·PNG GUI 연결 검증

GUI의 새 프로젝트 폼·열기·PNG 가져오기·장면 배치·저장·재열기를 문서 세션 API에 연결했습니다.
문서 전환과 종료 시 미저장 변경을 차단하는 최소 보호도 적용했습니다.
macOS ARM64에서 전체 offline check가 통과했습니다: 기존8개·문서14개·GUI 래스터/뷰포트6개·
doctest1개, Clippy, 포맷, 전체 빌드와 CLI 계약5개입니다.
래스터 검사는 알파 합성·음수 위치와 화면 밖 클리핑·프로젝트 해상도·물리 픽셀 배율을 확인합니다.

이 검사는 네이티브 파일 선택 대화상자나 실제 GUI에서 저장·재열기한 결과를 대신하지 않습니다.
프로젝트·PNG 흐름의 실제 UI 검토는 최종 선택·속성 편집 연결 후 별도로 수행합니다.
