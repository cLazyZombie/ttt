# 언어

모든 대답은 한글(한국어)로 할 것.

# TTT - 타이핑 연습 TUI

구문 강조 기능이 있는 소스 코드 타이핑 연습 앱. Rust, ratatui, crossterm, syntect로 제작.

## 빌드 및 실행

```bash
cargo build
cargo run -- <소스파일>
cargo run -- --line 50 <소스파일>    # 50번째 줄부터 시작
cargo run -- --cursor <소스파일>     # 대화형 줄 선택
cat file.rs | cargo run -- -e rs        # 파이프로 stdin 입력 (확장자 지정)
git log -p | cargo run -- -e diff       # 임의의 명령 출력을 파이프로 전달
cargo run -- -e py -                    # `-`로 명시적 stdin 읽기
cargo run -- --ext js <파일>            # 파일 확장자 감지 재정의
git log -p | cargo run -- --diff        # diff에서 추가된 줄만 타이핑 (확장자 자동 감지)
git diff HEAD~1 | cargo run -- -d       # --diff 짧은 플래그
git log -p | cargo run -- --diff --src  # diff에서 소스 코드 파일만 (.rs, .c, .py 등)
cargo run -- --quiet <소스파일>      # 줄별 KPM 표시 끄기 (-q)
cargo run -- --wpm <소스파일>        # KPM 대신 WPM으로 표시
```

## 테스트

```bash
cargo test
```

## 테스트 규칙

- 로직이 있는 모든 모듈은 `#[cfg(test)] mod tests`와 단위 테스트를 갖추어야 함.
- 파서 테스트는 문자열 입력으로 `parse_source()`를 직접 사용 — 임시 파일 사용 금지.
- App 테스트는 헬퍼 함수(`make_code_line`, `make_comment_line` 등)를 사용하여 파싱 없이 `SourceLine`을 생성.
- 한 줄 전체를 타이핑하는 테스트는 반드시 `app.confirm_line()`을 호출하여 Enter를 시뮬레이션할 것.
- 테스트에서 `src/*.rs` 파일을 읽지 말 것 — 깨지기 쉬운 테스트를 방지하기 위해 인라인 문자열 패턴 사용.
- 커밋 전 `cargo test`가 통과해야 함.
- 작업 완료 시 반드시 `cargo fmt`, `cargo test`, `cargo clippy -- -D warnings` 세 가지를 모두 실행할 것.

## 커버리지

- 커버리지 계측 시 항상 아래 기준값과 현재 결과를 비교하여 차이를 보여줄 것.

```bash
cargo llvm-cov --summary-only   # 파일별 요약
cargo llvm-cov --text            # 줄별 상세
cargo llvm-cov --html            # HTML 리포트 (target/llvm-cov/html/)
```

- `stats.rs` — 100%
- `app.rs` — ~99.7%
- `parser.rs` — ~97.6%
- `main.rs`, `ui.rs` — 단위 테스트 불가 (터미널/Frame 의존)

## 아키텍처

- `src/main.rs` — CLI 진입점 (`--line`, `--cursor`, `--ext`, `--diff`, `--quiet`, `--wpm`, stdin), 터미널 생명주기, 이벤트 루프
- `src/parser.rs` — syntect 파싱, 주석 감지, `SourceLine` 생성
- `src/app.rs` — App 상태, 타이핑 로직, 줄 진행, 커서 선택 모드, 단어 삭제
- `src/ui.rs` — ratatui 렌더링 (인라인 오버레이, 선택 모드, 상태 바, 결과 화면)
- `src/stats.rs` — KPM 및 정확도 계산
- `themes/github-dark.tmTheme` — 내장 GitHub Dark 테마 (`include_bytes!`로 로드)
