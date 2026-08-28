# Boss Battle

<p align="center"><strong>English</strong> · <a href="boss-battle.ko.md">한국어</a></p>

Boss Battle is a Korean or English typing encounter available from **Games → Boss Battle**.
Every boss uses the same battle rules but has a different typing pattern, pressure mechanic, and
phase-two behavior.

## Battle rules

- Defeat the boss within 90 seconds of active battle time while protecting three hearts.
- Reaching half health starts phase two. Skill and transition cinematics lock input and pause the
  active timer.
- `Esc` pauses. Press `q` or `ㅂ` twice while paused to leave.
- On the result screen, `r` or `ㄱ` retries the same fight. `Enter` or `Esc` returns to that boss's
  options.
- Paste is ignored. `Backspace` edits normal input, but some boss mechanics punish a wrong key
  immediately.

## Progression

Difficulty unlocks separately for each boss. A clear also fills that boss's stars.

| Clear | Stars | Unlocks |
| --- | --- | --- |
| None | ☆☆☆ ✧ | IRON WARDEN Easy |
| Easy | ★☆☆ ✧ | The same boss on Medium and the next boss on Easy |
| Medium | ★★☆ ✧ | The same boss on Hard |
| Hard | ★★★ ✧ | The same boss on Hell |
| Hell | ★★★ ✦ | Final clear for that boss |

Stars and difficulty unlocks are shared between Korean and English. Personal best scores are saved
separately for each boss, language, and difficulty.

## Hell difficulty

Hell uses the Hard word pool with tighter health and timing pressure. Its double-line
`╬ HELL ╬ // REDLINE` frame identifies the tier even without color and uses no terminal blinking.
Balance targets a 540 KPM, 98.5% accurate player clearing every boss in either language in 75–88
seconds. A 420 KPM, 97% accurate player is expected to reach the final stretch but not clear it.

## Boss roster

### 1. IRON WARDEN

- **Pattern:** Complete three prompts to break its armor locks. Prompts typed while the core is
  exposed deal double damage.
- **Failure:** Let PILE DRIVER complete and you lose one heart; all armor locks reset.
- **Phase two:** Armor and core windows become shorter.
- **Signature:** PILE DRIVER and the exposed-core strike window.

### 2. THORN QUEEN

- **Pattern:** Two vines grow at once. The first physical key selects one matching vine; finish it
  before it blooms.
- **Failure:** Any blooming vine costs one heart and replaces that vine.
- **Phase two:** A third simultaneous vine joins the field.
- **Signature:** THORN BLOOM and parallel target selection.

### 3. NULL ARCHON

- **Pattern:** Fill three checksum slots to reverse the void canticle and deal bonus damage.
- **Failure:** A wrong key clears the current input and removes one stored checksum. If the canticle
  completes, you lose one heart.
- **Phase two:** C MAX locks the system during the transition, then the canticle accelerates.
- **Signature:** C MAX, checksum rollback, and the three-checksum reversal.

## Score and local data

Score is the sum of a 10,000-point victory bonus, whole seconds remaining × 100, remaining hearts ×
1,000, accuracy in basis points (100% = 10,000), and maximum combo × 10. A defeat score appears on
the result screen, but only victories update stars and personal bests.

Boss Battle runs are not added to practice session history. Clear stars and personal best scores
are stored in the local settings file; run `typerlude paths` to find it.
