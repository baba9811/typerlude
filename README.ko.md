# Typerlude

A typing interlude for your terminal.

[English](README.md) | [한국어](README.ko.md)

Typerlude는 계정, 텔레메트리, 클라우드 저장소 없이 키, 단어, 문장, 긴 글, 시간제 시험을
연습하는 오프라인 우선 한국어·영어 터미널 타자 연습기입니다.

## 설치

레지스트리 배포 여부는 아직 검증되지 않았습니다. 패키지가 공개된 뒤에는 다음을 사용하세요.

```bash
npm install -g typerlude
typerlude
```

```bash
cargo install typerlude
typerlude
```

그전에는 현재 소스에서 실행하세요.

```bash
git clone https://github.com/baba9811/typerlude.git
cd typerlude
cargo run --release --
```

UTF-8 대화형 터미널을 사용하세요. 권장 최소 크기는 80×24입니다.

## 사용

홈 화면에서 모드를 고르고 옵션을 조정한 뒤 `Enter`를 누르세요. `Tab`, 화살표 키, `j`, `k`로
포커스를 옮기고, `Esc`로 뒤로 가거나 지원 모드를 일시 정지하며, `q`로 종료합니다. 점수 연습
중 붙여넣기는 무시됩니다.

```bash
typerlude --help
typerlude notes.txt
typerlude stats
typerlude history
typerlude paths
typerlude themes
typerlude licenses
```

사용자 텍스트는 메모리에만 남고 세션 기록에 복사되지 않습니다. 로컬 세션 기록에는 집계된
연습 지표와 의도한 키별 횟수만 저장됩니다.

## 더 알아보기

- [Content-pack guide](docs/content-packs.md) / [콘텐츠 팩 안내](docs/content-packs.ko.md)
- [배포 안내](docs/releasing.md)
- [보안 정책](SECURITY.md)
- [라이선스와 제3자 고지](LICENSE) / [자료 권리](THIRD_PARTY_NOTICES.md)

## 개발

```bash
git clone https://github.com/baba9811/typerlude.git
cd typerlude
cargo run --
make test
```

Typerlude는 Rust 1.88.0을 고정합니다. 전체 검증에는 Node.js와 CI에 문서화한 정책 도구도
사용합니다.
