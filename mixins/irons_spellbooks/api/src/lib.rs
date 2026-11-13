//! Iron's Spellbooks for Horizon: the native (trait mod) half.
//!
//! A clean-room reimplementation of the *behaviour* of Iron's Spellbooks
//! (a mana pool, scrolls, spellbooks and a core set of spells): the original
//! ships All-Rights-Reserved, so none of its code, data or art is used here.
//! The spells, their numbers, their wording and the art are this mod's own,
//! written from the public description of how such a mod plays.
//!
//! The launcher links this crate into the game and calls `hooks()`
//! (docs/native-mods.md, "Trait mods"). The same `Hooks` serve a dedicated
//! server (the casting, mana and network hooks) and a client (the tooltip and
//! the mana bar).
//!
//! Hook points used: `player_tick`, `use_item`, `tooltip`, `hud`,
//! `net_from_server`.
#![forbid(unsafe_code)]

pub mod aim;
pub mod book;
pub mod mana;
pub mod spells;

mod client;
mod server;
mod state;

use horizon_client::mod_api::{
    Flow, Hooks, Host, HudEvent, NetFromServerEvent, PlayerTickEvent, TooltipEvent, UseItemEvent,
};

pub use spells::MOD_ID;

/// The launcher calls this for the built game.
pub fn hooks() -> Box<dyn Hooks> {
    Box::new(Spellbooks)
}

/// One instance serves the window's game and the server thread alike, so it
/// keeps nothing of its own: the pools live in attachments, the bar's copy in
/// a static.
struct Spellbooks;

impl Hooks for Spellbooks {
    /// Mana comes back and cooldowns run down, once a tick for every living
    /// player on a server.
    fn player_tick(&self, host: &mut dyn Host, _event: &mut PlayerTickEvent) {
        server::tick(host);
    }

    /// A right click with a scroll, a spellbook or (nothing) else.
    fn use_item(&self, host: &mut dyn Host, event: &mut UseItemEvent<'_>) -> Flow {
        server::use_item(host, event)
    }

    /// What a casting item is worth, wherever the game shows a tooltip.
    fn tooltip(&self, host: &dyn Host, event: &mut TooltipEvent<'_>) {
        client::tooltip(host, event);
    }

    /// The mana bar the server's message fills.
    fn hud(&self, host: &mut dyn Host, event: &mut HudEvent<'_>) {
        client::hud(host, event);
    }

    fn net_from_server(&self, _host: &mut dyn Host, event: &mut NetFromServerEvent<'_>) {
        client::net(event);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_namespace_is_the_mod_id() {
        assert_eq!(super::MOD_ID, "irons_spellbooks");
    }
}
