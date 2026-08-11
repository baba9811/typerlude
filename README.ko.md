# Typeul

[English](https://github.com/baba9811/typeul/blob/main/README.md) |
[한국어](https://github.com/baba9811/typeul/blob/main/README.ko.md)

[![CI](https://github.com/baba9811/typeul/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/baba9811/typeul/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/typeul?logo=rust)](https://crates.io/crates/typeul)
[![npm](https://img.shields.io/npm/v/%40baba9811%2Ftypeul?logo=npm)](https://www.npmjs.com/package/@baba9811/typeul)
[![License: MIT](https://img.shields.io/badge/license-MIT-14B8A6)](LICENSE)

타자 연습할 때, 혹은 바이브 코딩하다 심심할 때 터미널에서 잠깐 머리를 식혀 보세요.

Typeul(타이플)은 오프라인 우선 한국어·영어 터미널 타자 연습기입니다. 계정, 텔레메트리,
클라우드 저장 없이 여섯 가지 연습 모드와 로컬 기록, 약한 키 분석, 출처가 명확한 연습
자료를 제공합니다.

## 1분 만에 시작하기

위 두 레지스트리 배지에 모두 배포 버전이 표시되면 다음처럼 설치하세요.

```bash
npm install -g @baba9811/typeul
typeul
```

npm 패키지는 macOS, Linux, Windows의 x64·arm64에 맞는 사전 빌드 바이너리를 함께
설치합니다. 이 설치 경로에는 Rust가 필요하지 않습니다.

<details>
<summary>Cargo로 설치하기</summary>

```bash
cargo install typeul
typeul
```

</details>

최초 배포 전이거나 최신 `main`을 바로 써보려면 소스에서 실행하세요.

```bash
git clone https://github.com/baba9811/typeul.git
cd typeul
cargo run --release --
```

UTF-8 대화형 터미널에서 실행하세요. 권장 최소 크기는 80×24입니다.

## Typeul을 쓰는 이유

- **한국어와 영어:** 한글 두벌식과 영문 QWERTY를 한 앱에서 연습합니다.
- **여섯 가지 모드:** 키, 빠른 연습, 단어, 문장, 긴 글, 타자 시험을 제공합니다.
- **정직한 지표:** 실제 입력 시간 기준 속도, 최초 시도 기반 정확도, 최고 구간, 약한 키를 봅니다.
- **로컬 우선:** 설정과 집계된 연습 기록은 내 컴퓨터에만 저장됩니다.
- **내 글로 연습:** UTF-8 파일, Unix stdin, 재사용 가능한 콘텐츠 팩을 지원합니다.
- **검증 가능한 자료:** 내장 항목마다 출처, 수정 여부, 라이선스 메타데이터를 보존합니다.

## 연습 모드

| 모드 | 이런 때 사용하세요 |
| --- | --- |
| 키 | 두벌식 또는 QWERTY를 단계별로 익힐 때 |
| 빠른 연습 | 시간이나 항목 수를 정해 짧게 쉬고 싶을 때 |
| 단어 | 난이도와 기록을 반영해 단어를 연습할 때 |
| 문장 | 문장마다 속도와 정확도를 바로 확인할 때 |
| 긴 글 | 문단 진행률, 출처, 최고 구간 속도를 보며 연습할 때 |
| 타자 시험 | 중단 없는 시간제 시험과 상대 등급이 필요할 때 |

Home에서 모드를 고르고 옵션을 조정한 뒤 `Enter`를 누르세요. 타자 시험은 일시 정지할 수
없습니다.

## 핵심 키

| 키 | 동작 |
| --- | --- |
| `Tab`, `Shift+Tab`, `↑`, `↓`, `j`, `k` | 포커스 이동 |
| `Enter` | 선택, 시작, 줄바꿈 입력 |
| `Esc`, `Ctrl+P` | 뒤로 가기, 시험 외 연습 일시 정지·계속 |
| `q`, `Ctrl+C` | 종료; `q`는 연습 조기 종료 확인에도 사용 |
| `Backspace` | 현재 항목 안에서 수정 |
| `r`, `n` | 결과 화면에서 재시도 또는 지원 모드의 다음 연습 |
| `?` | 현재 화면 도움말 |

점수 조작을 막기 위해 연습 중 붙여넣기는 무시됩니다. 전체 명령은 `typeul --help`에서
확인하세요.

## 내 글로 연습하기

```bash
typeul notes.txt
typeul practice notes.txt
cat notes.txt | typeul  # 제어 터미널이 있는 Unix
```

사용자 텍스트는 메모리에서만 사용하며 세션 기록에 복사하지 않습니다. 단어·문장·인용문·
긴 글 묶음을 재사용하려면
[콘텐츠 팩 안내](https://github.com/baba9811/typeul/blob/main/docs/content-packs.ko.md)를 참고하세요.

유용한 명령:

```bash
typeul stats
typeul history
typeul paths
typeul themes
typeul licenses
typeul update
```

## 로컬 데이터와 개인정보

`typeul paths`는 현재 운영체제에서 실제로 사용하는 설정, 세션, 콘텐츠, 테마, 캐시 경로를
출력합니다. `TYPEUL_HOME=/path`를 지정하면 모든 파일을 한 루트 아래에 둘 수 있습니다.

세션 파일에는 집계된 시간, 정확도, 속도, 오류, Backspace, 모드, 의도한 키별 횟수만
저장합니다. 연습 원문, 실제로 입력한 문자열, 사용자 파일명, 키별 시각, 계정, 네트워크
식별자는 저장하지 않습니다. 손상되거나 지원하지 않는 사용자 파일은 덮어쓰지 않고 보존한
채 경고로 알립니다.

## 콘텐츠와 라이선스

내장 연습 원문은 키보드로 바로 입력할 수 있는 ASCII와 한국어 현대 한글 음절만 사용합니다.
원문의 동그라미 문단 번호 같은 문자는 일반 키보드 문자로 옮기고 출처 정보에 수정 사실을
표시합니다. 이 제한은 사용자가 직접 추가한 텍스트에는 적용하지 않습니다.

Typeul 소프트웨어는 MIT 라이선스입니다. 프로젝트 작성 연습 자료는 CC0 1.0이며, 나머지
내장 자료는 선언된 퍼블릭 도메인·Creative Commons·MIT 조건을 유지합니다. 전체 고지는
`typeul licenses` 또는 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)에서 확인하세요.

## 개발

```bash
git clone https://github.com/baba9811/typeul.git
cd typeul
cargo run --
make test
```

Typeul은 Rust 1.88을 고정합니다. 전체 검증에는 Node.js와 CI에 명시된 정책 도구도 사용합니다.
관리자는 [배포 안내](https://github.com/baba9811/typeul/blob/main/docs/releasing.md)를 따르고,
보안 문제는 공개 issue가 아닌
[GitHub Security Advisory](https://github.com/baba9811/typeul/security/advisories/new)로 제보해
주세요.

## 라이선스

[MIT](LICENSE) © Typeul contributors.
