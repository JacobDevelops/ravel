//! The small utilities of §12.
//!
//! A clamp, a registry-presence test, a test of whether a package is loaded, and
//! a rotation of a point about a centre.
//!
//! # The changes declared here, both §13's
//!
//! - **The presence predicate is named for what it returns.** The original's is
//!   named for invalidity and its body returns true when the identifier *is*
//!   present, so every caller that read the name got the opposite of what it
//!   asked for. [`registry_holds`] returns what its name says.
//! - **A malformed spelling is absent rather than a throw.** The original's
//!   string overload parses before testing, so the routine named for invalidity
//!   raises on a genuinely invalid identifier instead of reporting it.
//!   [`registry_holds_spelling`] answers false.
//!
//! # Declined
//!
//! **The hash strategy that treats two classes as equal when their names match.**
//! It exists in the original only to key §4's block-entity cache, and
//! [`crate::registration::ClassId`] keys that by identity, so the strategy has nothing
//! to be for. Implementing it would be implementing the collision.

use ravel_sdk::ModId;
use ravel_sdk::content::{ContentId, Registrar};

/// Clamps a value between a floor and a ceiling.
///
/// A value that is not a number answers the floor, which is what comparing it
/// against both bounds yields and is stated here rather than left to be
/// discovered.
#[must_use]
pub fn clamp(value: f64, low: f64, high: f64) -> f64 {
    if value.is_nan() || value < low {
        low
    } else if value > high {
        high
    } else {
        value
    }
}

/// Returns whether a registry holds an identifier.
#[must_use]
pub fn registry_holds(registry: &[ContentId], id: &ContentId) -> bool {
    registry.contains(id)
}

/// Returns whether a registry holds the identifier a spelling names.
///
/// A spelling that names no legal identifier is not held, which is the answer
/// the question has. It is not an error.
#[must_use]
pub fn registry_holds_spelling(registry: &[ContentId], spelling: &str) -> bool {
    spelling
        .parse::<ContentId>()
        .is_ok_and(|id| registry_holds(registry, &id))
}

/// Returns whether a package is loaded.
///
/// The loader answers this, and it answers the same before and during
/// construction, which is what the original's two-list fallback is for.
#[must_use]
pub fn is_loaded(registrar: &dyn Registrar, package: &ModId) -> bool {
    registrar.loads(package)
}

/// Which axis a rotation turns about.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Axis {
    /// The east-west axis.
    X,
    /// The vertical axis.
    Y,
    /// The north-south axis.
    Z,
}

/// Rotates a point about a centre, around one axis, by an angle in radians.
#[must_use]
pub fn rotate(
    point: (f64, f64, f64),
    centre: (f64, f64, f64),
    axis: Axis,
    radians: f64,
) -> (f64, f64, f64) {
    let (dx, dy, dz) = (point.0 - centre.0, point.1 - centre.1, point.2 - centre.2);
    let (sin, cos) = radians.sin_cos();
    let (rx, ry, rz) = match axis {
        Axis::X => {
            let y_cos = dy * cos;
            let z_sin = dz * sin;
            let y_sin = dy * sin;
            let z_cos = dz * cos;
            (dx, y_cos - z_sin, y_sin + z_cos)
        }
        Axis::Y => {
            let x_cos = dx * cos;
            let z_sin = dz * sin;
            let negative_x_sin = -dx * sin;
            let z_cos = dz * cos;
            (x_cos + z_sin, dy, negative_x_sin + z_cos)
        }
        Axis::Z => {
            let x_cos = dx * cos;
            let y_sin = dy * sin;
            let x_sin = dx * sin;
            let y_cos = dy * cos;
            (x_cos - y_sin, x_sin + y_cos, dz)
        }
    };
    (centre.0 + rx, centre.1 + ry, centre.2 + rz)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use ravel_sdk::ModId;
    use ravel_sdk::content::{ContentId, Registrar, RegistrationRefused, RegistryKey};

    use super::{Axis, clamp, is_loaded, registry_holds, registry_holds_spelling, rotate};

    struct Loading {
        loaded: Vec<ModId>,
        asked: Cell<usize>,
    }

    impl Registrar for Loading {
        fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
            let _ = key;
            Ok(())
        }

        fn claimed(&self) -> &[RegistryKey] {
            &[]
        }

        fn loads(&self, mod_id: &ModId) -> bool {
            self.asked.set(self.asked.get() + 1);
            self.loaded.contains(mod_id)
        }
    }

    fn id(path: &str) -> ContentId {
        ContentId::new(
            "rvl_146_counterpart"
                .parse()
                .expect("the declared identity is a legal mod id"),
            path,
        )
        .expect("a legal content path")
    }

    #[test]
    fn a_clamp_holds_a_value_between_its_bounds_and_answers_the_floor_for_no_number() {
        assert!((clamp(0.5, 0.0, 1.0) - 0.5).abs() < f64::EPSILON);
        assert!((clamp(-1.0, 0.0, 1.0) - 0.0).abs() < f64::EPSILON);
        assert!((clamp(2.0, 0.0, 1.0) - 1.0).abs() < f64::EPSILON);
        assert!((clamp(f64::NAN, 0.0, 1.0) - 0.0).abs() < f64::EPSILON);
        assert!(clamp(f64::INFINITY, 0.0, 1.0) <= 1.0);
    }

    #[test]
    fn the_presence_test_returns_what_its_name_says() {
        let registry = [id("crusher")];
        assert!(registry_holds(&registry, &id("crusher")));
        assert!(!registry_holds(&registry, &id("mixer")));
    }

    #[test]
    fn a_spelling_naming_no_legal_identifier_is_not_held_rather_than_a_failure() {
        let registry = [id("crusher")];
        assert!(registry_holds_spelling(
            &registry,
            "rvl_146_counterpart:crusher"
        ));
        assert!(!registry_holds_spelling(&registry, "no namespace at all"));
        assert!(!registry_holds_spelling(&registry, ""));
        assert!(!registry_holds_spelling(&registry, ":"));
    }

    #[test]
    fn whether_a_package_is_loaded_is_asked_of_the_loader_and_of_no_second_list() {
        // The original consults a still-loading list when the loaded list is not
        // available yet, and that fallback is the whole of its complexity. What
        // has to hold here is that every question reaches the loader, which
        // answers the same at both times, rather than a list this package keeps.
        let other: ModId = "another_package".parse().expect("a legal mod id");
        let registrar = Loading {
            loaded: vec![other.clone()],
            asked: Cell::new(0),
        };
        assert!(is_loaded(&registrar, &other));
        assert!(!is_loaded(
            &registrar,
            &"absent_package".parse().expect("a legal mod id")
        ));
        assert_eq!(
            registrar.asked.get(),
            2,
            "both questions went to the loader"
        );
    }

    #[test]
    fn a_quarter_turn_about_each_axis_moves_the_point_the_way_the_axis_says() {
        let centre = (0.0, 0.0, 0.0);
        let quarter = std::f64::consts::FRAC_PI_2;
        let close = |left: (f64, f64, f64), right: (f64, f64, f64)| {
            (left.0 - right.0).abs() < 1e-9
                && (left.1 - right.1).abs() < 1e-9
                && (left.2 - right.2).abs() < 1e-9
        };
        assert!(close(
            rotate((0.0, 1.0, 0.0), centre, Axis::X, quarter),
            (0.0, 0.0, 1.0)
        ));
        assert!(close(
            rotate((1.0, 0.0, 0.0), centre, Axis::Y, quarter),
            (0.0, 0.0, -1.0)
        ));
        assert!(close(
            rotate((1.0, 0.0, 0.0), centre, Axis::Z, quarter),
            (0.0, 1.0, 0.0)
        ));
    }

    #[test]
    fn a_rotation_about_a_centre_leaves_the_centre_where_it_is() {
        let centre = (4.0, 5.0, 6.0);
        let turned = rotate(centre, centre, Axis::Y, 1.234);
        assert!((turned.0 - centre.0).abs() < 1e-9);
        assert!((turned.1 - centre.1).abs() < 1e-9);
        assert!((turned.2 - centre.2).abs() < 1e-9);
        let full = rotate((7.0, 5.0, 6.0), centre, Axis::Y, std::f64::consts::TAU);
        assert!((full.0 - 7.0).abs() < 1e-9);
    }
}
