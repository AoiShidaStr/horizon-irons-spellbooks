//! What a client shows: the lines a casting item's tooltip carries, and the
//! mana bar the server's message fills.

use horizon_client::mod_api::{Corner, Host, HudEvent, NetFromServerEvent, Tag, Text, TooltipEvent};

use crate::book;
use crate::spells::{self, Spell};
use crate::state;

/// The server's mana message: the bar appears, moves or goes away.
pub fn net(event: &mut NetFromServerEvent<'_>) {
    if event.channel == state::CHANNEL_MANA {
        state::remember(state::decode(event.data));
    }
}

/// The `isb` string a stack carries, if any.
fn custom_data<'a>(host: &'a dyn Host, stack: &'a horizon_client::mod_api::ItemStack) -> Option<String> {
    let data = host.component(stack, "minecraft:custom_data").ok()??;
    let Tag::Compound(data) = data else {
        return None;
    };
    let Tag::String(text) = data.get(&"isb".into())? else {
        return None;
    };
    text.to_string().ok()
}

pub fn tooltip(host: &dyn Host, event: &mut TooltipEvent<'_>) {
    let item = &*event.stack.item;

    if let Some(spell) = book::scroll_spell(item) {
        let level = book::scroll_level(custom_data(host, event.stack).as_deref());
        push_spell(event, spell, level);
        event
            .lines
            .push((Text::literal(spell.summary).with_color(0x9E_9E_9E), 0xFF9E_9E_9E));
        event.lines.push((
            Text::literal("Used up when cast").with_color(0x7A_7A_7A),
            0xFF7A_7A_7A,
        ));
        return;
    }

    if let Some(capacity) = book::book_capacity(item) {
        let book = book::Book::parse(custom_data(host, event.stack).as_deref(), capacity);
        event.lines.push((
            Text::literal(format!("Pages: {} of {capacity}", book.spells.len())).with_color(0xC8_A0_FF),
            0xFFC8_A0_FF,
        ));
        if let Some((spell, level)) = book.current() {
            push_spell(event, spell, level);
            event.lines.push((
                Text::literal("Use: cast this page").with_color(0x9E_9E_9E),
                0xFF9E_9E_9E,
            ));
        } else {
            event.lines.push((
                Text::literal("No pages yet").with_color(0x9E_9E_9E),
                0xFF9E_9E_9E,
            ));
        }
        event.lines.push((
            Text::literal("Sneak + use: next page").with_color(0x7A_7A_7A),
            0xFF7A_7A_7A,
        ));
        event.lines.push((
            Text::literal("Use with a scroll in the off hand: inscribe it")
                .with_color(0x7A_7A_7A),
            0xFF7A_7A_7A,
        ));
        return;
    }

    if book::is_mana_potion(item) {
        event.lines.push((
            Text::literal(format!(
                "Restores {:.0} mana",
                crate::mana::POTION_RESTORE
            ))
            .with_color(0x4C_6B_FF),
            0xFF4C_6B_FF,
        ));
        return;
    }

    if book::is_staff(item) {
        event.lines.push((
            Text::literal(format!(
                "+{:.0}% spell power while carried",
                (crate::mana::STAFF_POWER - 1.0) * 100.0
            ))
            .with_color(0xC8_A0_FF),
            0xFFC8_A0_FF,
        ));
        return;
    }

    if book::is_mage_armour(item) {
        event.lines.push((
            Text::literal(format!(
                "+{:.0} max mana while worn",
                crate::mana::ARMOUR_BONUS
            ))
            .with_color(0x8F_E3_F5),
            0xFF8F_E3_F5,
        ));
    }
}

fn push_spell(event: &mut TooltipEvent<'_>, spell: &Spell, level: u8) {
    let colour = spell.school.rgb();
    event.lines.push((
        Text::literal(format!(
            "{} spell • level {}",
            spell.school.name(),
            spells::roman(level)
        ))
        .with_color(colour),
        0xFF00_0000 | colour,
    ));
    event.lines.push((
        Text::literal(format!(
            "Mana {:.0} • rest {}",
            spell.mana_cost(level),
            spells::seconds(spell.cooldown)
        ))
        .with_color(0x9E_9E_9E),
        0xFF9E_9E_9E,
    ));
}

/// The mana bar: a dark box, the mana it holds in the school's blue, and the
/// numbers on top.
pub fn hud(host: &mut dyn Host, event: &mut HudEvent<'_>) {
    let Some((mana, max)) = state::shown() else {
        return;
    };
    let (width, height) = (82.0, 9.0);
    let (x, y) = event.corners.place(Corner::BottomRight, width, height);
    host.fill(event.batch, [x, y, width, height], 0xC0_10_10_18);
    let filled = (mana / max).clamp(0.0, 1.0) * (width - 2.0);
    if filled > 0.0 {
        host.fill(event.batch, [x + 1.0, y + 1.0, filled, height - 2.0], 0xFF_4C_6B_FF);
    }
    let label = format!("{} / {}", mana.round() as i32, max.round() as i32);
    host.text(event.batch, &label, [x + 4.0, y + 1.0], 0xFF_FF_FF, true);
}
