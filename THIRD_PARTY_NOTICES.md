# Third-Party Notices and Data Rights

Typerlude is a composite project. The Rust and JavaScript software is licensed
under the MIT License in `LICENSE`. Practice word selections written by
Typerlude contributors are released under CC0 1.0 Universal. Third-party
material keeps the license or public-domain status stated below; MIT does not
replace those terms.

Compiled Rust dependencies keep their upstream terms. Their crate names,
versions, repositories, SPDX expressions, and complete license texts are
bundled in `THIRD_PARTY_LICENSES.html`. In particular, `option-ext` 0.2.0 is
MPL-2.0 licensed; its corresponding source is available from the upstream
repository at commit `272f22fc9ea1ac6b08f01704af52c4ac338df4e2`:
https://github.com/soc/option-ext/tree/272f22fc9ea1ac6b08f01704af52c4ac338df4e2.
The Typerlude MIT license does not replace or restrict the MPL-2.0 terms.

The complete offline legal texts are bundled under `assets/licenses/` in the
source tree, Cargo package, and root npm package, and under `licenses/` in
native npm packages and native archives. These layouts contain the same
snapshot bytes for `CC0-1.0.txt`, `CC-BY-2.0-FR.txt`, `CC-BY-4.0.txt`,
`CC-BY-SA-4.0.txt`, and `NORD-MIT.txt`.
The canonical online texts are:

- CC0 1.0 Universal: https://creativecommons.org/publicdomain/zero/1.0/legalcode.txt
- Creative Commons Attribution 2.0 France (`CC-BY-2.0-FR`):
  https://creativecommons.org/licenses/by/2.0/fr/legalcode.fr
- Creative Commons Attribution 4.0 International (`CC-BY-4.0`):
  https://creativecommons.org/licenses/by/4.0/legalcode.txt
- Creative Commons Attribution-ShareAlike 4.0 International (`CC-BY-SA-4.0`):
  https://creativecommons.org/licenses/by-sa/4.0/legalcode.txt
- Nord MIT license: https://github.com/nordtheme/nord/blob/1cef71605416a222e57225b544540ce0fcec18d4/license

The first two Creative Commons sources were retrieved `2026-08-07`; the 4.0
sources were retrieved `2026-08-12`:

| Shipped file | Official source | Source bytes / SHA-256 | Shipped bytes / SHA-256 |
| --- | --- | --- | --- |
| `assets/licenses/CC0-1.0.txt` | https://creativecommons.org/publicdomain/zero/1.0/legalcode.txt | 7048 / `a2010f343487d3f7618affe54f789f5487602331c0a8d03f49e9a7c547cf0499` | 7048 / `a2010f343487d3f7618affe54f789f5487602331c0a8d03f49e9a7c547cf0499` |
| `assets/licenses/CC-BY-2.0-FR.txt` | https://creativecommons.org/licenses/by/2.0/fr/legalcode.fr | 39707 / `af0d7ada8b9be52a6874238f4533512d0b2568595bf7cb3427e41f7c38847b71` | 15978 / `94690c30fa9b7650a55ea91f9158e3dab81a5e3a79ec1e07d9b0be8a5212b81a` |
| `assets/licenses/CC-BY-4.0.txt` | https://creativecommons.org/licenses/by/4.0/legalcode.txt | 18657 / `9ba9550ad48438d0836ddab3da480b3b69ffa0aac7b7878b5a0039e7ab429411` | 18657 / `9ba9550ad48438d0836ddab3da480b3b69ffa0aac7b7878b5a0039e7ab429411` |
| `assets/licenses/CC-BY-SA-4.0.txt` | https://creativecommons.org/licenses/by-sa/4.0/legalcode.txt | 20138 / `28a9529c7d0bb4dc51f4bf5c116a3d16ef247a052f7591466768ddf563fd1cf5` | 20138 / `28a9529c7d0bb4dc51f4bf5c116a3d16ef247a052f7591466768ddf563fd1cf5` |

The CC0 and both 4.0 files are official plain-text responses byte-for-byte.
The French license file is a faithful, complete plain-text rendering of only
the official HTML `plain-text-marker` legal-code container. The official HTML endpoint was
byte-identical to the Creative Commons `cc-legal-tools-data` source at commit
`56f8f157d9d395f48683cae3695973665f0c9162`:
https://github.com/creativecommons/cc-legal-tools-data/blob/56f8f157d9d395f48683cae3695973665f0c9162/docs/licenses/by/2.0/fr/legalcode.fr.html.

## Tatoeba sentence exports

The Korean and English sentence/quote packs select rows from
[Tatoeba](https://tatoeba.org/) official exports. The complete frozen audit inputs are
`assets/sources/tatoeba-selection.json` and
`assets/sources/tatoeba-snapshot.json`. The stable page for sentence ID
`<sentence-id>` is https://tatoeba.org/en/sentences/show/<sentence-id>. Every
selected item records `source_id = tatoeba:<sentence-id>`, that stable page,
the export's license and license URL, `modified = false`, and retrieval date
`2026-08-07`.

Retrieved: `2026-08-07`

| Frozen key | Official archive URL | Compressed bytes | Compressed SHA-256 | Decompressed bytes | Decompressed SHA-256 |
| --- | --- | ---: | --- | ---: | --- |
| `kor_detailed` | https://downloads.tatoeba.org/exports/per_language/kor/kor_sentences_detailed.tsv.bz2 | 304243 | `ffc266c1db3855728ee382b30530f854e3bf53760fb7c2bb36ddbd14d94efd96` | 1619506 | `d9ca57a59406753fea5510784ab60b778c713d5155cbf36fe84bc8909bda1687` |
| `kor_base` | https://downloads.tatoeba.org/exports/per_language/kor/kor_sentences_base.tsv.bz2 | 93636 | `805c8e9ecbf6c8e0ad34e05deba03403f30af5310edfb2de3f66140d7965c1f6` | 249817 | `8fe248bd4d755432cfe1894ca94c90b53b33946e3929d73fe137646a5455fcd8` |
| `eng_cc0` | https://downloads.tatoeba.org/exports/per_language/eng/eng_sentences_CC0.tsv.bz2 | 1288524 | `6ab169264a28008c25bf63042bf7535fc63137c9d7e09b7b8bd7812d10117d1b` | 4734738 | `9e8b3d587be1bd7cf299e09387aeec5707d48d988e1bea14cba091ebc5250262` |

Typerlude added only selection, packaging, and metadata. The sentence text is
unchanged: it was not normalized or otherwise modified (`modified = false`).

### Korean: CC BY 2.0 France

The Korean rows come from Tatoeba's detailed export under Creative Commons
Attribution 2.0 France (`CC-BY-2.0-FR`),
https://creativecommons.org/licenses/by/2.0/fr/. They were joined to the
official Korean base export and are all original rows (`base = 0`). Attribution
is grouped below by the actual contributor frozen in the detailed export; the
numbers are Tatoeba sentence IDs.

- H_Liliom (1): 6960364
- KoreanBeaver (31): 2655609, 2655644, 2655646, 2655670, 2655671,
  2655674, 2655679, 2655680, 2655683, 2655686, 2655688, 2655690,
  2655695, 2655699, 2655721, 2656056, 2656061, 2658153, 2658155,
  2658157, 2658166, 2658167, 2658170, 2658171, 2658180, 2658132,
  2655650, 2655654, 2655655, 2655656, 2655666
- Queserasera (9): 3712940, 3718218, 3718338, 6304350, 6304353,
  6305023, 6305025, 6305030, 6305031
- Sudajaengi (41): 906593, 906595, 906600, 906602, 906608, 906613,
  906621, 906624, 906642, 910371, 914555, 942427, 943271, 943331,
  951947, 982062, 1022631, 1022661, 1022647, 1218625, 1218633,
  1218636, 1218638, 1218649, 1218669, 1219115, 1219121, 1219141,
  1219159, 1219179, 1219181, 1637128, 1637156, 1637158, 1637160,
  1637171, 906615, 906604, 5261931, 1022640, 1022653
- atitarev (10): 13187817, 13147201, 13146473, 13146489, 13148497,
  13130044, 13167418, 13170177, 13187950, 13205186
- cueyayotl (4): 1278329, 2856662, 2870614, 418360
- intertime (14): 11104746, 11104747, 11104748, 11104759, 11104763,
  11104764, 11104768, 11104749, 11104750, 11104751, 11104752,
  11104760, 11104761, 11104770
- rainbow4us (4): 3718270, 3718354, 4950397, 4952644
- rbbrbb (1): 3655501
- ssuss32 (4): 10572153, 10572155, 10572158, 3718343
- wubbol (1): 331283

### English: explicit CC0 export

The English rows come only from Tatoeba's dedicated explicit-CC0 export under
CC0 1.0 Universal (`CC0-1.0`),
https://creativecommons.org/publicdomain/zero/1.0/. That export does not include
individual usernames, so the truthful collective attribution label is
`Tatoeba CC0 contributors`.

- Tatoeba CC0 contributors (120): 331259, 337215, 403860, 763114,
  2694734, 2694737, 2694740, 2694741, 2774346, 2804844, 3289035,
  3396931, 5047246, 8530262, 8679639, 8679712, 5047273, 5143055,
  5153393, 5159451, 5200520, 5342480, 6121806, 9158362, 6121968,
  6122006, 6319960, 6366618, 6718211, 6893978, 6900474, 7036151,
  7104289, 7108649, 8356983, 7166943, 7492431, 7545457, 7632877,
  7677650, 8502928, 8895474, 7742013, 7744148, 7744152, 7744157,
  7756804, 7792164, 7792237, 7799809, 7807073, 7807077, 7810087,
  7842725, 7865527, 7880677, 8415581, 8415585, 8415601, 11476060,
  7912910, 8636820, 8485239, 7948703, 7969669, 9415798, 8567766,
  7977020, 7977027, 7983650, 8573070, 7983678, 8589152, 7987815,
  8011630, 8014168, 8022176, 8047575, 8050968, 8089773, 8089833,
  8104349, 9054425, 8105963, 8606090, 8121223, 8612859, 8123782,
  8612869, 8180113, 8202928, 8286325, 8289409, 9414186, 9805138,
  8302381, 8612882, 8310548, 8613709, 8354372, 334553, 384016,
  9147454, 6824078, 7052915, 9158885, 9158888, 7087093, 7104386,
  8033223, 8037983, 8354398, 8354700, 8354702, 9161891, 8356841,
  9193323, 9227170, 8356993, 8357331

## Korean public-domain literature and Aegukga

The Korean long-text pack contains no project-authored or AI-authored prose.
The 18 literary works below were selected from the Korea Copyright Commission
Gongu Madang catalog as copyright-expired works. Gongu Madang explains that a
work whose copyright term has expired may be used without the copyright
holder's permission:
https://gongu.copyright.or.kr/gongu/main/contents.do?menuNo=200091.

### Complete Korean stories

The 15 complete stories use Korean Wikisource transcriptions because the
Gongu Madang TXT/HWP line breaks split words in ways unsuitable for typing.
The original works remain public domain; the community transcription and this
modified typing edition are distributed under `CC-BY-SA-4.0`:
https://creativecommons.org/licenses/by-sa/4.0/. Attribution is supplied by
the original author, the collective label `한국어 위키문헌 기여자`, and a
permanent revision URL whose page history identifies contributors, as allowed
by the Wikimedia Terms of Use:
https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use.

| Item and original author | Permanent transcription source | Gongu Madang expiration record |
| --- | --- | --- |
| `ko-text-classic-buckwheat-season`, 메밀꽃 필 무렵 — 이효석 | `kowikisource:2584:426852`; https://ko.wikisource.org/w/index.php?title=%EB%A9%94%EB%B0%80%EA%BD%83%20%ED%95%84%20%EB%AC%B4%EB%A0%B5&oldid=426852 | G905-9001211; https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=9001211 |
| `ko-text-classic-camellias`, 동백꽃 — 김유정 | `kowikisource:1923:247658`; https://ko.wikisource.org/w/index.php?title=%EB%8F%99%EB%B0%B1%EA%BD%83&oldid=247658 | G905-9000397; https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=9000397 |
| `ko-text-classic-spring-spring`, 봄봄 — 김유정 | `kowikisource:2036:245881`; https://ko.wikisource.org/w/index.php?title=%EB%B4%84%EB%B4%84&oldid=245881 | G905-9000404; https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=9000404 |
| `ko-text-classic-lucky-day`, 운수 좋은 날 — 현진건 | `kowikisource:2370:457472`; https://ko.wikisource.org/w/index.php?title=%EC%9A%B4%EC%88%98%20%EC%A2%8B%EC%9D%80%20%EB%82%A0&oldid=457472 | G905-9002094; https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=9002094 |
| `ko-text-classic-b-superintendent-love-letter`, B사감과 러브레터 — 현진건 | `kowikisource:2369:425616`; https://ko.wikisource.org/w/index.php?title=B%EC%82%AC%EA%B0%90%EA%B3%BC%20%EB%9F%AC%EB%B8%8C%EB%A0%88%ED%84%B0&oldid=425616 | G905-9002100; https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=9002100 |
| `ko-text-classic-poor-wife`, 빈처 — 현진건 | `kowikisource:4486:250704`; https://ko.wikisource.org/w/index.php?title=%EB%B9%88%EC%B2%98&oldid=250704 | G905-9002092; https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=9002092 |
| `ko-text-classic-wings`, 날개 — 이상 | `kowikisource:1370:221326`; https://ko.wikisource.org/w/index.php?title=%EB%82%A0%EA%B0%9C&oldid=221326 | G905-9000973; https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=9000973 |
| `ko-text-classic-potatoes`, 감자 — 김동인 | `kowikisource:1884:250692`; https://ko.wikisource.org/w/index.php?title=%EA%B0%90%EC%9E%90&oldid=250692 | G905-9000075; https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=9000075 |
| `ko-text-classic-baettaragi`, 배따라기 — 김동인 | `kowikisource:1885:250693`; https://ko.wikisource.org/w/index.php?title=%EB%B0%B0%EB%94%B0%EB%9D%BC%EA%B8%B0&oldid=250693 | G905-9000094; https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=9000094 |
| `ko-text-classic-readymade-life`, 레디메이드 인생 — 채만식 | `kowikisource:17184:223316`; https://ko.wikisource.org/w/index.php?title=%EB%A0%88%EB%94%94%EB%A9%94%EC%9D%B4%EB%93%9C%20%EC%9D%B8%EC%83%9D&oldid=223316 | G905-9001386; https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=9001386 |
| `ko-text-classic-gold-bean-field`, 금 따는 콩밭 — 김유정 | `kowikisource:1920:354124`; https://ko.wikisource.org/w/index.php?title=%EA%B8%88%20%EB%94%B0%EB%8A%94%20%EC%BD%A9%EB%B0%AD&oldid=354124 | G905-9000424; https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=9000424 |
| `ko-text-classic-society-offers-drink`, 술 권하는 사회 — 현진건 | `kowikisource:4002:425047`; https://ko.wikisource.org/w/index.php?title=%EC%88%A0%20%EA%B6%8C%ED%95%98%EB%8A%94%20%EC%82%AC%ED%9A%8C&oldid=425047 | G905-9002096; https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=9002096 |
| `ko-text-classic-mad-painter`, 광화사 — 김동인 | `kowikisource:1925:387811`; https://ko.wikisource.org/w/index.php?title=%EA%B4%91%ED%99%94%EC%82%AC&oldid=387811 | G905-9000079; https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=9000079 |
| `ko-text-classic-manmubang`, 만무방 — 김유정 | `kowikisource:4475:157696`; https://ko.wikisource.org/w/index.php?title=%EB%A7%8C%EB%AC%B4%EB%B0%A9&oldid=157696 | G905-9000403; https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=9000403 |
| `ko-text-classic-fiery-sonata`, 광염 소나타 — 김동인 | `kowikisource:17174:143125`; https://ko.wikisource.org/w/index.php?title=%EA%B4%91%EC%97%BC%20%EC%86%8C%EB%82%98%ED%83%80&oldid=143125 | G905-9000078; https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=9000078 |

Typerlude removed page furniture and parenthetical Hanja, converted curved
quotes, middle dots, ellipses, and long dashes to ordinary keyboard characters,
and retained the complete story. Every item declares `modified = true` and was
retrieved `2026-08-12`. The modified transcription remains under
`CC-BY-SA-4.0`; the full offline license is
`assets/licenses/CC-BY-SA-4.0.txt`.

### Korean poems

The following copyright-expired Gongu Madang texts were retrieved
`2026-08-12`. They use `LicenseRef-Public-Domain`; no Creative Commons license
is asserted over the underlying poems.

| Item | Author and source ID | Official source |
| --- | --- | --- |
| `ko-text-poem-counting-stars`, 별 헤는 밤 | 윤동주; `G905-13313879` | https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=13313879 |
| `ko-text-poem-silence-of-beloved`, 님의 침묵 | 한용운; `G905-9001830` | https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=9001830 |
| `ko-text-poem-when-that-day-comes`, 그날이 오면 | 심훈; `G905-13313827` | https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200019&wrtSn=13313827 |

For direct keyboard practice, Hanja was rendered in Hangul, typographic
punctuation was replaced with ordinary keyboard punctuation, and obvious
spacing or historical-spelling barriers were normalized. Each poem therefore
declares `modified = true`.

### Aegukga, verses 1-4

`ko-text-aegukga-verses-1-4`, 애국가 1절부터 4절, contains only the four
lyric verses; no recording, arrangement, or performance is bundled. Source ID
`G905-13211046`, attributed as `작사자 미상; 기증 안익태`, was retrieved
`2026-08-12` from the Korea Copyright Commission:
https://gongu.copyright.or.kr/gongu/wrt/wrt/view.do?menuNo=200020&wrtSn=13211046.
The Commission identifies the item as donated and gives the free-use example
`애국가, 기증 안익태, 공유마당, CC BY 4.0`:
https://gongu.copyright.or.kr/gongu/main/contents.do?menuNo=200092.
This item is distributed under `CC-BY-4.0`,
https://creativecommons.org/licenses/by/4.0/, and declares `modified = true`
because verse numbers were omitted. The full offline license is
`assets/licenses/CC-BY-4.0.txt`.

## English public-domain literature

The 17 English literary items are term-expired works by authors who died no
later than 1946. Their text was obtained from the Project Gutenberg editions
listed below. Project Gutenberg explains that after its license and trademark
references are stripped from an unrestricted ebook, the remaining text is
unrestricted under U.S. intellectual-property law:
https://www.gutenberg.org/policy/license.html.

| Item | Original author and source ID | Edition landing page |
| --- | --- | --- |
| `en-text-classic-alice-chapter-1`, Alice's Adventures in Wonderland - Chapter I | Lewis Carroll; `gutenberg:11:chapter-1` | https://www.gutenberg.org/ebooks/11 |
| `en-text-classic-pride-chapter-1`, Pride and Prejudice - Chapter I | Jane Austen; `gutenberg:1342:chapter-1` | https://www.gutenberg.org/ebooks/1342 |
| `en-text-classic-christmas-carol-stave-1`, A Christmas Carol - Stave I | Charles Dickens; `gutenberg:46:stave-1` | https://www.gutenberg.org/ebooks/46 |
| `en-text-classic-scandal-bohemia-part-1`, A Scandal in Bohemia - Part I | Arthur Conan Doyle; `gutenberg:1661:scandal-in-bohemia-part-1` | https://www.gutenberg.org/ebooks/1661 |
| `en-text-classic-time-machine-chapter-1`, The Time Machine - Chapter I | H. G. Wells; `gutenberg:35:chapter-1` | https://www.gutenberg.org/ebooks/35 |
| `en-text-classic-frankenstein-chapter-1`, Frankenstein - Chapter 1 | Mary Shelley; `gutenberg:84:chapter-1` | https://www.gutenberg.org/ebooks/84 |
| `en-text-classic-moby-dick-chapter-1`, Moby-Dick - Chapter 1 | Herman Melville; `gutenberg:2701:chapter-1` | https://www.gutenberg.org/ebooks/2701 |
| `en-text-classic-treasure-island-chapter-1`, Treasure Island - Chapter I | Robert Louis Stevenson; `gutenberg:120:chapter-1` | https://www.gutenberg.org/ebooks/120 |
| `en-text-classic-little-women-chapter-1`, Little Women - Chapter One | Louisa May Alcott; `gutenberg:514:chapter-1` | https://www.gutenberg.org/ebooks/514 |
| `en-text-classic-wizard-oz-chapter-1`, The Wonderful Wizard of Oz - Chapter I | L. Frank Baum; `gutenberg:55:chapter-1` | https://www.gutenberg.org/ebooks/55 |
| `en-text-classic-secret-garden-chapter-1`, The Secret Garden - Chapter I | Frances Hodgson Burnett; `gutenberg:17396:chapter-1` | https://www.gutenberg.org/ebooks/17396 |
| `en-text-classic-sleepy-hollow`, The Legend of Sleepy Hollow | Washington Irving; `gutenberg:41:complete` | https://www.gutenberg.org/ebooks/41 |
| `en-text-classic-gift-magi`, The Gift of the Magi | O. Henry; `gutenberg:7256:complete` | https://www.gutenberg.org/ebooks/7256 |
| `en-text-classic-last-leaf`, The Last Leaf | O. Henry; `gutenberg:3707:last-leaf` | https://www.gutenberg.org/ebooks/3707 |
| `en-text-classic-happy-prince`, The Happy Prince | Oscar Wilde; `gutenberg:902:happy-prince` | https://www.gutenberg.org/ebooks/902 |
| `en-text-classic-dracula-chapter-1`, Dracula - Chapter I | Bram Stoker; `gutenberg:345:chapter-1` | https://www.gutenberg.org/ebooks/345 |
| `en-text-classic-jane-eyre-chapter-1`, Jane Eyre - Chapter I | Charlotte Bronte; `gutenberg:1260:chapter-1` | https://www.gutenberg.org/ebooks/1260 |

Only a complete named chapter, stave, or story is bundled; no arbitrary
mid-work excerpt is used. Project Gutenberg headers, footers, license text,
illustration captions, and trademark references were removed. Curved quotes,
long dashes, ellipses, emphasis markers, and non-ASCII letters were converted
to directly typable ASCII and wrapped source lines were reflowed into
paragraphs. Every item declares `modified = true`, was retrieved `2026-08-12`,
and uses `LicenseRef-Public-Domain`.

## Public-domain official documents

### 대한민국헌법 제1조부터 제5조

`ko-text-constitution-articles-1-5` contains Korean Constitution Articles 1
through 5, source ID `rok-constitution:articles-1-5`, retrieved `2026-08-07`
from the official National Law Information Center text:
https://www.law.go.kr/법령/대한민국헌법.

For direct keyboard practice, the official circled paragraph numerals `①` and
`②` were transcribed as `(1)` and `(2)` without changing the wording. This item
therefore declares `modified = true`.

The public-domain basis is Korean Copyright Act Article 7, which excludes the
Constitution and other official edicts from protected works:
https://www.law.go.kr/법령/저작권법/제7조. The National Law Information
Center's official reuse policy expressly permits unrestricted reuse, including
commercial reuse, of Article 7 material:
https://www.law.go.kr/lawPetitionForm.do?menuId=13&subMenuId=79.

### The Declaration of Independence

`en-text-declaration-independence`, The Declaration of Independence, contains
the complete official transcript attributed to the Second Continental
Congress. Source ID `nara:declaration-transcript`, retrieved `2026-08-12`:
https://www.archives.gov/founding-docs/declaration-transcript. The National
Archives download page marks the founding-document images public domain:
https://www.archives.gov/founding-docs/downloads. The item is unchanged and
uses `LicenseRef-Public-Domain`.

### The Gettysburg Address

`en-text-gettysburg-address`, The Gettysburg Address, is attributed to Abraham
Lincoln and follows the standard Bliss-copy wording published by the National
Park Service. Source ID `nps:gettysburg-address:bliss-copy`, retrieved
`2026-08-12`:
https://www.nps.gov/linc/learn/historyculture/gettysburgaddress.htm. Tildes used
as separators on the web page were rendered as `--`, so the item declares
`modified = true`. Its 1863 publication is term-expired; the U.S. Copyright
Office explains copyright expiration and the public domain at
https://copyright.gov/what-is-copyright/. The item uses
`LicenseRef-Public-Domain`.

### U.S. Constitution, Article I, Section 2, Clauses 1-2

`en-text-constitution-article-1-section-2-clauses-1-2` contains exactly Article
I, Section 2, Clauses 1-2, source ID
`us-constitution:article-1-section-2-clauses-1-2`, attributed to the
Constitutional Convention of 1787 and retrieved `2026-08-07` from the official
Congress Constitution Annotated text:
https://constitution.congress.gov/constitution/article-1/.

The public-domain basis is expiration of the 1787 publication's copyright term.
The U.S. Copyright Office explains expiration and the public domain at
https://copyright.gov/what-is-copyright/; as of 2026, all U.S. works published
before 1931 are public domain. The exact clauses were cross-checked against the
National Archives official transcript at
https://www.archives.gov/founding-docs/constitution-transcript. Its official
download page expressly marks the Constitution images public domain:
https://www.archives.gov/founding-docs/downloads.

This basis is term expiration, not 17 U.S.C. § 105. Section 105 concerns U.S.
federal-government material such as the federal page or transcription; it is
not a claim that the federal government authored the 1787 Constitution.

## Nord theme palette

The bundled `nord` theme uses the official
[Nord](https://github.com/nordtheme/nord) palette, package 0.2.1, pinned at
commit `1cef71605416a222e57225b544540ce0fcec18d4`. The palette source is
https://raw.githubusercontent.com/nordtheme/nord/1cef71605416a222e57225b544540ce0fcec18d4/src/nord.css
(5,380 bytes, SHA-256
`b931ac3732582b2066b2d6cadec02d9820ba7081e6e3e404c31cb62d9315a962`).
Typerlude maps unchanged hex values from that palette to its theme roles:
`#2e3440` background, `#d8dee9` foreground, `#88c0d0` accent, `#a3be8c`
correct, `#bf616a` error, `#ebcb8b` cursor, and `#81a1c1` dim. The dim value
provides at least 4.5:1 contrast against the background. Typerlude also renders
errors bold and underlined, and renders the cursor bold and reversed, so color
is not the only signal for either role.

Nord is distributed under the MIT License. Its byte-identical license is
bundled at `assets/licenses/NORD-MIT.txt` (1,132 bytes, SHA-256
`25ac8188d670bd2ad2ce2f4f55ab88573010ee9f7a4502543cb1eea1e2274f8a`).
Copyright (c) 2016-present Sven Greb <development@svengreb.de> (https://www.svengreb.de).
