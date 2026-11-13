//! A player's mana pool and spell cooldowns: plain arithmetic, no game types.

/// Mana every player has with no gear.
pub const BASE_MAX: f32 = 100.0;
/// What a tick gives back: 1% of the maximum a second.
pub const REGEN_PER_TICK: f32 = 0.01 / 20.0;
/// What each piece of mage armour adds to the maximum.
pub const ARMOUR_BONUS: f32 = 25.0;
/// What a mana potion gives back.
pub const POTION_RESTORE: f32 = 50.0;
/// How much stronger spells are with a staff in either hand.
pub const STAFF_POWER: f32 = 1.15;

#[derive(Clone, Debug, PartialEq)]
pub struct Pool {
    pub mana: f32,
    /// Mana above [`BASE_MAX`], from what the player wears.
    pub bonus: f32,
    /// Spell ids and the ticks each still rests.
    cooldowns: Vec<(&'static str, u32)>,
}

impl Default for Pool {
    fn default() -> Self {
        Self {
            mana: BASE_MAX,
            bonus: 0.0,
            cooldowns: Vec::new(),
        }
    }
}

impl Pool {
    pub fn with_mana(mana: f32) -> Self {
        let mut pool = Self::default();
        pool.set(mana);
        pool
    }

    pub fn max(&self) -> f32 {
        BASE_MAX + self.bonus.max(0.0)
    }

    /// Sets the mana, kept between 0 and the maximum; a value that is not a
    /// number changes nothing.
    pub fn set(&mut self, mana: f32) {
        if mana.is_finite() {
            self.mana = mana.clamp(0.0, self.max());
        }
    }

    pub fn add(&mut self, amount: f32) {
        self.set(self.mana + amount);
    }

    /// The gear changed: a smaller maximum cuts what is above it.
    pub fn set_bonus(&mut self, bonus: f32) {
        self.bonus = if bonus.is_finite() { bonus.max(0.0) } else { 0.0 };
        self.mana = self.mana.min(self.max());
    }

    /// Takes `cost` when the pool holds it; nothing is taken otherwise.
    pub fn spend(&mut self, cost: f32) -> bool {
        if !cost.is_finite() || cost < 0.0 || self.mana + 1e-3 < cost {
            return false;
        }
        self.mana = (self.mana - cost).max(0.0);
        true
    }

    /// One server tick: mana comes back and cooldowns run down.
    pub fn tick(&mut self) {
        let max = self.max();
        if self.mana < max {
            self.mana = (self.mana + max * REGEN_PER_TICK).min(max);
        }
        for entry in &mut self.cooldowns {
            entry.1 = entry.1.saturating_sub(1);
        }
        self.cooldowns.retain(|entry| entry.1 > 0);
    }

    pub fn cooldown(&self, spell: &str) -> u32 {
        self.cooldowns
            .iter()
            .find(|entry| entry.0 == spell)
            .map_or(0, |entry| entry.1)
    }

    pub fn start_cooldown(&mut self, spell: &'static str, ticks: u32) {
        self.cooldowns.retain(|entry| entry.0 != spell);
        if ticks > 0 {
            self.cooldowns.push((spell, ticks));
        }
    }

    pub fn cooldowns(&self) -> &[(&'static str, u32)] {
        &self.cooldowns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_pool_is_full() {
        let pool = Pool::default();
        assert_eq!(pool.mana, 100.0);
        assert_eq!(pool.max(), 100.0);
    }

    #[test]
    fn spending_needs_the_whole_cost() {
        let mut pool = Pool::with_mana(25.0);
        assert!(!pool.spend(30.0));
        assert_eq!(pool.mana, 25.0);
        assert!(pool.spend(25.0));
        assert_eq!(pool.mana, 0.0);
        assert!(!pool.spend(f32::NAN));
        assert!(!pool.spend(-1.0));
        assert!(pool.spend(0.0));
    }

    #[test]
    fn a_pool_refills_in_a_hundred_seconds_and_no_further() {
        let mut pool = Pool::with_mana(0.0);
        for _ in 0..20 {
            pool.tick();
        }
        assert!((pool.mana - 1.0).abs() < 1e-3, "{}", pool.mana);
        for _ in 0..(100 * 20) {
            pool.tick();
        }
        assert_eq!(pool.mana, 100.0);
    }

    #[test]
    fn gear_raises_the_maximum_and_losing_it_cuts_the_surplus() {
        let mut pool = Pool::default();
        pool.set_bonus(4.0 * ARMOUR_BONUS);
        assert_eq!(pool.max(), 200.0);
        pool.set(500.0);
        assert_eq!(pool.mana, 200.0);
        pool.set_bonus(0.0);
        assert_eq!(pool.mana, 100.0);
        pool.set(f32::INFINITY);
        assert_eq!(pool.mana, 100.0);
        pool.add(-250.0);
        assert_eq!(pool.mana, 0.0);
    }

    #[test]
    fn cooldowns_run_down_and_leave() {
        let mut pool = Pool::default();
        pool.start_cooldown("firebolt", 3);
        pool.start_cooldown("heal", 1);
        assert_eq!(pool.cooldown("firebolt"), 3);
        pool.tick();
        assert_eq!(pool.cooldown("firebolt"), 2);
        assert_eq!(pool.cooldown("heal"), 0);
        assert_eq!(pool.cooldowns().len(), 1);
        pool.start_cooldown("firebolt", 10);
        assert_eq!(pool.cooldown("firebolt"), 10);
        assert_eq!(pool.cooldowns().len(), 1);
    }
}
