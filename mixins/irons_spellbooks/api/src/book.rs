//! What the mod's items carry: a scroll's level and a spellbook's pages.
//!
//! Both are one short string kept under `isb` in the stack's
//! `minecraft:custom_data` (a vanilla component, so the stack survives being
//! saved, traded, dropped and sent to clients with no component of the mod's
//! own): `l=3` on a scroll, `s=1;b=firebolt:2,heal:1` on a spellbook. This
//! module only reads and writes that string.

use crate::spells::{self, MOD_ID, Spell};

/// The key under `minecraft:custom_data`.
pub const DATA_KEY: &str = "isb";

/// The spell a scroll item casts (`irons_spellbooks:scroll_firebolt`).
pub fn scroll_spell(item: &str) -> Option<&'static Spell> {
    let path = item.strip_prefix(MOD_ID)?.strip_prefix(':')?;
    spells::by_id(path.strip_prefix("scroll_")?)
}

/// How many spells a spellbook item holds, None for another item.
pub fn book_capacity(item: &str) -> Option<usize> {
    match item.strip_prefix(MOD_ID)?.strip_prefix(':')? {
        "apprentice_spellbook" => Some(5),
        "journeyman_spellbook" => Some(8),
        "archmage_spellbook" => Some(10),
        _ => None,
    }
}

/// Whether the item is one of the wandering mage's four armour pieces.
pub fn is_mage_armour(item: &str) -> bool {
    matches!(
        item.strip_prefix(MOD_ID).and_then(|rest| rest.strip_prefix(':')),
        Some("wizard_hat" | "wizard_robe" | "wizard_leggings" | "wizard_boots")
    )
}

pub fn is_staff(item: &str) -> bool {
    item.strip_prefix(MOD_ID) == Some(":arcane_staff")
}

/// Whether holding the item makes the mana bar show.
pub fn is_casting_item(item: &str) -> bool {
    scroll_spell(item).is_some() || book_capacity(item).is_some() || is_staff(item)
}

fn field<'a>(data: &'a str, key: &str) -> Option<&'a str> {
    data.split(';').find_map(|part| {
        let (name, value) = part.split_once('=')?;
        (name == key).then_some(value)
    })
}

/// A scroll's level: 1 with no data or a level that does not read.
pub fn scroll_level(data: Option<&str>) -> u8 {
    data.and_then(|data| field(data, "l"))
        .and_then(|level| level.parse::<u8>().ok())
        .filter(|level| *level >= 1)
        .unwrap_or(1)
}

pub fn scroll_data(level: u8) -> String {
    format!("l={}", level.max(1))
}

/// Why a scroll did not go into a book.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    Full,
    /// The book already holds the spell at this level or a higher one.
    Known,
}

/// A spellbook's pages.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Book {
    pub selected: usize,
    pub spells: Vec<(&'static Spell, u8)>,
}

impl Book {
    /// Reads a book's data; pages of unknown spells are left out, levels are
    /// brought into the spell's range and `capacity` cuts what is past it.
    pub fn parse(data: Option<&str>, capacity: usize) -> Self {
        let mut book = Self::default();
        let Some(data) = data else {
            return book;
        };
        for page in field(data, "b").unwrap_or("").split(',') {
            let Some((id, level)) = page.split_once(':') else {
                continue;
            };
            let (Some(spell), Ok(level)) = (spells::by_id(id), level.parse::<u8>()) else {
                continue;
            };
            if book.spells.len() < capacity && !book.spells.iter().any(|(s, _)| s.id == spell.id) {
                book.spells.push((spell, spell.clamp_level(level)));
            }
        }
        book.selected = field(data, "s")
            .and_then(|selected| selected.parse::<usize>().ok())
            .unwrap_or(0);
        if book.selected >= book.spells.len() {
            book.selected = 0;
        }
        book
    }

    pub fn encode(&self) -> String {
        let pages: Vec<String> = self
            .spells
            .iter()
            .map(|(spell, level)| format!("{}:{level}", spell.id))
            .collect();
        format!("s={};b={}", self.selected, pages.join(","))
    }

    pub fn current(&self) -> Option<(&'static Spell, u8)> {
        self.spells.get(self.selected).copied()
    }

    /// Selects the next page (`forward`) or the one before, around the ends.
    pub fn cycle(&mut self, forward: bool) -> Option<(&'static Spell, u8)> {
        let pages = self.spells.len();
        if pages == 0 {
            return None;
        }
        self.selected = if forward {
            (self.selected + 1) % pages
        } else {
            (self.selected + pages - 1) % pages
        };
        self.current()
    }

    /// Writes a scroll into the book: a new page, or a higher level over the
    /// page the book has. The new page becomes the selected one.
    pub fn inscribe(&mut self, spell: &'static Spell, level: u8, capacity: usize) -> Result<(), Refused> {
        let level = spell.clamp_level(level);
        if let Some(index) = self.spells.iter().position(|(s, _)| s.id == spell.id) {
            if self.spells[index].1 >= level {
                return Err(Refused::Known);
            }
            self.spells[index].1 = level;
            self.selected = index;
            return Ok(());
        }
        if self.spells.len() >= capacity {
            return Err(Refused::Full);
        }
        self.spells.push((spell, level));
        self.selected = self.spells.len() - 1;
        Ok(())
    }

    /// Tears the selected page out.
    pub fn remove_selected(&mut self) -> Option<(&'static Spell, u8)> {
        if self.selected >= self.spells.len() {
            return None;
        }
        let page = self.spells.remove(self.selected);
        if self.selected >= self.spells.len() {
            self.selected = self.spells.len().saturating_sub(1);
        }
        Some(page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spell(id: &str) -> &'static Spell {
        spells::by_id(id).unwrap()
    }

    #[test]
    fn items_are_told_apart() {
        assert_eq!(scroll_spell("irons_spellbooks:scroll_heal").map(|s| s.id), Some("heal"));
        assert_eq!(scroll_spell("irons_spellbooks:scroll_nothing"), None);
        assert_eq!(scroll_spell("minecraft:scroll_heal"), None);
        assert_eq!(scroll_spell("irons_spellbooksXscroll_heal"), None);
        assert_eq!(book_capacity("irons_spellbooks:apprentice_spellbook"), Some(5));
        assert_eq!(book_capacity("irons_spellbooks:archmage_spellbook"), Some(10));
        assert_eq!(book_capacity("minecraft:book"), None);
        assert!(is_mage_armour("irons_spellbooks:wizard_hat"));
        assert!(!is_mage_armour("minecraft:leather_helmet"));
        assert!(is_staff("irons_spellbooks:arcane_staff"));
        assert!(is_casting_item("irons_spellbooks:scroll_gust"));
        assert!(!is_casting_item("irons_spellbooks:arcane_essence"));
        for spell in &spells::SPELLS {
            assert_eq!(scroll_spell(&spell.scroll_item()), Some(spell));
        }
    }

    #[test]
    fn a_scroll_without_data_is_level_one() {
        assert_eq!(scroll_level(None), 1);
        assert_eq!(scroll_level(Some("")), 1);
        assert_eq!(scroll_level(Some("l=0")), 1);
        assert_eq!(scroll_level(Some("l=abc")), 1);
        assert_eq!(scroll_level(Some("l=4")), 4);
        assert_eq!(scroll_level(Some(&scroll_data(3))), 3);
        assert_eq!(scroll_data(0), "l=1");
    }

    #[test]
    fn a_book_round_trips() {
        let mut book = Book::default();
        book.inscribe(spell("firebolt"), 2, 5).unwrap();
        book.inscribe(spell("heal"), 1, 5).unwrap();
        assert_eq!(book.encode(), "s=1;b=firebolt:2,heal:1");
        assert_eq!(Book::parse(Some(&book.encode()), 5), book);
        assert_eq!(Book::parse(None, 5), Book::default());
        assert_eq!(Book::default().encode(), "s=0;b=");
        assert_eq!(Book::parse(Some("s=0;b="), 5), Book::default());
    }

    #[test]
    fn bad_data_is_read_as_far_as_it_makes_sense() {
        let book = Book::parse(Some("s=9;b=firebolt:99,bogus:1,heal,heal:2,heal:3,gust:x"), 5);
        assert_eq!(book.spells, vec![(spell("firebolt"), 5), (spell("heal"), 2)]);
        assert_eq!(book.selected, 0);
        let cut = Book::parse(Some("s=1;b=firebolt:1,heal:1,gust:1"), 2);
        assert_eq!(cut.spells.len(), 2);
        assert_eq!(cut.current().map(|(s, _)| s.id), Some("heal"));
    }

    #[test]
    fn inscribing_fills_upgrades_and_refuses() {
        let mut book = Book::default();
        assert_eq!(book.inscribe(spell("heal"), 2, 2), Ok(()));
        assert_eq!(book.inscribe(spell("heal"), 2, 2), Err(Refused::Known));
        assert_eq!(book.inscribe(spell("heal"), 1, 2), Err(Refused::Known));
        assert_eq!(book.inscribe(spell("gust"), 1, 2), Ok(()));
        assert_eq!(book.selected, 1);
        assert_eq!(book.inscribe(spell("icicle"), 1, 2), Err(Refused::Full));
        // A higher level replaces the page even in a full book.
        assert_eq!(book.inscribe(spell("heal"), 4, 2), Ok(()));
        assert_eq!(book.selected, 0);
        assert_eq!(book.current(), Some((spell("heal"), 4)));
        // Levels past the spell's cap are brought down to it.
        assert_eq!(book.inscribe(spell("gust"), 9, 2), Ok(()));
        assert_eq!(book.current(), Some((spell("gust"), 3)));
    }

    #[test]
    fn cycling_wraps_both_ways() {
        let mut book = Book::default();
        assert_eq!(book.cycle(true), None);
        for id in ["firebolt", "heal", "gust"] {
            book.inscribe(spell(id), 1, 5).unwrap();
        }
        assert_eq!(book.selected, 2);
        assert_eq!(book.cycle(true).map(|(s, _)| s.id), Some("firebolt"));
        assert_eq!(book.cycle(false).map(|(s, _)| s.id), Some("gust"));
        assert_eq!(book.cycle(false).map(|(s, _)| s.id), Some("heal"));
    }

    #[test]
    fn a_page_can_be_torn_out() {
        let mut book = Book::default();
        assert_eq!(book.remove_selected(), None);
        for id in ["firebolt", "heal"] {
            book.inscribe(spell(id), 1, 5).unwrap();
        }
        assert_eq!(book.remove_selected().map(|(s, _)| s.id), Some("heal"));
        assert_eq!(book.selected, 0);
        assert_eq!(book.remove_selected().map(|(s, _)| s.id), Some("firebolt"));
        assert_eq!(book.selected, 0);
        assert_eq!(book.remove_selected(), None);
    }
}
