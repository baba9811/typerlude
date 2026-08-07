# Typeul

오프라인 우선 한국어·영어 터미널 타자 연습기입니다. This repository builds an
offline-first Korean and English terminal typing tutor. The current core validates
interactive launch requests and provides the headless commands below; the guarded
terminal event loop is integrated in the next implementation plan.

```text
typeul
typeul quick [--lang ko|en] [--time 15|30|60|120]
typeul keys|words|sentence|long [--lang ko|en]
typeul test [--lang ko|en] [--time 60|180|300|600]
typeul FILE
typeul practice FILE
cat FILE | typeul
typeul stats|history|themes
typeul content list
typeul content add PACK.toml
typeul content validate [PACK.toml]
typeul content disable PACK_ID
typeul paths|licenses|update
typeul --help|--version|--smoke
```

파일과 stdin은 UTF-8이어야 하며 공백만 있는 입력과 8 MiB 초과 입력은 거부합니다.
Files and stdin must be nonblank UTF-8 and are read with an 8 MiB limit.

## 사용자 데이터 / User data

`typeul paths`가 실제 경로를 표시합니다. `TYPEUL_HOME=/portable/typeul`을 지정하면
`config.toml`, `sessions/`, `content/`, `themes/`, `cache/update.json`을 모두 그
루트 아래에 둡니다. Otherwise Typeul uses the platform config, state, data, and
cache categories:

| OS | Config | Sessions | Content/themes | Cache |
| --- | --- | --- | --- | --- |
| Linux | `$XDG_CONFIG_HOME/typeul/config.toml` | `$XDG_STATE_HOME/typeul/sessions/` | `$XDG_DATA_HOME/typeul/` | `$XDG_CACHE_HOME/typeul/` |
| macOS | `~/Library/Application Support/typeul/config.toml` | `~/Library/Application Support/typeul/sessions/` | `~/Library/Application Support/typeul/` | `~/Library/Caches/typeul/` |
| Windows | `%APPDATA%\typeul\config\config.toml` | `%LOCALAPPDATA%\typeul\data\sessions\` | `%APPDATA%\typeul\data\` | `%LOCALAPPDATA%\typeul\cache\` |

완료한 연습은 변경하지 않는 JSON 파일 하나로 저장합니다. 기록에는 집계된 시간,
속도, 정확도, 오류와 의도한 키별 횟수만 있으며 원문, 실제 입력문, 파일명, 키 입력
시각은 저장하지 않습니다. 손상된 설정·세션·콘텐츠는 삭제하거나 덮어쓰지 않고
경고 후 건너뜁니다. Each completed practice uses one immutable, aggregate-only
session JSON; malformed files remain untouched and are reported as warnings.

## 콘텐츠 / Content packs

`content validate`는 시작 시 사용되는 것과 같은 스키마·출처·라이선스·충돌 규칙을
적용합니다. `content add`는 검증된 원본 바이트를 사용자 content 디렉터리에 원자적으로
추가하며 기존 파일을 덮어쓰지 않습니다. `content disable PACK_ID`는 내장 팩을 거부하고
사용자 팩만 `content/disabled/`로 이동합니다. `content list`는 활성 팩의 언어, 항목 수,
라이선스와 출처를 표시합니다.

The equivalent English workflow is `content validate PACK.toml`, `content add
PACK.toml`, and `content disable PACK_ID`. Pack IDs used as filenames are limited
to ASCII letters, digits, `-`, and `_`.

## 업데이트와 라이선스 / Updates and licenses

Typeul은 스스로 설치 파일을 교체하지 않습니다. `typeul update`는 현재 버전과 공개
릴리스 위치를 표시합니다. 이후의 선택적 알림 확인은 `check_updates = false` 또는
`TYPEUL_NO_UPDATE_CHECK=1`로 끌 수 있습니다. Typeul never performs an automatic
installation or replacement.

Typeul software is MIT-licensed. Project-authored practice data is CC0 1.0;
third-party data keeps its stated license. Run `typeul licenses` for the complete
offline texts, or read [LICENSE](LICENSE) and
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

이 저장소 문서는 npm 또는 crates.io에 이미 배포되었다고 주장하지 않습니다.
This README does not claim that registry publication has already occurred.
