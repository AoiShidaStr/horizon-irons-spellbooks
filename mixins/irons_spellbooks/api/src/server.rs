//! Casting, on the server: what a scroll, a spellbook or a staff does when it
//! is used, and the mana every player's tick brings back.
//!
//! A spell's effect is played through the trait mod API's helpers (a
//! projectile is summoned with its motion, a blink moves the player, heal and
//! fortify add vanilla effects, area spells hurt what stands in reach), so the
//! mod needs no game internals, no operator permission, and a dedicated server
//! and a LAN host behave alike.

use std::sync::atomic::{AtomicU32, Ordering};

use horizon_client::mod_api::{
    Compound, EquipmentSlot, Flow, Host, ItemStack, NativeEffect, NativeInventoryKind,
    NativeServerPlayer, ParticleOptions, ParticleType, Tag, Text, UseItemEvent,
};

use crate::aim;
use crate::book::{self, Book, Refused};
use crate::mana;
use crate::spells::{self, Kind, School, Spell};
use crate::state;

/// How often the client is told its mana, in ticks (four times a second is
/// enough for a bar).
const SYNC_EVERY: u32 = 5;

/// Vanilla's `SoundSource` ordinals: a spell's sound is one a player makes.
const PLAYER_SOUND: usize = 7;

static TICKS: AtomicU32 = AtomicU32::new(0);

/// Why a cast did not happen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Refusal {
    Resting(u32),
    Mana(f32),
    Book(Refused),
    Empty,
}

impl Refusal {
    pub fn line(self, spell: &Spell, _level: u8) -> String {
        match self {
            Refusal::Resting(ticks) => format!(
                "{} is resting for {} more",
                spell.name,
                spells::seconds(ticks)
            ),
            Refusal::Mana(need) => format!(
                "Not enough mana: {} costs {need:.0}",
                spell.name
            ),
            Refusal::Book(Refused::Full) => "That spellbook has no room for another page".into(),
            Refusal::Book(Refused::Known) => {
                format!("That spellbook already knows {} at this level", spell.name)
            }
            Refusal::Empty => "That spellbook has no pages yet".into(),
        }
    }
}

/// One player tick: their gear decides the maximum, mana comes back, and the
/// cooldowns run down.
pub fn tick(host: &mut dyn Host) {
    let Some(player) = host.acting_player() else {
        return;
    };
    let Ok(mut pool) = state::pool(host, player.uuid) else {
        return;
    };
    let bonus = armour_bonus(host);
    pool.set_bonus(bonus);
    pool.tick();
    let _ = state::save(host, player.uuid, &pool);

    if TICKS.fetch_add(1, Ordering::Relaxed) % SYNC_EVERY == 0 {
        let held = host
            .held_stack(player.uuid)
            .is_some_and(|stack| book::is_casting_item(&stack.item));
        let show = held || pool.mana < pool.max();
        let text = state::encode(pool.mana, pool.max(), show);
        let _ = host.net_send_to_player(player.uuid, state::CHANNEL_MANA, text.as_bytes());
    }
}

/// What each piece of mage armour adds, from what the player wears (the four
/// armour slots; `carried_items` is the inventory, which no armour slot is in).
fn armour_bonus(host: &mut dyn Host) -> f32 {
    const SLOTS: [EquipmentSlot; 4] = [
        EquipmentSlot::Head,
        EquipmentSlot::Chest,
        EquipmentSlot::Legs,
        EquipmentSlot::Feet,
    ];
    let mut pieces = 0;
    for slot in SLOTS {
        if let Some(stack) = host.equipment(slot) {
            if book::is_mage_armour(&stack.item) {
                pieces += 1;
            }
        }
    }
    pieces as f32 * mana::ARMOUR_BONUS
}

/// A right click with a casting item: a scroll casts once and is used up, a
/// spellbook casts the page it has selected, or turns the page while sneaking.
pub fn use_item(host: &mut dyn Host, event: &mut UseItemEvent<'_>) -> Flow {
    let item = &*event.stack.item;
    let Some(player) = host.acting_player() else {
        return Flow::Continue;
    };

    if book::is_mana_potion(item) {
        let Ok(mut pool) = state::pool(host, player.uuid) else {
            return Flow::Cancel;
        };
        if pool.mana + 0.5 < pool.max() {
            pool.add(mana::POTION_RESTORE);
            let _ = state::save(host, player.uuid, &pool);
            if player.game_mode != 1 {
                host.take_items(item, 1);
            }
            host.action_bar(
                player.uuid,
                Text::literal(format!(
                    "Mana restored: {} / {}",
                    pool.mana.round() as i32,
                    pool.max().round() as i32
                )),
            );
        } else {
            host.action_bar(player.uuid, Text::literal("Your mana is already full"));
        }
        return Flow::Cancel;
    }

    if let Some(spell) = book::scroll_spell(item) {
        let level = book::scroll_level(custom_data(host, event.stack).as_deref());
        let outcome = cast(host, &player, spell, level, true);
        if outcome.is_ok() && player.game_mode != 1 {
            host.take_items(item, 1);
        }
        report(host, &player, spell, level, outcome);
        return Flow::Cancel;
    }

    if let Some(capacity) = book::book_capacity(item) {
        let mut spellbook = Book::parse(custom_data(host, event.stack).as_deref(), capacity);
        // A scroll in the off hand is inscribed into the page the book opens
        // on: the scroll is used up and the held book written back.
        if let Some(off) = host.player_equipment(player.uuid, EquipmentSlot::OffHand) {
            if let Some(spell) = book::scroll_spell(&off.item) {
                let level = book::scroll_level(custom_data(host, &off).as_deref());
                match spellbook.inscribe(spell, level, capacity) {
                    Ok(()) => {
                        if write_held(host, &player, event.stack, &spellbook.encode()) {
                            if player.game_mode != 1 {
                                host.take_items(&off.item, 1);
                            }
                            host.action_bar(
                                player.uuid,
                                Text::literal(format!(
                                    "{} {} written into the spellbook",
                                    spell.name,
                                    spells::roman(level)
                                )),
                            );
                        } else {
                            host.action_bar(
                                player.uuid,
                                Text::literal("The spellbook could not be written"),
                            );
                        }
                    }
                    Err(refusal) => {
                        host.action_bar(
                            player.uuid,
                            Text::literal(Refusal::Book(refusal).line(spell, level)),
                        );
                    }
                }
                return Flow::Cancel;
            }
        }
        if host.sneaking() {
            match spellbook.cycle(true) {
                Some((spell, level)) => {
                    let saved = write_held(host, &player, event.stack, &spellbook.encode());
                    let line = if saved {
                        format!(
                            "Page {}: {} {}",
                            spellbook.selected + 1,
                            spell.name,
                            spells::roman(level)
                        )
                    } else {
                        format!("The spellbook could not be written, page stays as it was")
                    };
                    host.action_bar(player.uuid, Text::literal(line));
                }
                None => {
                    host.action_bar(player.uuid, Text::literal("That spellbook has no pages yet"));
                }
            }
            return Flow::Cancel;
        }
        let Some((spell, level)) = spellbook.current() else {
            report(host, &player, spells::by_id("firebolt").expect("table"), 1, Err(Refusal::Empty));
            return Flow::Cancel;
        };
        let outcome = cast(host, &player, spell, level, false);
        report(host, &player, spell, level, outcome);
        return Flow::Cancel;
    }

    Flow::Continue
}

/// The `isb` string a stack carries, if any.
fn custom_data(host: &mut dyn Host, stack: &ItemStack) -> Option<String> {
    let data = host.component(stack, "minecraft:custom_data").ok()??;
    let Tag::Compound(data) = data else {
        return None;
    };
    let Tag::String(text) = data.get(&"isb".into())? else {
        return None;
    };
    text.to_string().ok()
}

fn report(
    host: &mut dyn Host,
    player: &NativeServerPlayer,
    spell: &Spell,
    level: u8,
    outcome: Result<(), Refusal>,
) {
    let line = match outcome {
        Ok(()) => format!(
            "{} {} cast",
            spell.name,
            spells::roman(level)
        ),
        Err(refusal) => refusal.line(spell, level),
    };
    host.action_bar(player.uuid, Text::literal(line));
}

/// Spends the mana, plays the spell and starts its cooldown.
pub fn cast(
    host: &mut dyn Host,
    player: &NativeServerPlayer,
    spell: &Spell,
    level: u8,
    from_scroll: bool,
) -> Result<(), Refusal> {
    let level = spell.clamp_level(level);
    let Ok(mut pool) = state::pool(host, player.uuid) else {
        return Err(Refusal::Mana(spell.mana_cost(level)));
    };
    let resting = pool.cooldown(spell.id);
    if resting > 0 {
        return Err(Refusal::Resting(resting));
    }
    let cost = spell.mana_cost(level);
    if !pool.spend(cost) {
        return Err(Refusal::Mana(cost));
    }
    let power = if has_staff(host) { mana::STAFF_POWER } else { 1.0 };
    let played = play(host, player, spell, level, power);
    let _ = state::save(host, player.uuid, &pool);
    if !played {
        pool.add(cost);
        let _ = state::save(host, player.uuid, &pool);
        return Err(Refusal::Mana(cost));
    }
    cast_sound(host, player, spell.school);
    cast_anim(host, player);
    // A scroll is spent anyway; both start the spell's rest.
    let ticks = if from_scroll { spell.cooldown / 2 } else { spell.cooldown };
    pool.start_cooldown(spell.id, ticks);
    let _ = state::save(host, player.uuid, &pool);
    Ok(())
}

fn has_staff(host: &mut dyn Host) -> bool {
    host.carried_items()
        .iter()
        .any(|(item, count)| book::is_staff(item) && *count > 0)
}

/// Plays the spell's effect. False when the game refused the command it needed.
///
/// Every spell uses the table's strength: a direct spell hurts the creature in
/// the line of sight for it (it no longer borrows a vanilla projectile's
/// damage), lightning is a visual bolt over the table's damage, and what a hit
/// leaves behind is the school's own (fire sets it alight, ice slows it).
fn play(
    host: &mut dyn Host,
    player: &NativeServerPlayer,
    spell: &Spell,
    level: u8,
    power: f32,
) -> bool {
    let strength = spell.strength(level, power);
    let dir = aim::direction(player.yaw, player.pitch);
    match spell.kind {
        Kind::Heal => {
            let amplifier = (strength / 4.0) as u8;
            burst(host, eye(player), spell.school, 12);
            effect(
                host,
                player.uuid,
                "minecraft:instant_health",
                amplifier.saturating_sub(1).min(4),
                1,
            )
        }
        Kind::Fortify => {
            let amplifier = ((strength / 4.0).ceil() as u8).saturating_sub(1).min(3);
            let mut any = false;
            let players = host.server_players();
            for other in players {
                if distance(player.position, other.position) > f64::from(spell.range) {
                    continue;
                }
                any |= effect(host, other.uuid, "minecraft:absorption", amplifier, 1200);
                if any {
                    burst(host, eye(&other), spell.school, 8);
                }
            }
            any
        }
        Kind::Regeneration => {
            let amplifier = (level - 1).min(3);
            burst(host, eye(player), spell.school, 12);
            effect(host, player.uuid, "minecraft:regeneration", amplifier, 200)
        }
        Kind::LightningBolt => {
            let at = impact(host, player, spell.range);
            // The bolt is the show: the damage is the table's, not the
            // vanilla strike's 5.
            let _ = host.lightning_bolt(&player.dimension, at, true);
            let _ = blast(host, player, at, 2.0, strength, "minecraft:magic");
            for (id, _) in within(host, &player.dimension, at, 2.0) {
                if is_self(player, id) {
                    continue;
                }
                afflict(host, id, spell.school);
            }
            burst(host, at, spell.school, 24);
            true
        }
        Kind::ChainLightning => {
            let at = impact(host, player, spell.range);
            let _ = host.lightning_bolt(&player.dimension, at, true);
            let _ = blast(host, player, at, 4.0, strength, "minecraft:magic");
            for (id, _) in within(host, &player.dimension, at, 4.0) {
                if is_self(player, id) {
                    continue;
                }
                afflict(host, id, spell.school);
            }
            burst(host, at, spell.school, 30);
            true
        }
        Kind::FrostStep | Kind::Teleport => {
            let wanted = f64::from(strength);
            let from = eye(player);
            let hit = host
                .raycast(spell.range)
                .map(|hit| distance(from, [hit.x, hit.y, hit.z]));
            let reach = aim::blink_reach(wanted, hit);
            let target = [
                player.position[0] + dir[0] * reach,
                player.position[1] + dir[1] * reach,
                player.position[2] + dir[2] * reach,
            ];
            burst(host, from, spell.school, 16);
            let landed = host.teleport_player(
                player.uuid,
                &player.dimension,
                target,
                player.yaw,
                player.pitch,
            );
            if landed {
                burst(host, eye(player), spell.school, 16);
            }
            landed
        }
        Kind::Firebolt | Kind::Icicle | Kind::MagicMissile => {
            bolt(host, player, spell, dir, strength)
        }
        Kind::Fireball => {
            let at = impact(host, player, spell.range);
            let _ = blast(host, player, at, 3.0, strength, "minecraft:magic");
            for (id, _) in within(host, &player.dimension, at, 3.0) {
                if is_self(player, id) {
                    continue;
                }
                ignite(host, id, spell.school.ignite_ticks());
            }
            burst(host, at, spell.school, 30);
            true
        }
        Kind::FrostNova => {
            let targets = within(host, &player.dimension, player.position, f64::from(spell.range));
            let attacker = attacker(player);
            for (id, at) in targets {
                if is_self(player, id) {
                    continue;
                }
                let _ = host.hurt_entity(
                    &player.dimension,
                    id,
                    strength,
                    "minecraft:magic",
                    attacker,
                );
                afflict(host, id, spell.school);
                let away = [
                    at[0] - player.position[0],
                    0.25,
                    at[2] - player.position[2],
                ];
                let length = (away[0] * away[0] + away[2] * away[2]).sqrt().max(0.1);
                let push = [
                    away[0] / length * 1.1,
                    away[1],
                    away[2] / length * 1.1,
                ];
                let _ = host.push_mob(&player.dimension, id, push);
            }
            ring(host, player, spell, f64::from(spell.range));
            true
        }
        Kind::Gust => {
            let targets = in_front(host, player, dir, f64::from(spell.range) + 4.0, 3.0);
            for (id, at) in targets {
                if is_self(player, id) {
                    continue;
                }
                let away = [
                    at[0] - player.position[0],
                    0.0,
                    at[2] - player.position[2],
                ];
                let length = (away[0] * away[0] + away[2] * away[2]).sqrt().max(0.1);
                let push = [
                    dir[0] * 1.2 + away[0] / length * 0.8,
                    0.5,
                    dir[2] * 1.2 + away[2] / length * 0.8,
                ];
                let _ = host.push_mob(&player.dimension, id, push);
            }
            let from = aim::cast_from(player.position, dir, 1.0);
            let to = [
                from[0] + dir[0] * f64::from(spell.range),
                from[1] + dir[1] * f64::from(spell.range),
                from[2] + dir[2] * f64::from(spell.range),
            ];
            trail(host, from, to, spell.school);
            true
        }
        Kind::Siphon => {
            let targets = in_front(host, player, dir, f64::from(spell.range), 2.5);
            let attacker = attacker(player);
            let Some((id, at)) = targets
                .into_iter()
                .min_by(|a, b| {
                    let da = distance(player.position, a.1);
                    let db = distance(player.position, b.1);
                    da.total_cmp(&db)
                })
            else {
                return false;
            };
            if !host.hurt_entity(
                &player.dimension,
                id,
                strength,
                "minecraft:magic",
                attacker,
            ) {
                return false;
            }
            afflict(host, id, spell.school);
            trail(host, eye(player), [at[0], at[1] + 1.0, at[2]], spell.school);
            let amplifier = ((strength / 8.0) as u8).min(3);
            let _ = effect(
                host,
                player.uuid,
                "minecraft:instant_health",
                amplifier,
                1,
            );
            true
        }
    }
}

/// A direct-damage spell: the first creature in the line of sight takes the
/// spell's own damage — the table's number, not a vanilla projectile's — and
/// the shot is drawn as a trail of the school's particles. A shot that meets
/// nobody still costs its mana: it was cast.
fn bolt(
    host: &mut dyn Host,
    player: &NativeServerPlayer,
    spell: &Spell,
    dir: [f64; 3],
    damage: f32,
) -> bool {
    let from = eye(player);
    let reach = host
        .raycast(spell.range)
        .map(|hit| distance(from, [hit.x, hit.y, hit.z]).min(f64::from(spell.range)))
        .unwrap_or(f64::from(spell.range));
    let target = nearest_on_line(host, player, from, dir, reach);
    let at = match target {
        Some((_, at)) => [at[0], at[1] + 1.0, at[2]],
        None => [
            from[0] + dir[0] * reach,
            from[1] + dir[1] * reach,
            from[2] + dir[2] * reach,
        ],
    };
    trail(host, from, at, spell.school);
    let Some((id, _)) = target else {
        return true;
    };
    // Magic Missile is the one that ignores armour.
    let damage_type = if spell.id == "magic_missile" {
        "minecraft:magic"
    } else {
        "minecraft:generic"
    };
    let _ = host.hurt_entity(
        &player.dimension,
        id,
        damage,
        damage_type,
        attacker(player),
    );
    ignite(host, id, spell.school.ignite_ticks());
    afflict(host, id, spell.school);
    burst(host, at, spell.school, 12);
    true
}

/// The first creature whose body a shot from the eye meets on its way, as its
/// id and position: a creature within 1.2 blocks of the aim line, nearest
/// first.
fn nearest_on_line(
    host: &mut dyn Host,
    player: &NativeServerPlayer,
    from: [f64; 3],
    dir: [f64; 3],
    reach: f64,
) -> Option<(u32, [f64; 3])> {
    let mut best: Option<(u32, [f64; 3], f64)> = None;
    for (id, _, at) in host.level_entities(&player.dimension) {
        if is_self(player, id) {
            continue;
        }
        let centre = [at[0], at[1] + 1.0, at[2]];
        let d = [
            centre[0] - from[0],
            centre[1] - from[1],
            centre[2] - from[2],
        ];
        let along = d[0] * dir[0] + d[1] * dir[1] + d[2] * dir[2];
        if along <= 0.0 || along > reach {
            continue;
        }
        let side = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2] - along * along)
            .max(0.0)
            .sqrt();
        if side > 1.2 {
            continue;
        }
        let closer = match best {
            Some((_, _, nearest)) => along < nearest,
            None => true,
        };
        if closer {
            best = Some((id, at, along));
        }
    }
    best.map(|(id, at, _)| (id, at))
}

/// The player's eye, where a spell starts and is aimed from.
fn eye(player: &NativeServerPlayer) -> [f64; 3] {
    [
        player.position[0],
        player.position[1] + aim::EYE,
        player.position[2],
    ]
}

/// Where a ray of `range` blocks from the crosshair stops: the block it met,
/// or the point it reaches.
fn impact(host: &mut dyn Host, player: &NativeServerPlayer, range: f32) -> [f64; 3] {
    match host.raycast(range) {
        Some(hit) => [hit.x, hit.y, hit.z],
        None => {
            let dir = aim::direction(player.yaw, player.pitch);
            let from = eye(player);
            [
                from[0] + dir[0] * f64::from(range),
                from[1] + dir[1] * f64::from(range),
                from[2] + dir[2] * f64::from(range),
            ]
        }
    }
}

/// The particles the school's spells give off.
fn school_particle(school: School) -> ParticleType {
    match school {
        School::Fire => ParticleType::Flame,
        School::Ice | School::Holy => ParticleType::EndRod,
        School::Lightning => ParticleType::ElectricSpark,
        School::Ender => ParticleType::Portal,
        School::Evocation => ParticleType::Crit,
        School::Blood => ParticleType::Lava,
        School::Nature => ParticleType::SporeBlossomAir,
    }
}

/// A puff of the school's particles at `at`.
fn burst(host: &mut dyn Host, at: [f64; 3], school: School, count: i32) -> bool {
    host.send_particles(
        &ParticleOptions::simple(school_particle(school)),
        None,
        at,
        [0.45, 0.45, 0.45],
        0.08,
        count,
    )
}

/// The path of a shot, as puffs of the school's particles.
fn trail(host: &mut dyn Host, from: [f64; 3], to: [f64; 3], school: School) {
    let options = ParticleOptions::simple(school_particle(school));
    for step in 1..=8 {
        let t = f64::from(step) / 8.0;
        let at = [
            from[0] + (to[0] - from[0]) * t,
            from[1] + (to[1] - from[1]) * t,
            from[2] + (to[2] - from[2]) * t,
        ];
        let _ = host.send_particles(&options, None, at, [0.1, 0.1, 0.1], 0.0, 1);
    }
}

/// A ring of the school's particles on the ground around the caster, for the
/// spells that burst outwards.
fn ring(host: &mut dyn Host, player: &NativeServerPlayer, spell: &Spell, radius: f64) {
    let options = ParticleOptions::simple(school_particle(spell.school));
    for step in 0..16 {
        let angle = f64::from(step) * std::f64::consts::TAU / 16.0;
        let at = [
            player.position[0] + angle.cos() * radius,
            player.position[1] + 0.2,
            player.position[2] + angle.sin() * radius,
        ];
        let _ = host.send_particles(&options, None, at, [0.1, 0.2, 0.1], 0.0, 1);
    }
}

/// Leaves the school's effect on a mob (`minecraft:slowness` for ice, poison
/// for nature, weakness for blood, glowing for lightning).
fn afflict(host: &mut dyn Host, id: u32, school: School) -> bool {
    match school.affliction() {
        Some((effect, amplifier, ticks)) => effect_on_mob(host, id, effect, amplifier, ticks),
        None => false,
    }
}

/// Gives a vanilla effect to a mob, found by its entity id.
fn effect_on_mob(host: &mut dyn Host, id: u32, effect: &str, amplifier: u8, ticks: i32) -> bool {
    let Some(entity) = host.entity(id) else {
        return false;
    };
    let Some(uuid) = host.entity_uuid(entity) else {
        return false;
    };
    host.add_effect(
        uuid,
        &NativeEffect {
            id: effect.to_string(),
            amplifier,
            duration: ticks,
            ambient: false,
            particles: true,
        },
    )
}

/// Sets a mob alight for `ticks`; 0 does nothing.
fn ignite(host: &mut dyn Host, id: u32, ticks: i32) -> bool {
    if ticks <= 0 {
        return false;
    }
    let Some(entity) = host.entity(id) else {
        return false;
    };
    host.entity_set_fire(entity, ticks)
}

/// Whether an entity id is the caster's own.
fn is_self(player: &NativeServerPlayer, id: u32) -> bool {
    player.entity_id > 0 && id == player.entity_id as u32
}

fn attacker(player: &NativeServerPlayer) -> Option<u32> {
    (player.entity_id > 0).then_some(player.entity_id as u32)
}

/// Gives `id` to `uuid` as an effect; a duration of 1 tick is enough for the
/// instant effects (`instant_health`).
fn effect(host: &mut dyn Host, uuid: u128, id: &str, amplifier: u8, duration: i32) -> bool {
    host.add_effect(
        uuid,
        &NativeEffect {
            id: id.to_string(),
            amplifier,
            duration,
            ambient: false,
            particles: true,
        },
    )
}

/// The entities of the caster's level within `radius` blocks of `at`.
fn within(
    host: &mut dyn Host,
    dimension: &str,
    at: [f64; 3],
    radius: f64,
) -> Vec<(u32, [f64; 3])> {
    host.level_entities(dimension)
        .into_iter()
        .filter(|(_, _, p)| distance(at, *p) <= radius)
        .map(|(id, _, p)| (id, p))
        .collect()
}

/// Damages everything within `radius` of `at`, the caster excepted.
fn blast(
    host: &mut dyn Host,
    player: &NativeServerPlayer,
    at: [f64; 3],
    radius: f64,
    damage: f32,
    damage_type: &str,
) -> bool {
    let attacker = attacker(player);
    let mut any = false;
    for (id, _) in within(host, &player.dimension, at, radius) {
        if is_self(player, id) {
            continue;
        }
        any |= host.hurt_entity(&player.dimension, id, damage, damage_type, attacker);
    }
    any
}

/// The entities in a cone `reach` blocks long and `half_width` wide along
/// `dir`, from where the caster stands.
fn in_front(
    host: &mut dyn Host,
    player: &NativeServerPlayer,
    dir: [f64; 3],
    reach: f64,
    half_width: f64,
) -> Vec<(u32, [f64; 3])> {
    host.level_entities(&player.dimension)
        .into_iter()
        .filter(|(_, _, p)| {
            let d = [
                p[0] - player.position[0],
                p[1] - player.position[1],
                p[2] - player.position[2],
            ];
            let along = d[0] * dir[0] + d[1] * dir[1] + d[2] * dir[2];
            if along <= 0.0 || along > reach {
                return false;
            }
            let side = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2] - along * along).max(0.0).sqrt();
            side <= half_width
        })
        .map(|(id, _, p)| (id, p))
        .collect()
}

/// Writes a spellbook's new `isb` string over the held stack, checking that
/// the slot still holds the stack the use hook was given.
fn write_held(
    host: &mut dyn Host,
    player: &NativeServerPlayer,
    expected: &ItemStack,
    isb: &str,
) -> bool {
    let mut data = match host.component(expected, "minecraft:custom_data") {
        Ok(Some(Tag::Compound(data))) => data.clone(),
        _ => Compound::new(),
    };
    data.insert("isb".into(), Tag::String(isb.into()));
    let mut stack = expected.clone();
    if host
        .set_component(&mut stack, "minecraft:custom_data", Tag::Compound(data))
        .is_err()
    {
        return false;
    }
    host.set_inventory_slot(
        player.uuid,
        NativeInventoryKind::Player,
        usize::from(player.selected_slot),
        Some(expected),
        Some(stack),
    )
    .is_ok()
}

/// The school's sound at the caster, heard by everyone near them.
fn cast_sound(host: &mut dyn Host, player: &NativeServerPlayer, school: School) {
    let _ = host.play_sound(
        school.sound(),
        [
            player.position[0],
            player.position[1] + 1.0,
            player.position[2],
        ],
        PLAYER_SOUND,
        0.8,
        1.0,
    );
}

/// The caster's arm swings, as if the spell were thrown.
fn cast_anim(host: &mut dyn Host, player: &NativeServerPlayer) {
    if player.entity_id <= 0 {
        return;
    }
    let Some(entity) = host.entity(player.entity_id as u32) else {
        return;
    };
    let _ = host.entity_swing(entity, false);
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusals_read_plainly() {
        let bolt = spells::by_id("firebolt").unwrap();
        assert_eq!(
            Refusal::Resting(40).line(bolt, 1),
            "Firebolt is resting for 2s more"
        );
        assert_eq!(
            Refusal::Mana(10.0).line(bolt, 1),
            "Not enough mana: Firebolt costs 10"
        );
        assert_eq!(
            Refusal::Empty.line(bolt, 1),
            "That spellbook has no pages yet"
        );
    }

    #[test]
    fn a_pool_learns_the_right_power() {
        // The pool's arithmetic is tested in mana.rs; here only that casting
        // asks the table for its numbers.
        let bolt = spells::by_id("firebolt").unwrap();
        assert_eq!(bolt.mana_cost(3), 14.0);
        assert_eq!(bolt.strength(3, mana::STAFF_POWER), 9.2);
    }
}
