# 초기 검증 기록

기준일: 2026-10-25. macOS ARM64 개발 환경에서 확인했습니다.

## 확인한 것

| 항목 | 결과 |
| --- | --- |
| 기존 루트 AGENTS.md | 존재 확인. 수정하지 않음 |
| 기존 저장소 | 소스·빌드 설정·Git 이력 없음 |
| Python | 3.14.8 설치 확인 |
| Apple clang | 21.0.0 설치 확인 |
| Rust 도구 | rustup·rustc·cargo·rustfmt·cargo-clippy 없음 |
| Cargo·도구 설정 파일 | Python tomllib로 TOML 문법 확인 |
| 개발 스크립트 | Python AST 문법 검사 및 도움말 실행 확인 |
| 개발 명령 인자 | 도움말·인자 누락·불필요한 인자 처리 3건 확인 |
| 문서 연결 | README·설계 문서의 로컬 링크 대상 확인 |
| doctor 실행 | 누락된 Rust 도구를 나열하고 오류 종료하는 동작 확인 |
| setup 실행 | 공식 설치기 URL까지 진행 후 DNS 조회 실패. Rust는 설치되지 않음 |
| check 실행 | Rust 미설치 안내 후 종료. 실제 검사 단계는 실행되지 않음 |

설치 실패 메시지: `nodename nor servname provided, or not known`.
별도의 curl 요청도 `Could not resolve host: static.rust-lang.org`로 실패했습니다.
따라서 Rust 설치 완료나 실행 가능한 엔진이 준비됐다고 판단할 수 없습니다.

## 작성했지만 실행하지 못한 것

- Rust 코어 테스트 8개: 정수 배율·중앙 배치·최소 크기·고정 틱·긴 정지·잘못된 시간
- CLI 프로세스 계약 검사 5개: info·viewport 정상 응답, 작은 창·잘못된 크기·알 수 없는 명령
- rustfmt·Clippy 검사, workspace 빌드
- 실제 macOS 창 출력과 키 입력, Windows 빌드와 실행

문법 설정 검사는 Rust 컴파일이나 실제 렌더링 검증을 대신하지 않습니다.

## 설치가 가능한 환경에서 이어가기

```sh
python3 scripts/dev.py setup
python3 scripts/dev.py doctor
python3 scripts/dev.py fmt
python3 scripts/dev.py check
python3 scripts/dev.py run
```

`setup` 성공 후 생성된 Cargo.lock을 버전 관리에 포함하고,
실행 환경·명령·종료 코드·실제 화면 결과를 이 문서에 추가합니다.
Windows에서는 `py -3`를 사용해 같은 순서로 확인합니다.
MCP 서버는 아직 구현하지 않았으므로 연결 검증 대상도 아닙니다.
