# Third-Party Notices and Data Rights

Typeul is a composite project. The Rust and JavaScript software is licensed
under the MIT License in `LICENSE`. Practice data written by Typeul contributors
is released under CC0 1.0 Universal. Third-party material keeps the license or
public-domain status stated below; MIT does not replace those terms.

The complete offline legal texts are bundled at
`assets/licenses/CC0-1.0.txt`, `assets/licenses/CC-BY-2.0-FR.txt`, and
`assets/licenses/NORD-MIT.txt`.
The canonical online texts are:

- CC0 1.0 Universal: https://creativecommons.org/publicdomain/zero/1.0/legalcode.txt
- Creative Commons Attribution 2.0 France (`CC-BY-2.0-FR`):
  https://creativecommons.org/licenses/by/2.0/fr/legalcode.fr
- Nord MIT license: https://github.com/nordtheme/nord/blob/1cef71605416a222e57225b544540ce0fcec18d4/license

License sources were retrieved `2026-08-07`:

| Shipped file | Official source | Source bytes / SHA-256 | Shipped bytes / SHA-256 |
| --- | --- | --- | --- |
| `assets/licenses/CC0-1.0.txt` | https://creativecommons.org/publicdomain/zero/1.0/legalcode.txt | 7048 / `a2010f343487d3f7618affe54f789f5487602331c0a8d03f49e9a7c547cf0499` | 7048 / `a2010f343487d3f7618affe54f789f5487602331c0a8d03f49e9a7c547cf0499` |
| `assets/licenses/CC-BY-2.0-FR.txt` | https://creativecommons.org/licenses/by/2.0/fr/legalcode.fr | 39707 / `af0d7ada8b9be52a6874238f4533512d0b2568595bf7cb3427e41f7c38847b71` | 15978 / `94690c30fa9b7650a55ea91f9158e3dab81a5e3a79ec1e07d9b0be8a5212b81a` |

The CC0 file is the official plain-text response byte-for-byte. The French
license file is a faithful, complete plain-text rendering of only the official
HTML `plain-text-marker` legal-code container. The official HTML endpoint was
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

Typeul added only selection, packaging, and metadata. The sentence text is
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

## Public-domain Constitution excerpts

Both excerpts use the internal license identifier
`LicenseRef-Public-Domain`; no Creative Commons license is asserted over the
underlying constitutional text.

### 대한민국헌법 제1조부터 제5조

`ko-text-constitution-articles-1-5` contains the exact current Korean
Constitution Articles 1 through 5, source ID
`rok-constitution:articles-1-5`, retrieved `2026-08-07` from the official
National Law Information Center text:
https://www.law.go.kr/법령/대한민국헌법.

The public-domain basis is Korean Copyright Act Article 7, which excludes the
Constitution and other official edicts from protected works:
https://www.law.go.kr/법령/저작권법/제7조. The National Law Information
Center's official reuse policy expressly permits unrestricted reuse, including
commercial reuse, of Article 7 material:
https://www.law.go.kr/lawPetitionForm.do?menuId=13&subMenuId=79.

### U.S. Constitution, Article I, Section 2, Clauses 1–2

`en-text-constitution-article-1-section-2-clauses-1-2` contains exactly Article
I, Section 2, Clauses 1–2, source ID
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

## Typeul-authored practice data and traditional-story retellings

All Typeul-authored word selections, essays, fiction, aphorisms, and retellings
are new Typeul expression released by Typeul contributors under CC0 1.0
Universal. In particular, the four retellings use only the named traditional or
public-domain plot bases below; no wording from a modern edition, translation,
or adaptation was copied.

- `ko-text-retelling-heungbu-nolbu`, “박씨 하나의 몫 — 흥부와 놀부 다시 쓰기”:
  the anonymous traditional Korean narrative 흥부와 놀부.
- `ko-text-retelling-sun-moon-siblings`, “하늘에 남은 두 빛 — 해와 달이 된 오누이 다시 쓰기”:
  the anonymous traditional Korean narrative 해와 달이 된 오누이.
- `en-text-retelling-tortoise-hare`, “Two Ways to the Finish — The Tortoise and
  the Hare Retold”: the ancient Aesopic fable The Tortoise and the Hare.
- `en-text-retelling-stone-soup`, “The Empty Pot — Stone Soup Retold”: the
  traditional European folktale Stone Soup.

## Nord theme palette

The bundled `nord` theme uses the official
[Nord](https://github.com/nordtheme/nord) palette, package 0.2.1, pinned at
commit `1cef71605416a222e57225b544540ce0fcec18d4`. The palette source is
https://raw.githubusercontent.com/nordtheme/nord/1cef71605416a222e57225b544540ce0fcec18d4/src/nord.css
(5,380 bytes, SHA-256
`b931ac3732582b2066b2d6cadec02d9820ba7081e6e3e404c31cb62d9315a962`).
Typeul maps unchanged hex values from that palette to its theme roles:
`#2e3440` background, `#d8dee9` foreground, `#88c0d0` accent, `#a3be8c`
correct, `#bf616a` error, `#ebcb8b` cursor, and `#4c566a` dim.

Nord is distributed under the MIT License. Its byte-identical license is
bundled at `assets/licenses/NORD-MIT.txt` (1,132 bytes, SHA-256
`25ac8188d670bd2ad2ce2f4f55ab88573010ee9f7a4502543cb1eea1e2274f8a`).
Copyright (c) 2016-present Sven Greb <development@svengreb.de> (https://www.svengreb.de).
