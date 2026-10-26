# 개발 및 실행 검증 기록

기준일: 2026-10-25. macOS ARM64 환경입니다.
소스 작성·도구 확인·기존 바이너리 실행·현재 소스 빌드·화면 확인을 구분합니다.

## 초기 환경 확인 이력

초기에는 Rust 도구가 없었고 공식 다운로드 서버의 DNS 조회가 실패했습니다.
당시 설치 오류는 `nodename nor servname provided, or not known`, curl 오류는
`Could not resolve host: static.rust-lang.org`였습니다.
Python 3.14.8, Apple clang 21.0.0과 설정 파일 문법·개발 명령의 기본 처리를 확인했습니다.
이 기록은 과거 상태이며 현재 도구 설치 상태를 뜻하지 않습니다.

## 기반 PR 시점의 검증 이력

원본 작업 폴더의 사용자 파일을 보존하고, 원격 main의 `f7d2a2fc` 기준 격리 작업 폴더에서 확인했습니다.
Rust 1.99.0 도구 체인과 기존에 다운로드된 의존성을 사용했습니다.
아래 표는 GUI 구현 전 기반 PR의 결과이며, 이후 GUI와 최종 편집 검증은 아래 단계별 기록을 따릅니다.

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

## GUI 구현 전에 정한 확인 항목

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
프로젝트·PNG 흐름의 실제 UI 확인 결과는 아래와 같이 별도로 기록합니다.

이후 실제 macOS/Retina 200%에서 native Save로 JSON 생성, native Open으로 PNG 가져오기,
장면 배치, Cmd+S와 dirty 해제, 초기 상태에서 저장 JSON 재열기와 픽셀 복원까지 화면으로 확인했습니다.
검토 폴더의 project.json은 schema_version 1, 320×180, 개체 1개(asset_id 1, 위치152,82)이며,
assets/asset-1.png는 16×16, 96바이트로 원본 fixture와 바이트가 같습니다.
저장된 프로젝트 이름은 기본값인 ‘새 프로젝트’여서 사용자 지정 한글 입력은 검증하지 않았습니다.
처음에는 Cmd+Q 뒤 초기 창만으로 종료를 단정하지 않았고, 후속 inventory 조회로 중간 앱의 실제 종료와 도구 재실행을 구분했습니다.
PR6의 새 head `0ecb1bc`는 이전 `cdfbf2f`와 전체 tree가 같고,
[일반 CI](https://github.com/qorcjftns/game-engine-dots/actions/runs/40183279370)는 Mac·Windows 모두 통과했습니다.
Windows 네이티브 실행과 DPI별 화면 검증은 아직 수행하지 않았습니다.

## 최종 선택·속성·드래그·실행 취소 연결

장면/캔버스 선택, 이름·정수 좌표 draft, 프레임 끝의 공통 명령 처리, 드래그 프리뷰와 종료 시 1회 커밋,
undo/redo, 삭제와 저장/버리기/취소 전환을 연결했습니다.
입력 중인 이름도 저장 전에 확정하고, 실패한 저장은 pending 전환과 작업을 보존합니다.

macOS ARM64의 전체 offline check가 통과했습니다: 기존 코어 8개, 문서 14개, GUI 18개,
doctest 1개와 CLI 계약 5개, rustfmt, Clippy 경고 없음과 workspace 빌드입니다.
GUI 회귀는 실제 egui RawInput의 다중 이동/놓기와 undo 1회, 텍스트 입력 중의 단축키,
취소한 미확정 draft 보존, 저장 실패, 정수 좌표 한계와 캐시 갱신을 검사합니다.

중간 앱의 실제 QA에서는 일반 빨간 창 닫기는 dirty 문서를 보호했지만 Cmd+Q는 기본 winit Quit 경로로 우회했습니다.
최종 코드는 Mac 기본 Quit 메뉴를 끄고 egui 기본 Quit 단축키도 비활성화한 뒤,
Cmd+Q를 메뉴/창 닫기와 같은 미저장 전환 처리로 연결했습니다.
소스 `d772d78`의 [일반 CI](https://github.com/qorcjftns/game-engine-dots/actions/runs/40184859990)는
Mac·Windows 모두 통과했습니다. macOS Dock Quit·운영체제 강제 kill은 보호 범위 밖이며,
Windows 네이티브/DPI 검증은 아직 미확인입니다.

## 최종 실제 UI 확인 결과

최종 편집 바이너리의 임시 macOS 앱(CFBundleVersion 3)을 네이티브 UI 경로에서 검토했습니다.
Retina 200% 배율에서 다음 흐름을 확인했습니다.

1. 개체를 선택하고 한글 이름·좌표(40,28)를 입력했습니다. 이름 필드에 포커스가 있는 상태의 Cmd+S도 최신 입력을 저장했습니다.
2. 실제 드래그로 (60,38)로 옮긴 뒤 Cmd+Z 한 번으로 (40,28)와 저장 상태가 복원됐고, redo로 (60,38)를 복원했습니다.
3. Cmd+Q 확인창의 취소는 현재 작업을 보존했습니다. 이름 필드에 ‘저장 재열기 검토’를 입력한 상태의 Cmd+Q와 취소도 입력을 보존했습니다.
4. Cmd+Q의 ‘저장 후 계속’은 입력을 확정·저장하고 종료했습니다. 앱 inventory에서 프로세스가 없음을 확인한 뒤 재실행·native Open으로 JSON을 다시 열었습니다.
5. 한글 개체 이름 ‘저장 재열기 검토’, 좌표(60,38), PNG 1개와 dirty 없음이 복원됐고 화면도 확인했습니다.
6. 이름 draft를 ‘버릴 임시 이름’으로 바꾼 뒤 빨간 창 닫기 → ‘변경 버리기’로 실제 종료를 확인했습니다. 재실행·재열기 시 원래 저장된 이름·좌표·PNG가 유지됐습니다.

검토 파일 `project.json`을 독립적으로 읽어 schema_version 1, 320×180, 개체 1개와
상대 경로 `assets/asset-1.png`를 확인했습니다. PNG는 16×16, 96바이트이며 원본 fixture와 바이트가 같습니다.
현재 사용자 검토용 최종 앱과 이 프로젝트를 열어 둔 상태입니다.

이 결과는 macOS의 에디터 메뉴·창 닫기·Cmd+Q 경로에 대한 확인입니다.
Windows 실제 GUI/DPI, macOS Dock Quit와 강제 종료까지 보호했다고 해석하지 않습니다.
이 M1 검토 시점의 CLI는 info·viewport만 구현돼 있었고 문서 편집 명령과 MCP 전송은 구현 전이었습니다.

## M2 PR1: 공통 응답 계약과 읽기 CLI

`project inspect FILE`과 `scene inspect FILE`은 저장된 JSON과 문서 모델을 검사하고
실제 개체 ID·이름·좌표·자산 참조를 반환합니다. 응답의 `protocol_version: 1`은
프로젝트 형식의 `schema_version`과 별개이며 `source: "disk"`로 조회 대상을 구분합니다.
PNG 파일의 존재/디코딩이나 저장 전 GUI 문서를 검사하는 기능은 아닙니다.

macOS ARM64에서 `CARGO_NET_OFFLINE=true python3 scripts/dev.py check`를 실행했습니다.
rustfmt, workspace Clippy 경고 없음, 테스트 42개(기존 40개와 공통 응답 계약 2개),
doctest 1개, 전체 빌드, 기존 CLI 계약 5개와 신규 읽기 CLI 계약 10개가 통과했습니다.
새 외부 의존성 버전 추가 없이 실제 Cargo 해석으로 workspace lockfile을 갱신했습니다.

신규 검사는 별도 CLI 프로세스로 한글·공백 경로와 따옴표 이름, 실제 ID·음수 좌표·자산 메타데이터,
잘못된 JSON·미지원 형식 버전·없는 자산 참조·없는 프로젝트·인자 오류를 확인합니다.
검사 전후 입력 디렉터리의 모든 파일 SHA-256과 파일 목록이 같았습니다.
Windows의 canonical 경로 표현 차이는 같은 파일인지 비교하도록 처리했으며 실제 Windows 실행 결과는 CI에서 별도로 확인합니다.

이 PR은 GUI 파일을 변경하지 않았으므로 실제 UI 검사를 반복하지 않았습니다.
live 조회·편집, 세션 revision/busy/중복 요청 처리와 MCP 연결은 후속 PR 범위입니다.

첫 Windows CI는 Rust 검사와 신규 CLI 검증을 마친 뒤 스모크의 한글 완료 메시지를
cp1252로 출력하면서 실패했습니다. 별도 Python 프로세스에도 stdout/stderr UTF-8을 명시했고,
macOS에서 `PYTHONIOENCODING=cp1252 python3 scripts/smoke_readonly.py`로 10건과 출력 성공을 확인했습니다.
수정된 commit의 Windows 결과는 후속 CI에서 확인합니다.

PR1 수정 `3ec5b24`의 [자동 CI](https://github.com/qorcjftns/game-engine-dots/actions/runs/40205152906)는
macOS와 Windows 모두 통과했습니다. 읽기 CLI의 실제 Windows 프로세스 검사와 한글 출력도 포함합니다.

## M2 PR2: opt-in GUI live 조회

에디터를 `--agent-control --connection-file FILE`로 실행하면 loopback 조회 통로를 엽니다.
`session inspect --connection-file FILE`은 디스크 대신 GUI가 게시한 확정 문서·session ID·revision·busy·dirty·이력 상태를 읽습니다.
미확정 이름 draft·드래그 프리뷰는 문서에 포함하지 않으며 재열기는 새 session ID를 사용합니다.
기본 실행은 통로를 열지 않고 프로젝트 안의 연결 파일과 기존 파일 덮어쓰기를 거부합니다.

macOS ARM64 로컬에서는 loopback bind와 connect가 모두 sandbox의 `EPERM`으로 차단됐습니다.
검사를 조용히 생략하지 않고 `cargo test --locked --workspace -- --skip tcp_`로 부분 범위를 명시했습니다.
이 결과는 non-TCP 테스트 50개와 doctest 1개 통과입니다. workspace rustfmt·Clippy(all targets, 경고 오류 처리),
locked build, 기존 CLI 계약 5건·디스크 읽기 CLI 10건도 통과했습니다.
개발 스크립트의 `session inspect --connection-file` 인자 전달과 없는 연결 파일의 구조화된 오류도 확인했습니다.

실제 TCP 회귀 7개는 로컬 통과로 세지 않았습니다. 기본 `scripts/dev.py check`에는 제외 옵션이 없으므로
두 OS CI에서 transport·별도 CLI 프로세스·GUI core 게시의 실제 socket 검사를 필수로 실행합니다.
인증/크기/잘못된 요청·전체 frame 기한·느린 전송·bounded 종료·연결 파일 정리/외부 변경 보존,
저장 파일과 다른 live 상태 조회 및 GUI draft/commit·재열기 연결을 검사합니다.
원격 검사 결과와 별도 macOS QA 앱의 실제 화면/종료 확인은 확인 후 추가합니다.

live 변경·중복 요청 결과·MCP stdio는 후속 PR 범위입니다.

첫 PR2 CI는 양 OS에서 slow-drip 회귀의 EOF 가정 때문에 실패했습니다.
서버의 전체 기한이 지난 뒤 테스트가 추가 송신하여 발생한 TCP reset/aborted를 허용하지 않았습니다.
수정 검사는 실제 닫힘까지의 1.8~4초 상·하한, 추가 송신 없는 frame의 구조화된 기한 오류,
새 연결 조회 성공과 bounded 종료를 유지합니다. 로컬에서는 socket 실행 대신 테스트 컴파일을 확인하고
실제 수정 결과는 후속 CI에서 확인합니다.

별도 앱 `dev.engine1.editor.m2review` 버전 1의 macOS/Retina 200% 실제 UI를 확인했습니다.
opt-in 연결 표시와 연결 파일 존재·0600 권한, native Open으로 프로젝트/PNG 표시,
한글 이름 입력 후 Cmd+S 저장, 메뉴 undo의 dirty와 redo의 clean 복원을 확인했습니다.
이름 draft를 입력한 Cmd+Q에서 취소하면 초안이 보존됐고, 다시 ‘변경 버리기’로 종료한 뒤
앱 inventory에 프로세스가 없고 연결 파일도 삭제된 것을 확인했습니다.
독립 파일 검사에서 마지막 저장 이름 ‘M2 연결 중 저장 검토’가 유지되고 ‘M2 버릴 초안’은 없었습니다.
최소화 동작과 실제 GUI↔CLI 교차 조회, Windows native/DPI는 이번 UI 확인에 포함하지 않습니다.

후속 양 OS CI에서 transport 회귀는 통과했으며 GUI core 회귀의 fixture draft 동기화 누락이 확인됐습니다.
테스트가 직접 명령을 적용한 뒤 실제 GUI commit 경로와 같이 draft를 동기화하도록 보완했습니다.
미동기화 draft가 Undo 전에 추가 명령으로 확정되는 테스트 설정 문제이며 production 편집 규칙은 바꾸지 않았습니다.

## M2 PR3: 같은 세션의 편집·이력·저장

CLI의 `session mutate --connection-file FILE --request-file JSON`과 `session result`를 연결했습니다.
GUI가 원본 세션을 계속 소유하며 추가/삭제·이름/위치·undo/redo·저장을 같은 명령 경로에서 실행합니다.
session ID·예상 revision·busy를 수신 시점과 적용 직전에 검사하고 사람의 draft를 강제로 확정하지 않습니다.
요청 ID별 원장은 같은 요청의 결과를 보존하고 다른 내용의 재사용을 거부합니다.
대기 기한이 지난 요청은 적용하지 않으며 실행이 시작된 저장은 대기 기한으로 취소하지 않습니다.

macOS ARM64에서 fmt, workspace Clippy(all targets, 경고 오류 처리), locked build,
기존 CLI 5건·디스크 읽기 CLI 10건을 통과했습니다.
`cargo test --locked --workspace -- --skip tcp_`는 non-TCP 58개와 doctest 1개가 통과했습니다.
대기/원장 상한, 중복·다른 내용 충돌·같은 revision 경합·busy 재검사·만료·세션 교체·실패한 저장 보존을 포함합니다.
실제 TCP 8개는 로컬 sandbox 제한 때문에 이 통과 수에 포함하지 않으며 두 OS CI에서 필수 실행합니다.

추가 CI 명령 `python scripts/dev.py native-smoke`는 테스트 전용 example의 실제 eframe main-thread 창과
Wgpu renderer를 띄워 별도 CLI 프로세스가 같은 세션을 조회·편집하도록 시도합니다.
이후 같은 GUI action의 undo/redo·저장/재열기·정상 종료와 descriptor 정리를 확인하도록 작성했습니다.
30초 watchdog 또는 graphics/session 실패는 실패로 처리하며 skip으로 통과시키지 않습니다.
실행물과 orchestration은 컴파일/구문 검사 및 읽기 리뷰를 마쳤지만 실제 CI 실행은 아직 확인 전입니다.
PNG는 renderer의 실제 frame 캡처이며 물리 마우스 입력·OS 전체 화면·DPI 검증과 구분합니다.
artifact는 frame PNG·비밀값 없는 결과 JSON·host 로그만 포함하고 token 연결 파일·요청 파일은 제외합니다.

native example용 dev-dependency는 이미 잠긴 image 0.25.10과 serde_json을 재사용했습니다.
MCP stdio와 실제 SDK 연결은 다음 PR 범위입니다.

첫 native CI는 양 OS에서 실제 창/renderer와 CLI 편집·GUI undo/redo·저장/재열기까지 진행했습니다.
첫 frame PNG는 완전한 파일이었으나 두 번째 캡처가 생성 직후 0바이트일 때 orchestration이 읽어 실패했습니다.
캡처를 명시적 PNG 형식의 임시 파일에 완성한 뒤 rename하여 게시하도록 보완했습니다.
수정 후 전체 native 성공·정상 종료/정리는 새 CI 결과로 확인하며 첫 시도를 통과로 기록하지 않습니다.
첫 renderer PNG는 Mac 1024×653·Windows 1024×768, 배율 100%에서 사람이 직접 확인했습니다.
한글·dirty 표시·좌표(60,38)·PNG 리소스와 뷰포트의 실제 magenta 픽셀 위치가 일치했습니다.
이는 CI의 첫 이동 frame 검토이며 Windows 실기기/DPI 전체 인수를 뜻하지 않습니다.

PR3 수정 `506c239`의 [자동 CI](https://github.com/qorcjftns/game-engine-dots/actions/runs/40254179598)는 양 OS 모두 통과했습니다.
실제 TCP 회귀와 native eframe/renderer↔별도 CLI 편집, GUI action undo/redo·저장/재열기,
정상 프로세스 종료·descriptor 삭제까지 script assertion과 `native-summary.json`의 passed 결과로 확인했습니다.
사람이 최종 frame-6 PNG도 검토해 dirty 해제·저장 완료 상태, 좌표(60,38)와 실제 magenta 픽셀,
PNG 리소스 유지 및 한글 UI의 잘림 없음을 확인했습니다. 종료/정리는 PNG만으로 판단하지 않았습니다.
이 단계까지 실제 OS 최소화 상태와 MCP 교차는 미검증이므로 다음 PR에서 별도로 시도합니다.

## M2 PR4: 공식 MCP stdio와 교차 검증

공식 rmcp 3.7.0 SDK로 session_inspect·session_mutate·session_result를 제공하며
CLI와 같은 control client·요청 원장을 사용합니다. stdout은 SDK wire만 출력하고
조회/접수 응답과 편집 terminal state를 구분합니다.

신규 SDK 의존성은 정상 CI의 Cargo가 생성하고 실제 컴파일한 lockfile을 반영했습니다.
초기 bootstrap은 양 OS에서 SDK 포함 검사를 통과한 뒤 예상한 lockfile 변경 검사만 실패했습니다.
공개 registry만 임시 checkout에 복원했으며 임시 bootstrap workflow 단계는 제거했습니다.

macOS ARM64 offline/locked 통합 검사에서 non-TCP 64개와 doctest 1개,
workspace rustfmt·Clippy(all targets, 경고 오류 처리), 전체/네이티브 example 빌드와 기존 CLI 15건이 통과했습니다.
실제 SDK stdio 별도 프로세스의 initialize·tools/list·tools/call, stdout JSON 전용,
typed 입력·구조화된 연결 오류와 추가 필드 거부를 포함합니다.
검사 중 발견한 serde unit 명령의 추가 필드 무시 문제는 공통 strict wire Deserialize로 보완해
CLI와 MCP 모두 같은 입력 규칙을 유지했습니다.

실제 TCP 9개는 로컬 sandbox 때문에 이 통과 수에 포함하지 않으며 정상 양 OS CI에서 필수 실행합니다.
`python scripts/dev.py native-mcp-smoke`는 실제 창의 OS 최소화 상태를 관측한 뒤
공식 SDK 프로세스의 변경을 CLI에서 재조회하고 같은 receipt/replay를 확인하도록 작성했습니다.
최소화 중 logic의 문서 변경을 관측하고 실제 restore 상태를 확인한 후
같은 GUI undo/redo·저장/재열기·frame 캡처·정상 종료/descriptor 정리까지 검사합니다.
OS 상태가 관측되지 않거나 30초를 넘으면 실패합니다. 실제 실행은 CI 결과로 별도 갱신합니다.
이 test-only adapter의 GUI action 전달은 물리 마우스 입력 및 여러 DPI 인수와 구분합니다.

마지막 SDK lifecycle 보강에서는 초기화 지원 최신 2025-11-25, 이전 2024-11-05의 협상과
최신 2026-07-28의 초기화 없는 SDK list/call 경로를 각각 실제 stdio 프로세스로 통과했습니다.
native 교차 클라이언트는 초기화를 지원하는 2025-11-25를 명시합니다.
기존 통합 64개에 새 SDK 호환 2개를 더한 최종 로컬 non-TCP 검사 범위는 66개입니다.

## M2 최종 통합 결과와 병합 상태

구현 commit `015fa3b`의 [정상 CI](https://github.com/qorcjftns/game-engine-dots/actions/runs/40255432022)는
macOS·Windows 모두 통과했습니다. Rust 테스트 75개(실제 TCP 9개 포함)와 doctest 1개,
CLI 계약 15건, 포맷·Clippy·빌드·lockfile 불변 검사 및 실제 native GUI↔CLI↔공식 MCP 교차를 포함합니다.

두 OS에서 CLI 이동 후 MCP 이름 변경을 같은 GUI에 적용하고 CLI 결과 조회·동일 ID replay를 확인했습니다.
단순 최소화 요청이 아니라 실제 `viewport.minimized=true`를 기다린 뒤 변경했습니다.
Mac은 UI frame 12, Windows는 frame 7이 그대로인 최소화 상태에서 revision 1→2와 새 이름을 관측했습니다.
실제 복원 false를 확인한 뒤 같은 GUI action undo/redo는 revision 3/4를 만들었고,
저장/재열기는 새 session ID·revision 0·clean 상태를 복원했습니다.
정상 host 종료·연결 파일 삭제와 MCP 교차 결과는 script assertion·summary의 passed 기록으로 확인했습니다.

사람이 양 OS 최종 frame-8 PNG를 직접 검토해 ‘MCP 공동 편집’ 이름·좌표(60,38), dirty 해제,
PNG 리소스와 실제 magenta 픽셀, 한글 표시와 잘림 없음을 확인했습니다.
이것은 정상 CI의 실제 native 창/renderer와 테스트 전용 GUI action 전달 검증입니다.
물리 마우스 자동화·Windows 실기기·여러 DPI·macOS Dock Quit/강제 종료 보호를 완료한 것은 아닙니다.

확인 시 원격 main은 `dd5a12bb`이고 PR #11~#14는 모두 OPEN입니다.
M2는 구현 브랜치에서 검증된 상태이며 main에 병합된 것으로 기록하지 않습니다.
PR #11은 main 대상 ready, #12→#13→#14는 선행 브랜치를 base로 한 Draft이며 병합/자동 병합을 수행하지 않았습니다.
최종 README·설계·이 기록 정리는 문서만 변경하며 실행 코드·manifest·lock·CI는 검증한 `015fa3b`와 같습니다.

## M3 구현과 검증 (2026-10-26)

M3는 #16 snapshot/공통 출력 → #17 Play/input → #18 독립 player → runtime 제어/회귀 PR의 stack이다.
M2 최종 `050a8da`가 `feat/shared-editor-live-query`에 있으며 main에는 아직 선행 코드 전체가 반영되지 않았다.
따라서 구현·검증과 main 병합 완료는 구분하며 이 PR들은 선행 병합을 기다리는 Draft이다.

### 실제 실행한 검사

- macOS ARM64에서 `python3 scripts/dev.py check`: rustfmt·Clippy `-D warnings`·Rust 테스트 85개와 doctest 1개·workspace build·CLI 계약 5+10건 통과.
- 실제 native editor fixture에서 dirty 편집 → Play → Pause 정지 → 정확 73 tick → 실행 중 undo 거부 → Stop → undo 원복 → 재시작 run ID 교체. JSON·PNG SHA256 보존.
- `python3 scripts/native_runtime_smoke.py`: 실제 editor 창과 외부 CLI/MCP의 start/pause/73 tick/동일 request ID 재시도/old run 거부/Stop 후 이동과 undo를 검사. 독립 player의 동일 snapshot frame hash를 비교하고 두 native screenshot에서 magenta fixture 픽셀을 확인한다.
- 실제 독립 player 창에서 alpha checker와 좌하단 clipping 표시를 확인했다. headless 73 tick frame hash `26f080ad36e9e825`는 같은 snapshot의 editor와 일치했다.
- CUA로 실제 canvas 클릭 뒤 D/Space 각각 약 1.5초 입력, 458개 CLI 관측에서 right/primary/pointer를 확인하고 release 후 버튼 해제를 확인했다.
- 실제 TextEdit에 `ad`를 입력한 상태에서 원격 Play가 `editor_busy`를 반환하고 runtime은 null인 것을 확인했다.
- raw egui regression으로 같은 frame의 key/button press-release, 화면 밖 pointer, focus/input 차단을 검사했다. 실제 키를 누른 채 OS focus를 잃는 조작은 별도 미확인이다.
- `behavior_probe`의 정상 계산·문법 오류·무한 반복 budget 중단·실행 오류를 실행했다. 제품 행동 언어가 구현됐다는 뜻은 아니다.

[editor 실제 화면](images/m3-editor-play-macos.png), [독립 player 실제 화면](images/m3-player-macos.png).
네이티브 smoke의 control 조작은 테스트 host와 같은 production EditorApp 경로를 사용하며 물리 입력 시험과 구분한다.
초기 검증에서 발견한 Play 검정 화면은 panel scope를 수정하여 해결했고 native screenshot의 fixture 색 검사로 회귀를 막는다.

### 성능 기준선과 남은 범위

macOS 27.0.1 ARM64, debug 빌드, 320×180, 16×16 alpha sprite 두 개의 기준선:
약 19.959초 동안 1198 tick(60.023Hz), 1123 표시 요청(56.265Hz). frame은 GPU 완료 측정값이 아니다.
player 프로세스 시작+load+100000 tick의 5회 wall time은 약 770.7/13.65/11.60/10.79/9.83ms로 cold/warm 차이가 컸다.
해당 headless 측정의 max RSS는 10,551,296 bytes, peak footprint는 3,293,712 bytes였다. native GUI 메모리나 최대 부하의 60Hz 보장으로 해석하지 않는다.

Windows x64와 macOS native 창 smoke는 PR의 GitHub Actions에 등록했다. CI 결과는 실행한 revision에서 별도로 확인하며,
Windows 실제 사용자의 키보드·마우스·DPI, 최소 OS 실기기, Mac Intel/Windows ARM은 아직 미확인이다.
Dock Quit의 취소 보존은 미확인/미지원 경계로 남아 있고 강제 종료 뒤 미저장 복구 checkpoint는 구현하지 않았다.
현재 복구 지점은 마지막 성공 저장본이다. M10의 개발 환경 없는 패키지·서명·공증·외부 배포 인수는 남아 있다.
