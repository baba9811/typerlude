# Typeul

Typeul(타이플)은 오프라인 우선 한국어·영어 터미널 타자 연습기입니다. 키 학습부터
단어·문장·긴 글·타자 시험까지 한 앱에서 연습하고, 약한 키와 진행 기록을 로컬에서
확인할 수 있습니다.

Typeul is an offline-first Korean and English terminal typing tutor. It covers
key learning, words, sentences, long text, timed tests, weak-key analysis, and
local progress tracking without requiring an account or cloud service.

## 시작하기 / Getting started

레지스트리 정식 배포 전에는 소스에서 설치합니다. Rust 1.88 이상이 필요합니다.
Until the registry release is published, install from source with Rust 1.88 or
newer:

```bash
git clone https://github.com/baba9811/typeul.git
cd typeul
cargo install --path . --locked
typeul
```

UTF-8과 alternate screen을 지원하는 대화형 터미널에서 실행해야 하며 권장 최소 크기는
80×24입니다. 작은 창에서는 안전하게 크기 안내만 표시합니다. Typeul uses raw mode,
bracketed paste, and an alternate screen while running, then restores the cursor,
input mode, and original screen on normal exit, errors, and panics.

## 연습 모드 / Practice modes

| 모드 / Mode | 동작 / Behavior | 기본값 / Default |
| --- | --- | --- |
| 빠른 연습 / Quick | 단어를 시간 또는 항목 수 기준으로 연속 연습 | English/Korean words, 30 seconds |
| 키 연습 / Key | 한글 두벌식 또는 영문 QWERTY 키를 단계별 연습 | Stage 1, ordered |
| 단어 / Words | 난이도와 저장 기록을 반영한 단어 묶음 | Mixed difficulty, adaptive on |
| 문장 / Sentence | 문장별 즉시 속도·정확도 피드백 | Mixed sentences |
| 긴 글 / Long text | 문단 진행률, 출처, 30초 최고 구간 속도 표시 | First matching bundled text |
| 타자 시험 / Typing test | 중단 없는 시간제 문장 시험과 상대 등급 | 300 seconds |

Home에서 원하는 모드를 선택하고 Enter를 두 번 누르면 기본 설정으로 시작합니다.
CLI의 `--lang`과 `--time`으로 시작 값을 바로 지정할 수도 있습니다. Typing Test는
공정성을 위해 일시 정지를 허용하지 않습니다.

Choose a mode on Home and confirm it to start with the defaults. CLI `--lang`
and `--time` options override the relevant defaults. A typing test cannot be
paused.

## 명령줄 / Command line

```text
typeul
typeul quick [--lang ko|en] [--time 15|30|60|120]
typeul keys|words|sentence|long [--lang ko|en]
typeul test [--lang ko|en] [--time 60|180|300|600]
typeul FILE
typeul practice FILE
typeul stats|history|themes
typeul content list
typeul content add PACK.toml
typeul content validate [PACK.toml]
typeul content disable PACK_ID
typeul paths|licenses|update
typeul --help|--version|--smoke
```

`FILE`과 stdin은 NFC로 정규화할 수 있는 비어 있지 않은 UTF-8 텍스트여야 하며 최대
8 MiB입니다. CRLF는 LF로 바꾸고, 터미널 제어 문자가 든 입력은 거부합니다. Unix에서는
제어 터미널(`/dev/tty`)이 남아 있을 때 `cat FILE | typeul`도 사용할 수 있습니다.
Windows에서는 콘솔 입력을 유지하도록 `typeul FILE`을 사용하세요.

Files and stdin must be nonblank UTF-8, are bounded to 8 MiB, normalize CRLF to
LF, and reject terminal control characters. A custom text stays in memory and
is never copied into a session record.

`content`, `paths`, `licenses`, `update`, `--help`, `--version`, `--smoke`는
alternate screen을 열지 않는 headless 명령입니다. Interactive launch requests fail
with exit code 2 when no usable terminal is attached.

## 키보드 / Keyboard

| 키 / Key | 동작 / Action |
| --- | --- |
| `Tab`, `Shift+Tab`, `↑`, `↓`, `j`, `k` | 포커스 이동 / Move focus |
| `Enter` | 선택·입력 확정 / Select or commit a newline while typing |
| `←`, `→` | 필터·목표 값 변경 / Change filters or goal values |
| `Esc` | 뒤로; 연습 중 일시 정지 / Back; pause during practice |
| `Ctrl+P` | 연습 일시 정지·계속 / Pause or resume practice |
| `q` | 화면 종료; 일시 정지 중 두 번 누르면 연습 종료 / Quit; press twice while paused to leave practice |
| `Ctrl+C` | 어디서나 즉시 종료 / Quit globally |
| `?` | 도움말 / Help |
| `Backspace` | 현재 항목 안에서 수정 / Correct within the current item |
| `r` | 결과 화면에서 같은 연습 재시도 / Retry from Result |
| `d` twice | 사용자 콘텐츠 상세 화면에서 비활성화 / Disable a user pack in Content Detail |
| `l`, `s` | 표시된 업데이트 알림을 나중에 보기 / 해당 버전 건너뛰기 |

붙여넣기는 연습 점수 조작을 막기 위해 무시되며 3초간 안내가 표시됩니다. Mouse and
other non-key events do not mutate practice state.

## 지표 / Metrics

- Active time은 첫 유효 입력부터 세며 일시 정지 시간은 제외합니다.
- KPM은 분당 올바른 한글 키 단위, CPM은 분당 올바른 문자 셀 수입니다.
- English WPM은 표준 `CPM / 5`입니다.
- Accuracy는 최초 시도를 포함한 `correct attempted units / attempted units`입니다.
- 오류를 Backspace로 고쳐도 오류·시도 횟수는 지워지지 않으며 속도를 부풀리지 않습니다.
- 긴 글의 최고 구간 속도는 최근 30초의 올바른 입력으로 계산합니다.
- Statistics의 정확도는 시도 수로 가중하고, 기록은 7/30/90일 또는 전체 범위와
  언어·모드로 필터링합니다.

Active time starts on the first accepted input and excludes pauses. Korean
speed uses KPM, English uses WPM (`correct characters per minute / 5`), and
accuracy remains attempt-based even after corrections. Test grades are relative
to the speed and accuracy goals saved in your settings, not an official national
certification.

## 사용자 데이터와 개인정보 / User data and privacy

`typeul paths`가 이 설치에서 실제로 쓰는 경로를 출력합니다. 테스트·휴대용 실행에는
`TYPEUL_HOME=/path/to/typeul-home`을 지정하면 모든 파일을 그 루트 아래에 모을 수
있습니다. Otherwise Typeul follows platform config, state, data, and cache
directories through the operating system conventions:

| OS | Config | Sessions | Content / themes | Update cache |
| --- | --- | --- | --- | --- |
| Linux | `$XDG_CONFIG_HOME/typeul/config.toml` | `$XDG_STATE_HOME/typeul/sessions/` | `$XDG_DATA_HOME/typeul/{content,themes}/` | `$XDG_CACHE_HOME/typeul/update.json` |
| macOS | `~/Library/Application Support/typeul/config.toml` | `~/Library/Application Support/typeul/sessions/` | `~/Library/Application Support/typeul/{content,themes}/` | `~/Library/Caches/typeul/update.json` |
| Windows | `%APPDATA%\typeul\config\config.toml` | `%LOCALAPPDATA%\typeul\data\sessions\` | `%APPDATA%\typeul\data\{content,themes}\` | `%LOCALAPPDATA%\typeul\cache\update.json` |

완료한 연습은 `sessions/<id>.json` 한 파일로 원자적·무덮어쓰기 저장됩니다. 저장 필드는
schema/version ID, 시작 시각과 local date, 언어·모드·콘텐츠 ID·난이도, active duration,
correct/attempted units, errors, backspaces, CPM/KPM/WPM, accuracy, 그리고 의도한 키별
집계 `[correct, error]`뿐입니다.

Session JSON never contains the target text, what you typed, a custom filename,
per-keystroke timestamps, account identifiers, or network identifiers. Corrupt
or unsupported config/session/content/theme files are preserved byte-for-byte,
reported as warnings, and skipped instead of being deleted or silently replaced.

## 설정 / Settings

TUI의 Settings와 Goals 화면에서 값을 저장할 수 있습니다. 직접 편집할 때는 일부 필드를
생략해도 기본값이 적용됩니다. Settings and Goals save atomically:

```toml
schema_version = 1
language = "en"
ui_language = "ko"
theme = "nord"
show_keyboard = true
show_finger_guide = true
show_live_speed = true
show_accuracy = true
target_kpm = 450
target_wpm = 80
target_accuracy = 98.0
daily_minutes = 15
adaptive = true
check_updates = true
skipped_update_version = ""
```

설정 파일이 처음부터 없을 때만 `LC_ALL`, 그다음 `LANG`을 보고 한국어 UI를 선택합니다.
An existing or corrupt file is never overwritten merely because the locale
changed.

## 사용자 콘텐츠 / Custom content packs

추가는 `validate → add` 순서가 권장됩니다. `add`는 시작 시와 동일한 스키마·NFC·중복·
출처·라이선스 규칙을 적용하고 기존 파일을 덮어쓰지 않습니다. `disable`은 내장 팩을
거부하며 사용자 팩만 `content/disabled/`로 원자적으로 이동합니다.

```bash
typeul content validate my-pack.toml
typeul content add my-pack.toml
typeul content list
typeul content disable my-pack
```

최소 팩 예시 / Minimal pack example:

```toml
schema_version = 1
id = "my-pack"
title = "My practice pack"
language = "en"

[source]
author = "Your Name"
source_id = "my-pack-v1"
source_url = "https://example.com/my-pack"
license = "CC0-1.0"
license_url = "https://creativecommons.org/publicdomain/zero/1.0/"
modified = false
retrieved_at = "2026-08-10"

[[items]]
id = "my-pack-word-one"
kind = "word" # word | sentence | quote | text
text = "example"
difficulty = 1 # optional, 1..3
tags = ["custom"]
```

Pack/item IDs must be unique; addable pack filenames use ASCII letters, digits,
`-`, or `_`. Text must be NFC and duplicates are checked within the same
language and content kind. Supported declared licenses are `CC0-1.0`,
`CC-BY-2.0-FR`, `CC-BY-4.0`, `KOGL-0`, `KOGL-1.0`, and
`LicenseRef-Public-Domain`.

중요: 출처 표시는 그 자체로 재배포 권한을 만들지 않습니다. 본인이 권리를 가진 자료,
퍼블릭 도메인 자료, 또는 npm/crates 배포를 실제로 허용하는 라이선스 자료만 추가하고
해당 조건을 모두 지켜야 합니다. Attribution alone does not grant permission;
verify redistribution rights before publishing a pack.

## 사용자 테마 / Custom themes

`typeul paths`가 표시하는 `themes/`에 `<id>.toml`을 두면 시작 시 검증해 불러옵니다.
색은 `reset`, 16개 ANSI 이름, 또는 정확한 `#RRGGBB`를 사용합니다.

```toml
schema_version = 1
id = "my-theme"
background = "black"
foreground = "white"
accent = "light_cyan"
correct = "light_green"
error = "light_red"
cursor = "light_yellow"
dim = "dark_gray"
```

`typeul themes` 또는 Settings의 Theme 항목에서 선택합니다. Invalid, oversized,
non-UTF-8, symlinked, and duplicate-ID theme files are preserved and shown as
warnings rather than loaded.

## 업데이트 / Updates

Typeul은 설치 파일을 자동으로 바꾸지 않습니다. `typeul update`만 명시적으로 실행하면
설치 방식에 맞는 공개 버전을 확인하고 명령을 안내합니다. npm 설치본의 대화형 실행만
최대 하루에 한 번 백그라운드 확인할 수 있으며 UI를 막지 않습니다.

Typeul never self-installs an update. Disable optional npm background notices
with either:

```toml
check_updates = false
```

or:

```bash
TYPEUL_NO_UPDATE_CHECK=1 typeul
```

`CI` 환경에서도 자동 확인은 꺼집니다. 알림의 `l`은 나중에, `s`는 해당 버전만
건너뜁니다. The bounded cache is `cache/update.json`; malformed, oversized, or
non-regular cache entries are ignored.

## 터미널 호환성 / Terminal compatibility

Typeul is built on Crossterm 0.29 and Ratatui 0.30. The upstream tested set
includes Windows Terminal and Console Host, Ubuntu-family terminals, KDE
Konsole, Kitty, Alacritty, Crostini, and macOS terminals. Use a UTF-8 locale and
an 80×24 or larger window. RGB themes need a terminal with true-color support;
the bundled `default`, `minimal`, and `monochrome` themes remain usable with
basic ANSI colors.

If the process is killed with an uncatchable signal, the shell's `reset` command
may still be needed; normal exit, handled errors, Ctrl+C, and Rust panics restore
terminal state automatically.

## 출처와 라이선스 / Sources and licenses

Typeul software is MIT-licensed. Project-authored bundled data is CC0 1.0.
Bundled Tatoeba material retains its stated CC0 or CC BY 2.0 France provenance,
and the Nord palette retains its MIT notice. Every bundled source, retrieval
date, modification flag, and license is frozen in the content metadata and
displayed in Content Detail.

```bash
typeul content list
typeul licenses
```

`typeul licenses` prints the offline licenses and notices for Typeul's bundled
content and palette. Repository copies are in [assets/licenses](assets/licenses)
and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md); the Typeul source license
is [LICENSE](LICENSE). Any binary distribution must additionally include the
licenses and notices of its compiled Rust dependencies.

## 개발 검증 / Development checks

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
node --test scripts/import-tatoeba.test.mjs
cargo package --locked
make pty-smoke
```

`make pty-smoke` builds the release binary and completes a real 80×24 Unix PTY
session, checking Result persistence, privacy-safe aggregates, and terminal cleanup.

The repository does not claim npm or crates.io publication until those registry
steps have actually completed.

## 배포와 보안 / Releases and security

현재 레지스트리 배포 전 설치 방법은 위의 source installation입니다. 공개 배포가 완료되기
전에는 npm, crates.io, 또는 GitHub Release에 패키지가 있다고 가정하지 마세요. Maintainers
should follow the guarded bootstrap and OIDC checklist in
[docs/releasing.md](docs/releasing.md); it keeps the GitHub release as a draft until both registries
succeed. This ordering reduces accidental exposure but cannot make separate registries atomic.

보안 취약점은 공개 issue 대신 [GitHub Security Advisory](SECURITY.md)로 비공개 제보해
주세요. Please see the [security policy](SECURITY.md) for supported versions and private reporting.
