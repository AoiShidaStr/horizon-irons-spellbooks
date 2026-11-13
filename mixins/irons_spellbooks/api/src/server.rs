//! Casting, on the server: what a scroll, a spellbook or a staff does when it
//! is used, and the mana every player's tick brings back.
//!
//! A spell's effect is played through the game's own commands and helpers
//! (a projectile is summoned with its motion, a blink moves the player, heal
//! and fortify give vanilla effects), so the mod needs no game internals and a
//! dedicated server and a LAN host behave alike.

use std::sync::atomic::{AtomicU32, Ordering};

use horizon_client::mod_api::{
    Flow, Host, ItemStack, NativeServerPlayer, Tag, Text, UseItemEvent,
};

use crate::aim;
use crate::book::{self, Book, Refused};
use crate::mana;
use crate::spells::{self, Kind, Spell};
use crate::state;

/// How often the client is told its mana, in ticks (four times a second is
/// enough for a bar).
const SYNC_EVERY: u32 = 5;

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

/// What each piece of mage armour adds, from what the player carries.
fn armour_bonus(host: &mut dyn Host) -> f32 {
    let pieces = host
        .carried_items()
        .iter()
        .filter(|(item, count)| book::is_mage_armour(item) && *count > 0)
        .count()
        .min(4);
    pieces as f32 * mana::ARMOUR_BONUS
}

/// A right click with a casting item: a scroll casts once and is used up, a
/// spellbook casts the page it has selected, or turns the page while sneaking.
pub fn use_item(host: &mut dyn Host, event: &mut UseItemEvent<'_>) -> Flow {
    let item = &*event.stack.item;
    let Some(player) = host.acting_player() else {
        return Flow::Continue;
    };

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
        if host.sneaking() {
            match spellbook.cycle(true) {
                Some((spell, level)) => {
                    host.action_bar(
                        player.uuid,
                        Text::literal(format!(
                            "Page {}: {} {}",
                            spellbook.selected + 1,
                            spell.name,
                            spells::roman(level)
                        )),
                    );
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
            let amplifier = ((strength / 4.0).round() as i32).clamp(0, 4);
            command(
                host,
                &format!(
                    "effect give {} minecraft:instant_health 1 {amplifier} true",
                    player.name
                ),
            )
        }
        Kind::Fortify => {
            let amplifier = ((strength / 4.0).ceil() as i32 - 1).clamp(0, 3);
            let mut any = false;
            let players = host.server_players();
            for other in players {
                if distance(player.position, other.position) > f64::from(spell.range) {
                    continue;
                }
                any |= command(
                    host,
                    &format!(
                        "effect give {} minecraft:absorption 60 {amplifier} true",
                        other.name
                    ),
                );
            }
            any
        }
        Kind::LightningBolt => {
            let hit = host.raycast(spell.range);
            let Some(hit) = hit else {
                return false;
            };
            command(
                host,
                &format!(
                    "summon minecraft:lightning_bolt {:.2} {:.2} {:.2}",
                    hit.x, hit.y, hit.z
                ),
            )
        }
        Kind::FrostStep | Kind::Teleport => {
            let wanted = f64::from(strength);
            let eye = aim::cast_from(player.position, [0.0, 0.0, 0.0], 0.0);
            let hit = host
                .raycast(spell.range)
                .map(|hit| distance(eye, [hit.x, hit.y, hit.z]));
            let reach = aim::blink_reach(wanted, hit);
            let target = [
                player.position[0] + dir[0] * reach,
                player.position[1] + dir[1] * reach,
                player.position[2] + dir[2] * reach,
            ];
            host.teleport_player(
                player.uuid,
                &player.dimension,
                target,
                player.yaw,
                player.pitch,
            )
        }
        Kind::Firebolt => projectile(host, player, dir, "minecraft:small_fireball", 1.4, ""),
        Kind::Fireball => projectile(host, player, dir, "minecraft:small_fireball", 0.8, ""),
        Kind::Icicle => projectile(
            host,
            player,
            dir,
            "minecraft:arrow",
            2.6,
            &format!(",damage:{:.2}d,crit:1b", strength),
        ),
        Kind::MagicMissile => projectile(
            host,
            player,
            dir,
            "minecraft:arrow",
            3.2,
            &format!(",damage:{:.2}d,NoGravity:1b", strength),
        ),
        Kind::Gust => projectile(host, player, dir, "minecraft:breeze_wind_charge", 1.2, ""),
    }
}

fn projectile(
    host: &mut dyn Host,
    player: &NativeServerPlayer,
    dir: [f64; 3],
    kind: &str,
    speed: f64,
    extra: &str,
) -> bool {
    let from = aim::cast_from(player.position, dir, 1.0);
    let motion = aim::motion(dir, speed);
    command(
        host,
        &format!(
            "summon {kind} {} {{Motion:{}{extra}}}",
            aim::point(from),
            aim::motion_list(motion)
        ),
    )
}

fn command(host: &mut dyn Host, line: &str) -> bool {
    host.run_command(line)
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
