# Boss Battle

<p align="center"><strong>English</strong> · <a href="boss-battle.ko.md">한국어</a></p>

Boss Battle is a Korean or English typing encounter available from **Games → Boss Battle**.
Every boss uses the same battle rules but has a different typing pattern, pressure mechanic, and
phase-two behavior.

## Battle rules

- Defeat the boss within 90 seconds of active battle time while protecting three hearts.
- Reaching half health starts phase two. Skill and transition cinematics lock input and pause the
  active timer.
- `Esc` pauses. Press `q` or `ㅂ` twice while paused to leave; `Enter` retries from the result screen.
- Paste is ignored. `Backspace` edits normal input, but some boss mechanics punish a wrong key
  immediately.

## Progression

Difficulty unlocks separately for each boss. A clear also fills that boss's stars.

| Clear | Stars | Unlocks |
| --- | --- | --- |
| None | ☆☆☆ | IRON WARDEN Easy |
| Easy | ★☆☆ | The same boss on Medium and the next boss on Easy |
| Medium | ★★☆ | The same boss on Hard |
| Hard | ★★★ | Full clear for that boss |

Stars and difficulty unlocks are shared between Korean and English. Personal best scores are saved
separately for each boss, language, and difficulty.

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
- **Failure:** Any blooming vine costs one heart and refreshes the lanes.
- **Phase two:** A third simultaneous vine joins the field.
- **Signature:** THORN BLOOM and parallel target selection.

### 3. NULL ARCHON

- **Pattern:** Complete three checksum prompts in a row to reverse the void canticle and deal bonus
  damage.
- **Failure:** A wrong key clears the current input and removes one stored checksum. If the canticle
  completes, you lose one heart.
- **Phase two:** C MAX locks the system during the transition, then the canticle accelerates.
- **Signature:** C MAX, checksum rollback, and the three-checksum reversal.

## Score and local data

Score rewards victory, remaining time, remaining hearts, accuracy, and maximum combo. Boss Battle
runs are not added to practice session history; clear stars and personal best scores are stored in
the local settings file. Run `typerlude paths` to find it.
