# M3 runtime와 호스트 경계

M3 시작 snapshot은 확정 문서와 PNG 디코드 결과를 소유한다. GPU 자료·편집 이력·dirty를 소유하지 않는다.
시작 시 자산을 전부 읽지 못하면 실패하며 이미 열린 문서와 저장 파일은 유지한다. 원본에 실행 상태를 반영하지 않는다.
`engine-runtime`의 CPU RGBA 합성과 정수 배율 camera를 editor와 독립 player가 공유한다.
렌더링 순서는 문서 개체 순서, 배경은 불투명, 합성은 정수 alpha 반올림이다.
M3에는 행동 코드가 없어 입력으로 개체를 이동시키지 않는다. 입력 상태와 tick을 관찰하는 기반이다.

이 브랜치의 선행 M2는 `050a8da`이다. GitHub에서 M2 PR은 병합됐으나 main은 PR12 상태이므로
M3 PR은 M2 최종 브랜치를 선행으로 두며 main을 직접 변경하지 않는다.

## 실행 제어와 입력

Play는 확정된 문서만 복사하므로 입력 draft가 있으면 먼저 확정해야 한다. 실행 중 편집·저장·프로젝트 전환은 거부한다.
Stop은 원본·dirty·undo/redo를 바꾸지 않는다. 종료의 저장 확인에서는 Stop 뒤 원본을 저장한다. 취소는 실행을 보존한다.
고정 60 tick이며 실시간 한 프레임의 오래된 backlog는 최대 8 tick으로 제한한다.
Pause 뒤 1 tick 또는 도구의 N tick은 1~100000 범위에서 정확히 실행하며 따라잡기 제한을 사용하지 않는다.
키보드 WASD/방향키와 Space·왼쪽 마우스를 action으로 바꾼다. 화면 클릭으로 포커스를 얻고 TextEdit·모달·포커스 상실 때 해제한다.
실행 snapshot PNG 합산 RGBA 예산은 256 MiB이다. 최대 논리 프레임 4096²은 64 MiB이며 GPU/GUI 캐시는 별도이다.
정적 장면은 시작할 때 렌더하고 호스트가 텍스처를 보관한다. `frame`은 호스트 표시 요청 횟수이며 GPU 완료 fence가 아니다.

## 독립 player

`cargo run --locked -p engine-player -- PROJECT.json`은 editor/CLI/MCP 없이 같은 runtime과 입력·camera 호스트로 실행한다.
프로젝트와 상대 PNG를 읽으며 저장하지 않는다. `--headless-ticks N`은 같은 snapshot의 정확 tick/hash smoke이다.
headless 결과는 네이티브 창의 실제 출력 검증을 대신하지 않는다. M10 export 패키징·서명·공증은 별도 단계이다.

## 실제 화면

macOS ARM64/Retina 실제 eframe 창에서 dirty 문서 Play→Pause→73 tick→Stop→undo→재시작을 확인했다.
아래는 pause 후 tick 88/frame 39 상태이다. PNG·JSON hash는 흐름 전후 같았다.

![M3 editor Play](images/m3-editor-play-macos.png)

macOS 실제 독립 player의 같은 fixture 출력:

![M3 player](images/m3-player-macos.png)

## M4 선행 bounded 실행 실험과 지원 후보

`cargo run --locked -p engine-runtime --example behavior_probe`는 작은 텍스트 명령(`add`, `jump`, `fail`)을 파싱해
정상 계산·줄 번호가 있는 문법 오류·10000 instruction 무한 반복 중단·실행 오류를 검사한다. 2026-10-26 macOS ARM64에서 네 경우를 실행했다.
호스트 IO가 없는 명령 집합과 instruction budget은 같은 프로세스의 editor/player에 넣을 수 있는 격리 후보의 최소 실험이다.
이 toy 문법을 제품 행동 언어로 확정하지 않는다. M4에서 표현력·소스 위치·메모리 예산·호스트 API를 포함해 실제 작성 경로를 선택해야 한다.
Rhai/Wasm 등 외부 언어 backend의 통합/중단 가능성은 아직 검증하지 않았다.

지원 1차 후보는 macOS ARM64 + Windows x64, OS별 native build이다. CI 기준은 macOS 15/Windows 2025 runner이며,
사용자용 최소 OS 후보는 macOS 15/Windows 11이다. 최소 OS 실기기·Mac Intel·Windows ARM은 확정하지 않았고 M10에서 인수한다.
현재 60Hz 목표는 기본 320×180 정적 장면이다. 최대 해상도와 임의 자산 수에 대해 60Hz를 보장하지 않는다.
현재 기준선은 native tick/frame·프로세스 시작+load+step 시간·headless RSS이며, snapshot 생성과 CPU 합성만의 별도 측정은 추가 과제이다.
정적 화면은 1회 합성/업로드하여 반복 frame의 CPU 전체 재합성을 피한다.

정상 창 닫기·Cmd+Q의 dirty 취소 보존은 기존 보호 경로를 사용한다. Dock Quit와 강제 종료의 미저장 복구는 아직 지원하지 않는다.
강제 중단 뒤 복구 가능한 지점은 마지막 성공 저장이며 M3가 checkpoint나 마지막 입력 복구를 추가하지 않는다.

## M4 행동 실행 기반

행동은 프로젝트 JSON v2의 `behaviors: [{id, file, source}]`에 저장하며 개체의 nullable `behavior_id`로 연결합니다.
`file`은 오류에 표시할 `이동.rhai` 같은 논리 이름입니다. 외부 파일을 자동 실행하거나 실시간 재로딩하지 않습니다.
v1은 열 때 행동 없는 v2로 메모리에서 변환합니다. 열기만 하면 파일과 dirty는 바뀌지 않고 다음 저장에서 v2를 씁니다.
행동 추가/수정/삭제/연결은 `EditCommand`를 거쳐 원자 검증·undo/redo·기존 저장 보호를 공유합니다.
연결된 행동 삭제는 거부하므로 먼저 연결을 해제해야 합니다. 문법 오류가 있는 작업 중 소스도 저장할 수 있습니다.

Rhai 1.27.0을 고정합니다. 비교 실험에서 Wasm은 별도 컴파일/ABI가 필요하고 toy DSL은 표현력과 진단 비용이 커졌습니다.
Rhai 1.23.4에서는 map 인덱스 대입의 크기 제한과 산술 오류 위치 문제가 관찰됐습니다.
1.27.0의 실제 실험에서 나눗셈/overflow/중첩 함수 소스 위치, 연산/재귀/문자열 제한을 확인했습니다.
컬렉션은 M4 API에 필요 없어 `no_object`·`no_index`로 제거했습니다. module/time/float도 제거하며 `eval`을 금지합니다.
리뷰에서 표준 패키지 `pad(text, 2, "")`가 연산 budget 밖 native 무한루프를 만들 수 있음을 확인했습니다.
따라서 `Engine::new_raw()`에 패키지를 하나도 등록하지 않습니다. 언어의 기본 정수/비교 연산과 명시한 host API만 허용합니다.
`pad`/`repeat` 등 표준 라이브러리 함수는 unknown function 오류입니다. 파일·네트워크·프로세스 API도 등록하지 않습니다. 이것은 엔진이 제공하는 제한 실행 경계이며 OS process sandbox가 아닙니다.

매 60Hz tick에 연결된 소스를 장면 순서대로 실행합니다. 새 scope의 읽기 전용 `x`, `y`는 tick 시작 시 개체의 정수 위치,
`state`는 tick 시작 시 읽기 전용 i64 한 개(초기 0)입니다. 나머지 지역변수는 tick 뒤 사라집니다.
읽기 전용 `tick`(1부터), `left/right/up/down/primary`, `primary_pressed/primary_released`,
`pointer_x/pointer_y`(없으면 -1)를 읽을 수 있습니다. 좌표는 논리 픽셀입니다.

```rhai
if right { move(2, 0); }
if left { move(-2, 0); }
if down { move(0, 2); }
if up { move(0, -2); }
set_state(state + 1);
if primary_pressed { log("primary pressed"); }
```

`move(dx,dy)`는 임시 위치에 누적하고 `set_position(x,y)`는 임시 위치를 교체합니다.
`set_state(value)`는 임시 상태를 교체합니다. 호출 순서대로 반영하지만 읽기 상수는 같은 tick 안에서 바뀌지 않습니다.
세 함수는 정수만 받고 좌표는 i32 범위를 검사해 실제 호출 위치에서 오류를 반환합니다.
`log(string)`은 file/line/entity/tick을 남깁니다. print/debug는 외부 출력에 쓰지 않습니다.
모든 연결의 결과와 로그를 임시로 모아 성공할 때만 전체 tick을 반영합니다.
문법 오류는 Play 시작을 거부하고 기존 편집 문서를 보존합니다. 실행 오류는 해당 tick의 모든 위치/state/log를 취소하고 pause합니다.
이미 완료된 이전 tick은 유지합니다. `runtime.error`의 file/line/column/entity/tick/message로 진단하고 Stop→편집→Play로 재시작합니다.
중첩 함수 오류는 내부 오류의 소스 위치를 사용합니다. x/y/state 직접 대입은 상수 변경 오류이고 타입/범위 오류도 실제 호출 줄을 표시합니다.
실행 결과는 원본 문서·dirty·이력·파일에 반영하지 않습니다. editor와 player는 같은 실행기를 사용합니다.

상한은 소스 하나 8 KiB, 리소스32개(소스 합계256 KiB), 연결64개, 컴파일 AST64개입니다.
각 실행 연산2000/전체 tick8000, 호출 깊이8, 식 깊이32(함수16), 변수32, 함수16, 문자열256바이트로 제한합니다.
배열/map을 없애고 변수·호출·소스·연산 수를 함께 제한해 개별 문자열 외의 누적 자료도 제한합니다.
이는 byte 단위 프로세스 RSS 할당량을 의미하지 않습니다. scope는 tick마다 폐기하고 persistent 상태는 연결당 정수 하나뿐입니다.
로그는 전체 tick16개·문자열256바이트, 최근64개만 보관합니다. 초과하면 해당 tick이 실패합니다.
행동이 연결된 실행의 명시 step은 최대120 tick, 연결 없는 실행은 기존100000 tick입니다.
성공 시 요청한 tick을 모두 실행하고 실패 시 원인 tick 직전에 멈춥니다. 이 제한은 실시간 8 tick 따라잡기 제한과 별개입니다.
위치 변화는 visual revision을 올려 공통 호스트 texture를 갱신합니다. hash는 step/실시간 batch 끝의 공통 RGBA로 계산합니다.
