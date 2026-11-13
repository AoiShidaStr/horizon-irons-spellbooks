//! What the game keeps for the mod: a player's mana pool, saved with them, and
//! the copy the client shows.
//!
//! The pool is a server attachment, so it follows a player across restarts and
//! respawns ([`AttachmentType`] has a codec). The client is told the numbers it
//! draws as a plain mod message, so the bar needs no game-side lookup of the
//! local player.

use std::sync::{LazyLock, Mutex};

use horizon_client::mod_api::{
    AttachmentCodec, AttachmentError, AttachmentHost, AttachmentType, Compound, Holder, Host, Tag,
};

use crate::mana::{BASE_MAX, Pool};
use crate::spells;

/// The channel the server sends a player's mana on.
pub const CHANNEL_MANA: &str = "irons_spellbooks:mana";

/// A player's pool, saved, copied on death, and told to their client.
pub static POOL: AttachmentType<Pool> = AttachmentType {
    namespace: "irons_spellbooks",
    name: "pool",
    default: Pool::default,
    codec: Some(AttachmentCodec {
        encode: |pool: &Pool| {
            let mut data = Compound::new();
            data.insert("mana".into(), Tag::Float(pool.mana.to_bits()));
            data.insert("bonus".into(), Tag::Float(pool.bonus.to_bits()));
            let cooldowns = pool
                .cooldowns()
                .iter()
                .map(|(spell, ticks)| Tag::String(format!("{spell}:{ticks}").as_str().into()))
                .collect();
            data.insert("cooldowns".into(), Tag::List(cooldowns));
            Ok(data)
        },
        decode: |data: &Compound| {
            let mana = match data.get(&"mana".into()) {
                Some(Tag::Float(bits)) => f32::from_bits(*bits),
                _ => BASE_MAX,
            };
            let bonus = match data.get(&"bonus".into()) {
                Some(Tag::Float(bits)) => f32::from_bits(*bits),
                _ => 0.0,
            };
            let mut cooldowns = Vec::new();
            if let Some(Tag::List(entries)) = data.get(&"cooldowns".into()) {
                for entry in entries {
                    let Tag::String(text) = entry else {
                        continue;
                    };
                    let Ok(text) = text.to_string() else {
                        continue;
                    };
                    let Some((spell, ticks)) = text.split_once(':') else {
                        continue;
                    };
                    let (Some(spell), Ok(ticks)) = (spells::by_id(spell), ticks.parse::<u32>()) else {
                        continue;
                    };
                    cooldowns.push((spell.id, ticks));
                }
            }
            Ok(Pool::restore(mana, bonus, cooldowns))
        },
    }),
    copy_on_death: true,
    sync: false,
};

/// The player's pool, made at its default the first time it is asked for.
pub fn pool(host: &mut dyn Host, player: u128) -> Result<Pool, AttachmentError> {
    POOL.get(host, Holder::Player(player))
}

pub fn save(host: &mut dyn Host, player: u128, pool: &Pool) -> Result<(), AttachmentError> {
    POOL.set(host, Holder::Player(player), pool.clone())
}

/// What the client cached: the mana and maximum last told, or `None` when the
/// bar is hidden.
static SHOWN: LazyLock<Mutex<Option<(f32, f32)>>> = LazyLock::new(|| Mutex::new(None));

pub fn shown() -> Option<(f32, f32)> {
    *SHOWN.lock().expect("mana cache lock")
}

/// The bar's text the server sends: `m=<mana>;x=<max>`, or an empty payload
/// when the bar should go away.
pub fn encode(mana: f32, max: f32, show: bool) -> String {
    if show {
        format!("m={mana:.1};x={max:.1}")
    } else {
        String::new()
    }
}

/// Reads that text back; anything else hides the bar.
pub fn decode(data: &[u8]) -> Option<(f32, f32)> {
    let text = std::str::from_utf8(data).ok()?;
    let mut mana = None;
    let mut max = None;
    for part in text.split(';') {
        let Some((name, value)) = part.split_once('=') else {
            continue;
        };
        match (name, value.parse::<f32>()) {
            ("m", Ok(value)) => mana = Some(value),
            ("x", Ok(value)) => max = Some(value),
            _ => {}
        }
    }
    match (mana, max) {
        (Some(mana), Some(max)) if mana.is_finite() && max.is_finite() && max > 0.0 => {
            Some((mana.clamp(0.0, max), max))
        }
        _ => None,
    }
}

pub fn remember(shown: Option<(f32, f32)>) {
    *SHOWN.lock().expect("mana cache lock") = shown;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_text_round_trips_and_hides() {
        assert_eq!(encode(37.2, 125.0, true), "m=37.2;x=125.0");
        assert_eq!(decode(b"m=37.2;x=125.0"), Some((37.2, 125.0)));
        assert_eq!(encode(0.0, 100.0, false), "");
        assert_eq!(decode(b""), None);
        assert_eq!(decode(b"m=nan;x=100"), None);
        assert_eq!(decode(b"m=5;x=0"), None);
        assert_eq!(decode(b"garbage"), None);
        assert_eq!(decode(b"m=150;x=100"), Some((100.0, 100.0)));
    }
}
