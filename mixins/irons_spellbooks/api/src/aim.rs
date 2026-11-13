//! Where a spell goes: the direction a player looks, and the point a
//! projectile starts from. Plain arithmetic, so it is testable without a game.

/// Where the player looks, as a unit vector, from vanilla's yaw and pitch in
/// degrees: yaw 0 faces `+Z` (south), yaw 90 faces `-X` (west).
pub fn direction(yaw: f32, pitch: f32) -> [f64; 3] {
    let yaw = f64::from(yaw).to_radians();
    let pitch = f64::from(pitch).to_radians();
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    [-sy * cp, -sp, cy * cp]
}

/// Eye height above the feet, as a standing player's.
pub const EYE: f64 = 1.62;

/// `at` is the caster's feet; the eye is [`EYE`] above it, and the spell
/// starts `reach` blocks along `dir` so it does not spawn inside its caster.
pub fn cast_from(at: [f64; 3], dir: [f64; 3], reach: f64) -> [f64; 3] {
    [
        at[0] + dir[0] * reach,
        at[1] + EYE + dir[1] * reach,
        at[2] + dir[2] * reach,
    ]
}

/// A projectile's motion for `speed` blocks a tick.
pub fn motion(dir: [f64; 3], speed: f64) -> [f64; 3] {
    [dir[0] * speed, dir[1] * speed, dir[2] * speed]
}

/// How far a blink may go: what was asked, cut back to just before the block
/// the crosshair met (a whole block is left between the player and the wall).
pub fn blink_reach(wanted: f64, hit: Option<f64>) -> f64 {
    match hit {
        Some(distance) if distance < wanted => (distance - 1.0).max(0.0),
        _ => wanted,
    }
}

/// A `Motion` list for `/summon`, with three decimals.
pub fn motion_list(motion: [f64; 3]) -> String {
    format!("[{:.3},{:.3},{:.3}]", motion[0], motion[1], motion[2])
}

/// A position for a command, with two decimals and a leading `~`-less number.
pub fn point(at: [f64; 3]) -> String {
    format!("{:.2} {:.2} {:.2}", at[0], at[1], at[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn yaw_zero_faces_south_and_ninety_faces_west() {
        let south = direction(0.0, 0.0);
        assert!(close(south[0], 0.0) && close(south[1], 0.0) && close(south[2], 1.0));
        let west = direction(90.0, 0.0);
        assert!(close(west[0], -1.0) && close(west[2], 0.0), "{west:?}");
        let north = direction(180.0, 0.0);
        assert!(close(north[2], -1.0), "{north:?}");
    }

    #[test]
    fn looking_down_points_down_and_the_vector_stays_a_unit() {
        let down = direction(0.0, 90.0);
        assert!(close(down[1], -1.0), "{down:?}");
        let tilted = direction(37.0, -22.0);
        let length = (tilted[0] * tilted[0] + tilted[1] * tilted[1] + tilted[2] * tilted[2]).sqrt();
        assert!(close(length, 1.0), "{length}");
    }

    #[test]
    fn a_cast_starts_at_the_eye_and_moves_along_the_aim() {
        let at = cast_from([10.0, 64.0, -3.0], [1.0, 0.0, 0.0], 1.0);
        assert_eq!(at, [11.0, 65.62, -3.0]);
    }

    #[test]
    fn a_blink_stops_a_block_before_the_wall() {
        assert_eq!(blink_reach(16.0, None), 16.0);
        assert_eq!(blink_reach(16.0, Some(4.0)), 3.0);
        assert_eq!(blink_reach(16.0, Some(20.0)), 16.0);
        assert_eq!(blink_reach(16.0, Some(0.5)), 0.0);
    }

    #[test]
    fn motion_and_lists_read_as_commands_expect() {
        let m = motion([0.0, 1.0, -1.0], 2.0);
        assert_eq!(m, [0.0, 2.0, -2.0]);
        assert_eq!(motion_list(m), "[0.000,2.000,-2.000]");
        assert_eq!(point([1.0, 64.5, -2.25]), "1.00 64.50 -2.25");
    }
}
