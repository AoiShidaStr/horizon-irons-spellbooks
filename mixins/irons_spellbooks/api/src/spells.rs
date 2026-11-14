//! The spell table: what each spell costs, how long it rests and how strong it
//! is at each level. Pure data and arithmetic, shared by the server half (which
//! casts) and the client half (which shows tooltips and the HUD).
//!
//! The spells, their numbers and their wording are this mod's own.

/// The mod's namespace: its items, channels and files.
pub const MOD_ID: &str = "irons_spellbooks";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum School {
    Fire,
    Ice,
    Lightning,
    Holy,
    Ender,
    Evocation,
    Blood,
    Nature,
}

impl School {
    pub fn name(self) -> &'static str {
        match self {
            School::Fire => "Fire",
            School::Ice => "Ice",
            School::Lightning => "Lightning",
            School::Holy => "Holy",
            School::Ender => "Ender",
            School::Evocation => "Evocation",
            School::Blood => "Blood",
            School::Nature => "Nature",
        }
    }

    /// The legacy colour code (the character after the section sign) that
    /// tooltips and chat lines use for the school.
    pub fn code(self) -> char {
        match self {
            School::Fire => '6',
            School::Ice => 'b',
            School::Lightning => '9',
            School::Holy => 'e',
            School::Ender => 'd',
            School::Evocation => 'a',
            School::Blood => '4',
            School::Nature => '2',
        }
    }

    /// The school's colour, 0xRRGGBB, for the HUD and dust particles.
    pub fn rgb(self) -> u32 {
        match self {
            School::Fire => 0xF0_7A_1E,
            School::Ice => 0x8F_E3_F5,
            School::Lightning => 0x5C_7C_FF,
            School::Holy => 0xF7_E8_8A,
            School::Ender => 0xC0_5C_F0,
            School::Evocation => 0x7B_E0_8C,
            School::Blood => 0xB0_20_30,
            School::Nature => 0x4E_9F_3D,
        }
    }

    /// The vanilla effect a spell of this school leaves on what it wounds, as
    /// its id, its amplifier and how many ticks it lasts. Fire sets what it
    /// hits alight instead ([`School::ignite_ticks`]), and the schools with
    /// none only wound.
    pub fn affliction(self) -> Option<(&'static str, u8, i32)> {
        match self {
            School::Ice => Some(("minecraft:slowness", 1, 100)),
            School::Lightning => Some(("minecraft:glowing", 0, 100)),
            School::Nature => Some(("minecraft:poison", 0, 100)),
            School::Blood => Some(("minecraft:weakness", 0, 200)),
            School::Fire | School::Holy | School::Ender | School::Evocation => None,
        }
    }

    /// How long a spell of the school sets what it hits alight, in ticks: three
    /// seconds for fire, nothing for the other schools.
    pub fn ignite_ticks(self) -> i32 {
        match self {
            School::Fire => 60,
            _ => 0,
        }
    }

    /// The sound a spell of the school plays at its caster.
    pub fn sound(self) -> &'static str {
        match self {
            School::Fire => "minecraft:item.firecharge.use",
            School::Ice => "minecraft:block.glass.break",
            School::Lightning => "minecraft:entity.lightning_bolt.impact",
            School::Holy => "minecraft:block.beacon.activate",
            School::Ender => "minecraft:entity.enderman.teleport",
            School::Evocation => "minecraft:entity.evocation_illager.cast_spell",
            School::Blood => "minecraft:entity.witch.drink",
            School::Nature => "minecraft:block.grass.place",
        }
    }
}

/// What a spell does when it is cast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Firebolt,
    Fireball,
    Icicle,
    FrostStep,
    LightningBolt,
    Heal,
    Fortify,
    MagicMissile,
    Teleport,
    Gust,
    /// A ring of frost around the caster, thrown outwards.
    FrostNova,
    /// Lightning at the crosshair that leaps to what stands nearby.
    ChainLightning,
    /// A regeneration effect on the caster.
    Regeneration,
    /// Damage the creature in front and mend the caster with half of it.
    Siphon,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spell {
    /// The id in item names, commands and saved data (`firebolt`).
    pub id: &'static str,
    pub name: &'static str,
    pub kind: Kind,
    pub school: School,
    pub max_level: u8,
    /// Mana at level 1, and what each further level adds.
    pub mana: f32,
    pub mana_per_level: f32,
    /// Ticks before the same spell can be cast again from a spellbook.
    pub cooldown: u32,
    /// The spell's strength at level 1 and what each level adds: damage or
    /// healing in health points, or blocks for the movement spells.
    pub power: f32,
    pub power_per_level: f32,
    /// How far the spell reaches, in blocks.
    pub range: f32,
    pub summary: &'static str,
}

impl Spell {
    pub fn clamp_level(&self, level: u8) -> u8 {
        level.clamp(1, self.max_level)
    }

    pub fn mana_cost(&self, level: u8) -> f32 {
        let level = self.clamp_level(level);
        self.mana + self.mana_per_level * f32::from(level - 1)
    }

    /// Damage, healing or distance at `level`, times the caster's `power`
    /// multiplier (1.0 with no bonus).
    pub fn strength(&self, level: u8, power: f32) -> f32 {
        let level = self.clamp_level(level);
        (self.power + self.power_per_level * f32::from(level - 1)) * power
    }

    /// The item a scroll of this spell is.
    pub fn scroll_item(&self) -> String {
        format!("{MOD_ID}:scroll_{}", self.id)
    }
}

pub const SPELLS: [Spell; 15] = [
    Spell {
        id: "firebolt",
        name: "Firebolt",
        kind: Kind::Firebolt,
        school: School::Fire,
        max_level: 5,
        mana: 10.0,
        mana_per_level: 2.0,
        cooldown: 20,
        power: 5.0,
        power_per_level: 1.5,
        range: 32.0,
        summary: "A bolt of flame that burns the first creature it meets.",
    },
    Spell {
        id: "fireball",
        name: "Fireball",
        kind: Kind::Fireball,
        school: School::Fire,
        max_level: 3,
        mana: 30.0,
        mana_per_level: 10.0,
        cooldown: 120,
        power: 8.0,
        power_per_level: 3.0,
        range: 40.0,
        summary: "Bursts where it lands and scorches all within 3 blocks. Breaks no blocks.",
    },
    Spell {
        id: "icicle",
        name: "Icicle",
        kind: Kind::Icicle,
        school: School::Ice,
        max_level: 5,
        mana: 12.0,
        mana_per_level: 2.0,
        cooldown: 30,
        power: 4.0,
        power_per_level: 1.25,
        range: 32.0,
        summary: "A shard of ice that wounds the first creature it meets.",
    },
    Spell {
        id: "frost_step",
        name: "Frost Step",
        kind: Kind::FrostStep,
        school: School::Ice,
        max_level: 3,
        mana: 15.0,
        mana_per_level: 5.0,
        cooldown: 60,
        power: 6.0,
        power_per_level: 2.0,
        range: 12.0,
        summary: "A short blink the way you look.",
    },
    Spell {
        id: "lightning_bolt",
        name: "Lightning Bolt",
        kind: Kind::LightningBolt,
        school: School::Lightning,
        max_level: 5,
        mana: 35.0,
        mana_per_level: 5.0,
        cooldown: 140,
        power: 8.0,
        power_per_level: 2.5,
        range: 24.0,
        summary: "Calls lightning down where you look.",
    },
    Spell {
        id: "heal",
        name: "Heal",
        kind: Kind::Heal,
        school: School::Holy,
        max_level: 5,
        mana: 20.0,
        mana_per_level: 5.0,
        cooldown: 80,
        power: 4.0,
        power_per_level: 2.0,
        range: 0.0,
        summary: "Restores your own health.",
    },
    Spell {
        id: "fortify",
        name: "Fortify",
        kind: Kind::Fortify,
        school: School::Holy,
        max_level: 4,
        mana: 30.0,
        mana_per_level: 10.0,
        cooldown: 400,
        power: 4.0,
        power_per_level: 4.0,
        range: 8.0,
        summary: "Shields you and the players around you with absorption hearts.",
    },
    Spell {
        id: "magic_missile",
        name: "Magic Missile",
        kind: Kind::MagicMissile,
        school: School::Ender,
        max_level: 5,
        mana: 8.0,
        mana_per_level: 2.0,
        cooldown: 12,
        power: 3.0,
        power_per_level: 1.0,
        range: 36.0,
        summary: "A dart of raw magic that ignores armour.",
    },
    Spell {
        id: "teleport",
        name: "Teleport",
        kind: Kind::Teleport,
        school: School::Ender,
        max_level: 3,
        mana: 40.0,
        mana_per_level: 10.0,
        cooldown: 200,
        power: 16.0,
        power_per_level: 8.0,
        range: 32.0,
        summary: "A long blink the way you look.",
    },
    Spell {
        id: "gust",
        name: "Gust",
        kind: Kind::Gust,
        school: School::Evocation,
        max_level: 3,
        mana: 15.0,
        mana_per_level: 5.0,
        cooldown: 60,
        power: 1.0,
        power_per_level: 0.4,
        range: 8.0,
        summary: "A blast of wind that throws back what stands before you.",
    },
    Spell {
        id: "frost_nova",
        name: "Frost Nova",
        kind: Kind::FrostNova,
        school: School::Ice,
        max_level: 3,
        mana: 25.0,
        mana_per_level: 8.0,
        cooldown: 160,
        power: 6.0,
        power_per_level: 2.5,
        range: 6.0,
        summary: "Bursts in a ring of frost, wounding and throwing back what stands close.",
    },
    Spell {
        id: "chain_lightning",
        name: "Chain Lightning",
        kind: Kind::ChainLightning,
        school: School::Lightning,
        max_level: 4,
        mana: 30.0,
        mana_per_level: 8.0,
        cooldown: 180,
        power: 6.0,
        power_per_level: 2.0,
        range: 24.0,
        summary: "Strikes where you look and leaps to everything standing nearby.",
    },
    Spell {
        id: "blessing",
        name: "Blessing",
        kind: Kind::Regeneration,
        school: School::Holy,
        max_level: 4,
        mana: 25.0,
        mana_per_level: 8.0,
        cooldown: 300,
        power: 1.0,
        power_per_level: 1.0,
        range: 0.0,
        summary: "Mends your wounds over time.",
    },
    Spell {
        id: "blood_siphon",
        name: "Blood Siphon",
        kind: Kind::Siphon,
        school: School::Blood,
        max_level: 4,
        mana: 22.0,
        mana_per_level: 6.0,
        cooldown: 120,
        power: 5.0,
        power_per_level: 2.0,
        range: 6.0,
        summary: "Tears life from the creature before you and gives you half of it.",
    },
    Spell {
        id: "thorn_dart",
        name: "Thorn Dart",
        kind: Kind::Icicle,
        school: School::Nature,
        max_level: 5,
        mana: 10.0,
        mana_per_level: 2.0,
        cooldown: 24,
        power: 4.0,
        power_per_level: 1.2,
        range: 30.0,
        summary: "A dart of thorns that flies straight at the first creature it meets.",
    },
];

pub fn by_id(id: &str) -> Option<&'static Spell> {
    SPELLS.iter().find(|spell| spell.id == id)
}

/// `III` for 3; levels only go to 10.
pub fn roman(level: u8) -> &'static str {
    const NUMERALS: [&str; 11] = [
        "", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X",
    ];
    NUMERALS.get(usize::from(level)).copied().unwrap_or("X")
}

/// `2.5s` for 50 ticks.
pub fn seconds(ticks: u32) -> String {
    let tenths = ticks.div_ceil(2);
    if tenths % 10 == 0 {
        format!("{}s", tenths / 10)
    } else {
        format!("{}.{}s", tenths / 10, tenths % 10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_plain() {
        for (i, spell) in SPELLS.iter().enumerate() {
            assert!(
                spell.id.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{}",
                spell.id
            );
            assert!(
                SPELLS[i + 1..].iter().all(|other| other.id != spell.id),
                "{}",
                spell.id
            );
            assert_eq!(by_id(spell.id), Some(spell));
            assert!(spell.max_level >= 1 && spell.max_level <= 10);
        }
        assert_eq!(by_id("nope"), None);
    }

    #[test]
    fn cost_and_strength_grow_with_level_and_stop_at_the_cap() {
        let bolt = by_id("firebolt").unwrap();
        assert_eq!(bolt.mana_cost(1), 10.0);
        assert_eq!(bolt.mana_cost(3), 14.0);
        assert_eq!(bolt.mana_cost(0), 10.0);
        assert_eq!(bolt.mana_cost(99), bolt.mana_cost(5));
        assert_eq!(bolt.strength(1, 1.0), 5.0);
        assert_eq!(bolt.strength(3, 1.0), 8.0);
        assert_eq!(bolt.strength(3, 1.5), 12.0);
    }

    #[test]
    fn every_spell_is_castable_from_a_full_base_pool() {
        for spell in &SPELLS {
            assert!(spell.mana_cost(spell.max_level) <= 100.0, "{}", spell.id);
        }
    }

    #[test]
    fn every_school_sounds_and_only_some_afflict() {
        for spell in &SPELLS {
            assert!(spell.school.sound().starts_with("minecraft:"), "{}", spell.id);
            assert!(
                spell.school.ignite_ticks() == 0 || spell.school == School::Fire,
                "{}",
                spell.id
            );
        }
        assert_eq!(School::Ice.affliction(), Some(("minecraft:slowness", 1, 100)));
        assert_eq!(School::Nature.affliction(), Some(("minecraft:poison", 0, 100)));
        assert_eq!(School::Blood.affliction(), Some(("minecraft:weakness", 0, 200)));
        assert_eq!(School::Holy.affliction(), None);
        assert_eq!(School::Fire.ignite_ticks(), 60);
        assert_eq!(School::Ender.ignite_ticks(), 0);
    }

    #[test]
    fn numerals_and_seconds_read_well() {
        assert_eq!(roman(1), "I");
        assert_eq!(roman(4), "IV");
        assert_eq!(roman(200), "X");
        assert_eq!(seconds(20), "1s");
        assert_eq!(seconds(50), "2.5s");
        assert_eq!(seconds(1), "0.1s");
    }
}
