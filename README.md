# Iron's Spellbooks — Horizon port (Saison 2)

Mana, scrolls, spellbooks, mage gear and a core set of spells for Horizon:
a **content mod** (`.hmod`, the items, recipes and art) plus a **native trait
mod** (`.hnmod`, the casting, the mana pool and the tooltips).

Upstream Iron's Spellbooks is **All-Rights-Reserved**. Nothing of its code, data
or art is used here: the spells, their numbers, their wording and every texture
are written for this port, from the public description of how the mod plays.
The `tools/gen_content.py` script is the origin of the generated JSON and PNG
files, so they can be regenerated and audited.

## Layout

| Path | What |
|---|---|
| `content/` | `horizon-mod.toml` + items, recipes, creative tab, lang, models, textures → `target/irons_spellbooks.hmod` |
| `mixins/irons_spellbooks/` | the trait mod bundle: `horizon-mixin.toml` + `api/` → `irons_spellbooks.hnmod` |
| `tools/` | `gen_content.py` (art + data), `pack.py` (local `.hmod`), `vps_stage.sh` (stage the trait mod into the build worktree) |

## Done

**Native (server and client)**

- `mana.rs`: a player's pool — 100 base, 25 per mage armour piece worn, +1 % of
  the maximum a second, per-spell cooldowns. Saved with the player as the
  `irons_spellbooks:pool` attachment (codec, copied on death).
- `spells.rs`: 10 spells with school, level cap, mana cost, cooldown, strength,
  range and summary, plus Roman numerals and second formatting.
- `aim.rs`: yaw/pitch direction, cast origin at the eye, blink distance clamped
  by the crosshair's block hit, `Motion` and coordinate formatting.
- `book.rs`: scrolls (`irons_spellbooks:scroll_<spell>`) and spellbooks
  (5/8/10 pages) read from `minecraft:custom_data.isb` (`l=3`, `s=0;b=heal:2`),
  inscribing rules, page cycling and tearing a page out.
- Hooks: `player_tick` (regen, armour bonus, cooldowns, mana message),
  `use_item` (scroll cast + consume, spellbook cast, sneak+use to turn the
  page), `tooltip` (school, level, mana, rest, effect and how to use),
  `hud` (mana bar in the bottom-right corner), `net_from_server` (the bar's
  value).
- Casting runs on the server: projectiles are spawned with `Host::summon`
  (`small_fireball`, `arrow` with `damage`, `breeze_wind_charge`) carrying
  their `Motion`, lightning is a summoned `lightning_bolt` where the crosshair
  meets a block, blink is `teleport_player`, and heal and fortify run `/effect`
  as the caster (`run_command_as`, since `run_command` is the console's and
  cannot run inside a player's use hook). No item is consumed in creative.

**Content**

- 10 scrolls (paper, stack of 16, uncommon), 3 spellbooks (1 page stack,
  uncommon/rare/epic), the wizard armour set, the arcane staff (+15 % spell
  power while carried) and arcane essence.
- Crafting recipes for all of them, a creative tab, English and French names,
  item models and original 16×16 textures.

## Missing

- **Spell effects are approximations**, because the trait API has no entity
  query or custom projectile: Firebolt/Fireball are small fireballs, Icicle and
  Magic Missile are arrows (Magic Missile without gravity), Gust is a breeze
  wind charge. They deal vanilla projectile damage, not the table's numbers.
- **Fireball has no area damage** and **Gust does not push mobs around the
  caster** (it throws a wind charge instead).
- **No spellcasting animation, particles, sounds or boss loot**: casting plays
  no sound yet.
- **Scrolls and spellbooks cannot be inscribed in game**: `book.rs` implements
  inscribing, but no hook combines a scroll with a spellbook yet (the API has no
  off-hand read and no anvil-like interaction). Admin path:
  `/give @s irons_spellbooks:apprentice_spellbook[minecraft:custom_data={isb:"s=0;b=firebolt:2,heal:1"}]`
  and `/give @s irons_spellbooks:scroll_heal[minecraft:custom_data={isb:"l=3"}]`.
- **Heal and Fortify need the caster's own permissions**: they run `/effect`
  as the caster, so a survival player who is not an operator is told the mana
  was spent while nothing happens (the mana is refunded only when the spell
  helper itself fails).
- **Sneak + use does not change the page the spellbook casts**: the message
  names the next page, but the use hook is handed an immutable stack and the
  API has no helper that writes the held stack back, so the selection saved in
  `isb` (`s=`) never changes and every cast uses the page already stored. Only
  `/give … custom_data={isb:"s=1;b=…"}` selects another page.
- **The mana bar does not show a cooldown sweep**, and is a plain box.
- **Mage armour is stat-wise vanilla leather** apart from the mana bonus the
  native half grants; no attribute modifiers yet.
- No mob that drops scrolls, no structure loot, no advancements.

## Building

```sh
# content (light, works locally)
python tools/pack.py                 # regenerates content/ and writes target/irons_spellbooks.hmod

# native (on the build VPS; never a full game build on this PC)
#   1. scp this folder to /opt/build/mods/irons_spellbooks
#   2. bash vps_stage.sh                (stages api/ into /opt/build/wt/irons_spellbooks)
#   3. cargo test -p mod-irons_spellbooks
#      cargo check -p horizon-native-game
#      cargo build -p horizon-native-game --no-default-features   (dedicated server engine)
```

Checked against Horizon `a85af177a82504f357ff10d5d40d0b7230e2ac32` (the revision
`horizon-mixin.toml` pins).

### Build and test status

- `cargo horizon-mod check` on both files: **passes** (`.hmod`: mod, API 0.2, both;
  `.hnmod`: native-mod, pinned `a85af177a…/5a64af9…`, for 26.3).
- `cargo test -p mod-irons_spellbooks` against the pinned Horizon: **26 passed,
  0 failed** (the crate compiles against the real `horizon-client`, so every
  hook signature is checked).
- `cargo check -p horizon-native-game` (the generated game binary, client
  flavour, default features): **exit 0** (`Finished dev profile in 5m 43s`).
- Re-checked 2026-11-14 in one queued build on the build VPS against the pinned
  revision (`a85af177`), sources uploaded from this folder and staged by
  `tools/vps_stage.sh`: `cargo test -p mod-irons_spellbooks` **26 passed, 0
  failed** (`test_exit=0`), `cargo check -p horizon-native-game` **exit 0**,
  and the dedicated-server engine `cargo build -p horizon-native-game
  --no-default-features` **exit 0**. `cargo-horizon-mod horizon-mod check` on
  both files still passes (the `.hmod` unchanged at 99 files, the `.hnmod`
  repacked from the fixed sources).
- Still no in-game smoke test.
