<h1 align="center">Typerlude</h1>

<p align="center"><strong>터미널에서 잠깐 즐기는 타자 연습.</strong></p>

<p align="center">
  <a href="https://github.com/baba9811/typerlude/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/baba9811/typerlude/ci.yml?branch=main&amp;style=flat-square&amp;label=CI"></a>
  <a href="https://www.npmjs.com/package/typerlude"><img alt="npm" src="https://img.shields.io/npm/v/typerlude?logo=npm&amp;style=flat-square"></a>
  <a href="https://crates.io/crates/typerlude"><img alt="crates.io" src="https://img.shields.io/crates/v/typerlude?logo=rust&amp;style=flat-square"></a>
  <a href="https://github.com/baba9811/typerlude/releases"><img alt="GitHub release" src="https://img.shields.io/github/v/release/baba9811/typerlude?sort=semver&amp;style=flat-square"></a>
  <a href="LICENSE"><img alt="코드 라이선스: MIT" src="https://img.shields.io/badge/code%20license-MIT-blue?style=flat-square"></a>
</p>

<p align="center"><a href="README.md">English</a> · <strong>한국어</strong></p>

한국어·영어 타자를 연습하거나, 바이브 코딩 중 잠깐 심심할 때 터미널에서 쉬어가세요.
Typerlude는 오프라인으로 실행되며 연습 데이터를 내 컴퓨터에만 보관합니다.

## 설치

| npm · Node.js 18+ | Cargo · Rust 1.88+ |
| --- | --- |
| `npm install -g typerlude` | `cargo install typerlude` |

설치 후 실행하세요.

```bash
typerlude
```

UTF-8 대화형 터미널을 사용하세요. 권장 최소 크기는 80×24입니다.

## 바로 시작하기

모드를 고르고 옵션을 조정한 뒤 `Enter`를 누르세요. `Tab`, 화살표 키, `j`, `k`로 이동하고,
`Esc`, `q`, `ㅂ`으로 뒤로 가며 홈에서는 같은 키로 종료합니다. 연습 중에는 `Esc`로
일시 정지하거나 나가기 확인을 열고, 입력 중인 `q`와 `ㅂ`은 연습 글자로 처리합니다. 단어 연습은
`Space` 또는 `Enter`로 현재 단어를 제출합니다.

내 파일이나 표준 입력으로도 연습할 수 있습니다.

```bash
typerlude notes.txt
cat notes.txt | typerlude
```

파일과 표준 입력의 연속 빈 줄은 연습 전에 한 줄로 축약됩니다.

점수 연습 중 붙여넣기는 무시됩니다.

## 주요 기능

- 빠른 연습, 키, 단어, 문장, 긴 글, 시간제 시험 모드
- 한국어·영어 UI와 콘텐츠, 언어별 점수 및 속도 단위
- 유니코드 기반 타자 지표, 목표, 기록, 추이, 약한 키 연습
- 오프라인 기본 콘텐츠와 로컬 콘텐츠 팩·테마
- 계정, 텔레메트리, 광고, 클라우드 저장소 없음

## 유용한 명령

| 명령 | 기능 |
| --- | --- |
| `typerlude stats` | 통계 화면 열기 |
| `typerlude history` | 세션 기록 열기 |
| `typerlude themes` | 테마 선택하기 |
| `typerlude paths` | 로컬 데이터 경로 모두 출력하기 |
| `typerlude licenses` | 오프라인 라이선스 고지 출력하기 |
| `typerlude update` | 새 버전 확인하기 |
| `typerlude --help` | CLI 도움말 보기 |

## 개인정보와 로컬 데이터

사용자가 넣은 텍스트는 메모리에만 남고 세션 기록에 복사되지 않습니다. 저장되는 세션에는
집계 지표와 의도한 키별 횟수만 있으며, 실제 입력 내용은 없습니다. 설정, 세션, 콘텐츠, 테마,
업데이트 캐시는 운영체제의 표준 사용자 디렉터리를 사용합니다. 정확한 위치는
`typerlude paths`로 확인하세요.

## 기여하기

버그 제보, 기능 제안, 범위가 명확한 Pull Request를 환영합니다.
[기여 안내](https://github.com/baba9811/typerlude/blob/main/CONTRIBUTING.md)와
[행동 강령](https://github.com/baba9811/typerlude/blob/main/CODE_OF_CONDUCT.md)을 먼저 확인하세요.

## 프로젝트 링크

- [Content-pack guide](https://github.com/baba9811/typerlude/blob/main/docs/content-packs.md) · [콘텐츠 팩 안내](https://github.com/baba9811/typerlude/blob/main/docs/content-packs.ko.md)
- [보안 정책](https://github.com/baba9811/typerlude/blob/main/SECURITY.md)
- [릴리스](https://github.com/baba9811/typerlude/releases) · [배포 안내](https://github.com/baba9811/typerlude/blob/main/docs/releasing.md)
- [라이선스](LICENSE) · [제3자 및 콘텐츠 고지](THIRD_PARTY_NOTICES.md)

## 개발

```bash
git clone https://github.com/baba9811/typerlude.git
cd typerlude
cargo run --
make test
```

Typerlude 원본 소스 코드는 MIT 라이선스이고, 프로젝트가 만든 연습 데이터는 CC0입니다.
번들된 제3자 콘텐츠와 의존성에는 각자의 조건이 유지됩니다. 자세한 내용은
[LICENSE](LICENSE)를 확인하세요.
