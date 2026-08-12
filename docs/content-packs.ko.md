# 콘텐츠 팩

[English](content-packs.md) | [한국어](content-packs.ko.md)

검색 가능한 메타데이터와 명확한 출처를 가진 연습 묶음을 재사용하려면 콘텐츠 팩을
사용하세요. 한 번만 연습할 글은 `typerlude FILE`이 더 간단합니다.

## 팩 추가

설치 전에 검증하세요. Typerlude는 검증과 시작 시점에 같은 스키마, NFC, 중복, 출처,
라이선스 규칙을 적용하며 기존 팩을 덮어쓰지 않습니다.

```bash
typerlude content validate my-pack.toml
typerlude content add my-pack.toml
typerlude content list
```

최소 예시:

```toml
schema_version = 1
id = "my-pack"
title = "나의 연습 팩"
language = "ko" # en | ko

[source]
author = "작성자 이름"
source_id = "my-pack-v1"
source_url = "https://example.com/my-pack"
license = "CC0-1.0"
license_url = "https://creativecommons.org/publicdomain/zero/1.0/"
modified = false
retrieved_at = "2026-08-11"

[[items]]
id = "my-pack-word-one"
kind = "word" # word | sentence | quote | text
text = "예시"
difficulty = 1 # 선택, 1..3
tags = ["custom"]
```

팩 ID와 항목 ID는 고유해야 합니다. 설치 가능한 팩 ID에는 ASCII 영문자, 숫자, `-`, `_`만
사용합니다. 텍스트는 비어 있지 않은 NFC UTF-8이어야 하며 터미널 제어 문자를 포함할 수
없습니다. 중복은 같은 언어와 콘텐츠 종류 안에서 검사합니다.

선언할 수 있는 라이선스는 `CC0-1.0`, `CC-BY-2.0-FR`, `CC-BY-4.0`,
`CC-BY-SA-4.0`, `KOGL-0`, `KOGL-1.0`, `LicenseRef-Public-Domain`입니다.

출처 표시만으로 재배포 권한이 생기지 않습니다. 본인이 권리를 가진 자료, 퍼블릭 도메인
자료, 또는 npm·crates.io 패키지 재배포를 허용하는 라이선스 자료만 추가하고 모든 조건을
지키세요.

## 팩 비활성화

```bash
typerlude content disable my-pack
```

사용자 팩만 비활성화할 수 있습니다. Typerlude는 기존 항목을 덮어쓰지 않고 승인된 파일을
`content/disabled/`로 옮깁니다. 실제 경로는 `typerlude paths`로 확인하세요.
