pub const GCSIM_HELP_TEXT: &str = r#"# gcsim GCSL Configuration Guide

gcsim simulates Genshin Impact combat using GCSL (Genshin Combat Script Language).

## Basic Structure
A configuration file consists of:
1. `options` - Simulator settings
2. `character` - Team members, stats, weapons, artifacts
3. `target` - Enemy settings
4. `active` - Starting character
5. Action Queue / Loop - Combat sequence

## Example Template
```
options iteration=1 duration=90;

# Target definition (lvl, hp, pos, resist)
target lvl=100 hp=100000000 pos=0,2 resist=0.1;

# Characters
bennett char lvl=90/90 cons=6 talent=1,13,13;
bennett add weapon="aquilafavonia" refine=1 lvl=90/90;
bennett add set="noblesseoblige" count=4;
bennett add stats hp=4780 atk=311 er=0.518 pyro%=0.466 cr=0.311;

xiangling char lvl=90/90 cons=6 talent=9,9,13;
xiangling add weapon="thecatch" refine=5 lvl=90/90;
xiangling add set="emblemofseveredfate" count=4;
xiangling add stats hp=4780 atk=311 atk%=0.466 pyro%=0.466 cr=0.311;

xingqiu char lvl=90/90 cons=6 talent=9,13,13;
xingqiu add weapon="sacrificialsword" refine=5 lvl=90/90;
xingqiu add set="emblemofseveredfate" count=4;
xingqiu add stats hp=4780 atk=311 atk%=0.466 hydro%=0.466 cr=0.311;

raiden char lvl=90/90 cons=0 talent=9,9,10;
raiden add weapon="engulfinglightning" refine=1 lvl=90/90;
raiden add set="emblemofseveredfate" count=4;
raiden add stats hp=4780 atk=311 er=0.518 electro%=0.466 cr=0.311;

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

## Important Rules & Tips
- `options iteration=1 duration=90;` is recommended for sample testing.
- Swap cooldown is ~1 second (60 frames).
- Actions are separated by commas (`,`).
- Valid actions: `attack`, `charge`, `aim`, `skill`, `burst`, `dash`, `jump`, `swap`.
- Character keys use lowercase names without spaces (e.g. `raiden`, `xiangling`, `hutao`, `kamisatoayaka`).
- Use the `get_gcsim_reference` tool to look up valid keys for characters, weapons, and artifacts.
"#;
