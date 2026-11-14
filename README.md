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
- `spells.rs`: 15 spells over 8 schools (Fire, Ice, Lightning, Holy, Ender,
  Evocation, Blood, Nature) with level cap, mana cost, cooldown, strength,
  range and summary, plus Roman numerals and second formatting.
- `aim.rs`: yaw/pitch direction, cast origin at the eye, blink distance clamped
  by the crosshair's block hit, `Motion` and coordinate formatting.
- `book.rs`: scrolls (`irons_spellbooks:scroll_<spell>`) and spellbooks
  (5/8/10 pages) read from `minecraft:custom_data.isb` (`l=3`, `s=0;b=heal:2`),
  inscribing rules, page cycling and tearing a page out.
- Hooks: `player_tick` (regen, armour bonus, cooldowns, mana message),
  `use_item` (scroll cast + consume, spellbook cast and page turn, off-hand
  scroll inscription, mana potion), `tooltip` (school, level, mana, rest,
  effect and how to use), `hud` (mana bar in the bottom-right corner),
  `net_from_server` (the bar's value).
- Casting runs on the server through the trait API, so no `/effect` permission
  is involved: `Host::add_effect` for Heal, Fortify, Blessing and the siphon's
  self-heal, `Host::summon` for Firebolt's fireball and the Icicle, Thorn Dart
  and Magic Missile arrows, `Host::lightning_bolt` for Lightning Bolt and Chain
  Lightning, `Host::teleport_player` for the blinks, `Host::entities_in` /
  `Host::level_entities` with `Host::hurt_entity` for the area blasts (Fireball
  included), and `Host::push_mob` for Frost Nova and Gust.
- Page turns and inscriptions write the held stack back (`Host::set_component`
  + `Host::set_inventory_slot`), so the selected page survives the click and a
  scroll held in the off hand is consumed into the book. A mana potion restores
  50 mana when used. A quiet vanilla sound plays at the caster on a cast. No
  item is consumed in creative.

**Content**

- 15 scrolls (paper, stack of 16, uncommon), 3 spellbooks (1 page stack,
  uncommon/rare/epic), four mage armour sets of four pieces (Wizard,
  Pyromancer, Cryomancer, Electromancer; vanilla leather stats plus the native
  mana bonus), the arcane staff (+15 % spell power while carried), arcane
  essence and the mana potion.
- Crafting recipes for all of them, a creative tab of 37 items, English and
  French names, item models and original 16×16 textures.

## Missing

- **The direct-damage spells still use vanilla projectile damage** (Firebolt,
  Icicle, Thorn Dart, Magic Missile, Lightning Bolt): only the area spells
  (Fireball, Frost Nova, Chain Lightning) and Blood Siphon deal the table's
  numbers.
- **No status effects on mobs**: Frost Nova pushes and wounds instead of
  slowing, and nothing applies poison, fire or weakness to a victim.
- **No particle work, school-specific sounds or casting animation**: one
  vanilla sound plays, and the HUD bar is a plain box without a cooldown sweep.
- **Spell effects are still approximations**: Firebolt, Icicle, Thorn Dart and
  Magic Missile are vanilla projectiles (no custom projectile entity), and
  Magic Missile reaches its target only because its arrow ignores gravity.
- **No mob that drops scrolls, no structure loot, no advancements.**
- **Mage armour has no attribute modifiers or enchantment behaviour** beyond
  the leather template and the native maximum-mana bonus.
- **A spellbook's page can be torn out in `book.rs` but no in-game interaction
  exposes it.**
- Still no in-game smoke test.

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
- `cargo test -p mod-irons-spellbooks` against the pinned Horizon: **26 passed,
  0 failed** (the crate compiles against the real `horizon-client`, so every
  hook signature is checked).
- `cargo check -p horizon-native-game` (the generated game binary, client
  flavour, default features): **exit 0** (`Finished dev profile in 5m 43s`).
- Re-checked 2026-11-14 in one queued build on the build VPS against the pinned
  revision (`a85af177`), sources uploaded from this folder and staged by
  `tools/vps_stage.sh`: `cargo test -p mod-irons-spellbooks` **26 passed, 0
  failed** (`test_exit=0`), `cargo check -p horizon-native-game` **exit 0**,
  and the dedicated-server engine `cargo build -p horizon-native-game
  --no-default-features` **exit 0**. `cargo-horizon-mod horizon-mod check` on
  both files still passes (the `.hmod` unchanged at 99 files, the `.hnmod`
  repacked from the fixed sources).
- Still no in-game smoke test.
