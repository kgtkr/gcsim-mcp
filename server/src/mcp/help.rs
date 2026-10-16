pub const GCSIM_HELP_TEXT: &str = r#"# gcsim GCSL Configuration & Artifact Stats Guide

gcsim simulates Genshin Impact combat using GCSL (Genshin Combat Script Language).

## 1. Basic Structure
A configuration file consists of:
1. `options` - Simulator settings (e.g., `options iteration=1 duration=90;`)
2. `character` - Team members, levels, constellations, talents, weapons, artifact sets, and stats
3. `target` - Enemy settings (level, HP, position, resistance)
4. `active` - Starting active character
5. Action Queue / Loop - Combat sequence

---

## 2. Stat Names in GCSL (`add stats`)
In GCSL, character stats are added via `<char_name> add stats <stat_key>=<value> ...;`.
Multiple `add stats` lines for the same character are **cumulative** (summed together), and constant arithmetic expressions like `cr=0.0331*8` are supported by the parser.

### Valid Stat Keys & Units
- **Flat Stats** (written as raw numbers):
  - `hp`: Flat HP
  - `atk`: Flat ATK
  - `def`: Flat DEF
  - `em`: Elemental Mastery
- **Percentage Stats** (written as **decimals** where `1.0 = 100%`, e.g., `46.6%` -> `0.466`):
  - `hp%`: HP%
  - `atk%`: ATK%
  - `def%`: DEF%
  - `er`: Energy Recharge (e.g., `0.518` = 51.8%)
  - `cr`: Crit Rate (e.g., `0.311` = 31.1%)
  - `cd`: Crit DMG (e.g., `0.622` = 62.2%)
  - `heal`: Healing Bonus (e.g., `0.359` = 35.9%)
  - `pyro%`, `hydro%`, `cryo%`, `electro%`, `anemo%`, `geo%`, `dendro%`: Elemental DMG Bonus (e.g., `0.466` = 46.6%)
  - `phys%`: Physical DMG Bonus (e.g., `0.583` = 58.3%)
  - `atkspd%`: Normal Attack SPD%
  - `dmg%`: Generic All-DMG Bonus%

---

## 3. Artifact Main Stat Rules & Values (5★ Lv.20 / 4★ Lv.16)
A character equips 5 artifact pieces. Each slot only permits specific main stats:

| Slot | Allowed Main Stats (GCSL Key) | 5★ Lv.20 Value | 4★ Lv.16 Value |
| :--- | :--- | :--- | :--- |
| **Flower** | `hp` (Fixed) | `4780` | `3571` |
| **Plume** | `atk` (Fixed) | `311` | `232` |
| **Sands** | `hp%`, `atk%` | `0.466` | `0.348` |
| | `def%` | `0.583` | `0.435` |
| | `em` | `186.5` | `139.2` |
| | `er` | `0.518` | `0.387` |
| **Goblet** | `hp%`, `atk%` | `0.466` | `0.348` |
| | `def%` | `0.583` | `0.435` |
| | `em` | `186.5` | `139.2` |
| | `pyro%`, `hydro%`, `cryo%`, `electro%`, `anemo%`, `geo%`, `dendro%` | `0.466` | `0.348` |
| | `phys%` | `0.583` | `0.435` |
| **Circlet** | `hp%`, `atk%` | `0.466` | `0.348` |
| | `def%` | `0.583` | `0.435` |
| | `em` | `186.5` | `139.2` |
| | `cr` | `0.311` | `0.232` |
| | `cd` | `0.622` | `0.464` |
| | `heal` | `0.359` | `0.268` |

---

## 4. Artifact Substat Rules & Roll Mechanics (5★)

### Allowed Substats (10 Types Only)
Only the following 10 stats can appear as artifact substats:
`hp`, `atk`, `def`, `hp%`, `atk%`, `def%`, `em`, `er`, `cr`, `cd`
*(Note: Elemental DMG%, `phys%`, and `heal` NEVER appear as substats.)*

### Main Stat vs. Substat Exclusion Rule
- An artifact piece **cannot** have a substat that is identical to its own **main stat**.
- **Flat and Percentage stats are distinct and CAN coexist on the same piece**:
  - Flower (main stat `hp`) cannot roll flat `hp` substat, but **can** roll `hp%`.
  - Plume (main stat `atk`) cannot roll flat `atk` substat, but **can** roll `atk%`.
  - A Sands with `atk%` main stat cannot roll `atk%` substat, but **can** roll flat `atk`.
  - A Circlet with `cr` main stat cannot roll `cr` substat, but **can** roll `cd`.

### Initial Substats (3-liner vs. 4-liner) & Upgrade Steps
- **At +0 (Initial)**: A 5★ artifact starts with **3 substats** (3-liner, 80% chance in domains) or **4 substats** (4-liner, 20% chance in domains). All 4 substats on a single piece must be distinct.
- **Upgrades (+4, +8, +12, +16, +20 = 5 upgrades)**:
  - **Initial 3-liner**: The +4 upgrade unlocks the 4th substat. The remaining 4 upgrades (+8, +12, +16, +20) strengthen existing substats -> **8 total rolls** on the piece (max **5 rolls** into a single substat if all 4 upgrades hit it).
  - **Initial 4-liner**: All 5 upgrades strengthen existing substats -> **9 total rolls** on the piece (max **6 rolls** into a single substat if all 5 upgrades hit it).
- **Across 5 Artifacts**: A character has **40 to 45 total substat rolls** across 20 substat slots (5 pieces × 4 substats).

### Single-Roll Values (5★ Tiers & KQM Average)
Each substat roll (both initial and upgrade) randomly takes 70%, 80%, 90%, or 100% of the maximum roll value. KQM Standard uses the mean of the 4 tiers (**85%** of max roll):

| Stat Key | 70% (Low) | 80% (Mid) | 90% (High) | 100% (Max) | **KQM Average (1 Roll)** |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `hp` | `209.13` | `239.00` | `268.88` | `298.75` | **`253.94`** |
| `atk` | `13.62` | `15.56` | `17.51` | `19.45` | **`16.54`** |
| `def` | `16.20` | `18.52` | `20.83` | `23.15` | **`19.68`** |
| `hp%` | `0.0408` | `0.0466` | `0.0525` | `0.0583` | **`0.0496`** |
| `atk%` | `0.0408` | `0.0466` | `0.0525` | `0.0583` | **`0.0496`** |
| `def%` | `0.0510` | `0.0583` | `0.0656` | `0.0729` | **`0.0620`** |
| `em` | `16.32` | `18.65` | `20.98` | `23.31` | **`19.82`** |
| `er` | `0.0453` | `0.0518` | `0.0583` | `0.0648` | **`0.0551`** |
| `cr` | `0.0272` | `0.0311` | `0.0350` | `0.0389` | **`0.0331`** |
| `cd` | `0.0544` | `0.0622` | `0.0699` | `0.0777` | **`0.0662`** |

---

## 5. Default Policy When Artifact Stats Are Not Specified (KQM Standard / KQMS)
**IMPORTANT FOR AI AGENTS / LLMs**:
When the user does **not** specify exact artifact stats for one or more characters, you **MUST** allocate their stats according to the **KQM Standard (KQMS)** rules below, and **explicitly present the assumed Main Stats and Substat roll distribution to the user** in your response.

### KQMS Substat Allocation Rules (Total 40 Rolls = 20 Fixed + 20 Liquid)
1. **20 Fixed Rolls (Baseline)**:
   - Assign **2 rolls** (using the **KQM Average** value above) to **all 10 substat types** (`2 × 10 = 20` fixed rolls).
2. **20 Liquid Rolls (Flexible)**:
   - Distribute **20 additional liquid rolls** among useful substats to optimize team DPS (prioritize meeting Energy Recharge `er` needs for smooth rotations, then Crit `cr`/`cd` in a ~1:2 ratio, and scaling stats like `atk%`, `hp%`, `def%`, `em`).
3. **Per-Stat Liquid Roll Cap (`10 - 2 × MainStatCount`)**:
   - A substat can receive at most **2 liquid rolls per artifact piece that does NOT have that stat as its main stat**.
   - Therefore, for any substat:
     - **0 main stats** of that type -> max **10 liquid rolls** (2 fixed + 10 liquid = **12 total rolls**).
     - **1 main stat** of that type (e.g., `atk%` Sands, `cr` Circlet, `hp` Flower, `atk` Plume) -> max **8 liquid rolls** (2 fixed + 8 liquid = **10 total rolls**).
     - **2 main stats** of that type (e.g., `atk%` Sands + `atk%` Goblet) -> max **6 liquid rolls** (2 fixed + 6 liquid = **8 total rolls**).
     - **3 main stats** of that type (e.g., triple `em` Sands/Goblet/Circlet) -> max **4 liquid rolls** (2 fixed + 4 liquid = **6 total rolls**).
4. **4★ Artifact Set Adjustment (e.g., 4pc Instructor / The Exile)**:
   - For each 4★ artifact equipped (`N` pieces, typically `N=4` with 1× 5★ off-piece):
     - Reduce total liquid rolls by `2 × N` (e.g., for `N=4`, total liquid rolls = `20 - 8 = 12`).
     - Multiply all substat single-roll values by `1 - 0.04 × N` (e.g., for `N=4`, multiply by `0.84`).

### Example Template Using KQMS (Separate Main Stats & Substats Lines)
```gcsl
options iteration=1 duration=90;

# Target definition (lvl, hp, pos, resist)
target lvl=100 hp=100000000 pos=0,2 resist=0.1;

# Characters (Main stats on line 1, KQMS substats on line 2 using <avg_roll>*<total_rolls>)
bennett char lvl=90/90 cons=6 talent=1,13,13;
bennett add weapon="aquilafavonia" refine=1 lvl=90/90;
bennett add set="noblesseoblige" count=4;
bennett add stats hp=4780 atk=311 er=0.518 pyro%=0.466 cr=0.311;
bennett add stats def%=0.062*2 def=19.68*2 hp=253.94*2 hp%=0.0496*2 atk=16.54*2 atk%=0.0496*2 er=0.0551*10 em=19.82*2 cr=0.0331*6 cd=0.0662*10;

xiangling char lvl=90/90 cons=6 talent=9,9,13;
xiangling add weapon="thecatch" refine=5 lvl=90/90;
xiangling add set="emblemofseveredfate" count=4;
xiangling add stats hp=4780 atk=311 em=186.5 pyro%=0.466 cr=0.311;
xiangling add stats def%=0.062*2 def=19.68*2 hp=253.94*2 hp%=0.0496*2 atk=16.54*2 atk%=0.0496*4 er=0.0551*6 em=19.82*2 cr=0.0331*8 cd=0.0662*10;

xingqiu char lvl=90/90 cons=6 talent=9,13,13;
xingqiu add weapon="sacrificialsword" refine=5 lvl=90/90;
xingqiu add set="emblemofseveredfate" count=4;
xingqiu add stats hp=4780 atk=311 atk%=0.466 hydro%=0.466 cr=0.311;
xingqiu add stats def%=0.062*2 def=19.68*2 hp=253.94*2 hp%=0.0496*2 atk=16.54*2 atk%=0.0496*4 er=0.0551*4 em=19.82*2 cr=0.0331*10 cd=0.0662*10;

raiden char lvl=90/90 cons=0 talent=9,9,10;
raiden add weapon="engulfinglightning" refine=1 lvl=90/90;
raiden add set="emblemofseveredfate" count=4;
raiden add stats hp=4780 atk=311 er=0.518 electro%=0.466 cr=0.311;
raiden add stats def%=0.062*2 def=19.68*2 hp=253.94*2 hp%=0.0496*2 atk=16.54*2 atk%=0.0496*6 er=0.0551*2 em=19.82*2 cr=0.0331*10 cd=0.0662*10;

active raiden;

# Action Loop
while 1 {
    raiden skill;
    xingqiu burst, attack, skill;
    bennett burst, attack, skill;
    xiangling burst, attack, skill;
    raiden burst, attack:3, charge, attack:3, charge;
}
```

## 6. General Combat Script Tips
- `options iteration=1 duration=90;` is recommended for sample testing.
- Swap cooldown is 1 second (60 frames).
- Actions are separated by commas (`,`).
- Valid actions: `attack`, `charge`, `high_plunge`, `low_plunge`, `aim`, `skill`, `burst`, `dash`, `jump`, `walk`, `swap`.
- Character keys use lowercase names without spaces (e.g. `raiden`, `xiangling`, `hutao`, `kamisatoayaka`).
- Use the `get_gcsim_reference` tool to look up valid keys for characters, weapons, and artifacts.
"#;
