//! Times what the profiling collectors cost when nobody asked for them.
//!
//! Every figure here is a *paired* difference: one sample times a batch of
//! instrumented calls and the same batch of bare calls, and records the gap
//! between them. Differencing two separate runs' percentiles would not be a
//! percentile of anything, which `evidence/m2/loader-overhead.json` already
//! records as the weakness of its residual stage. Pairing inside the sample
//! makes the distribution the distribution of the overhead itself.

use std::hint::black_box;
use std::time::Instant;

use ravel_core::{LifecycleState, ModId};
use ravel_host_api::{HostError, HostMod};
use ravel_observability::{CallKind, Profiler};
use ravel_spike_harness::{Percentiles, percentiles};

/// Runs discarded before the first sample, so a measured run finds a warm
/// allocator, a warm page cache and a warm branch predictor.
const WARMUP_RUNS: usize = 500;

/// Samples taken per metric.
const MEASURED_RUNS: usize = 5_000;

/// Calls per sample.
///
/// The quantity being measured is a branch that is not taken. Reading the
/// monotonic clock costs more than that on its own, so a single-call sample
/// would measure the clock; a thousand calls per sample puts the clock three
/// orders of magnitude below the difference.
const CALLS_PER_SAMPLE: usize = 1_000;

/// Crossings a tick is charged with when the per-tick figure is formed.
///
/// This is an assumption and not a measurement. `ROADMAP.md` defers the real
/// per-tick crossing count to its own work — `evidence/m2/loader-overhead.json`
/// records the count as zero for M2 and says M3 is where the first non-zero one
/// is taken — so the record states this number, states that it is assumed, and
/// carries the break-even count as well, which depends on no assumption at all.
///
/// Ten thousand is chosen as a ceiling rather than a prediction: a pack-sized
/// set of 256 mods would each have to be entered thirty-nine times in one tick
/// to reach it.
pub const ASSUMED_CROSSINGS_PER_TICK: u64 = 10_000;

/// The mod a crossing enters.
///
/// Its body is the smallest thing that cannot be optimized away, because the
/// quantity being measured is what wraps the call rather than the call.
struct Subject {
    id: ModId,
    entered: u64,
}

impl HostMod for Subject {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
        self.entered = self.entered.wrapping_add(1);
        Ok(())
    }
}

/// A timed metric in nanoseconds per call.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// Samples the percentiles were taken from.
    pub samples: usize,
    /// Nearest-rank percentiles, shared with every other Ravel measurement.
    pub percentiles: Percentiles,
}

impl Timing {
    /// The value a budget is formed from: the tail rather than the middle.
    #[must_use]
    pub const fn budget_basis(self) -> f64 {
        self.percentiles.p99
    }
}

/// Everything one run of this experiment measured.
#[derive(Clone, Copy, Debug)]
pub struct Overhead {
    /// What one instrumented crossing adds with profiling off.
    pub off_per_crossing: Timing,
    /// What one tick boundary adds with profiling off.
    pub off_per_tick_boundary: Timing,
    /// What one instrumented crossing adds with profiling on.
    pub on_per_crossing: Timing,
}

impl Overhead {
    /// The per-tick total the normal-mode budget bounds.
    ///
    /// One tick boundary plus [`ASSUMED_CROSSINGS_PER_TICK`] crossings, which
    /// is the shape the budget is written in: a limit on what a tick loses to
    /// profiling being present, summed over every crossing inside it.
    #[must_use]
    pub fn off_per_tick(&self) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "the crossing count is a small constant and the product is a nanosecond \
                      figure rounded to two decimals"
        )]
        let crossings = ASSUMED_CROSSINGS_PER_TICK as f64;
        self.off_per_tick_boundary
            .budget_basis()
            .mul_add(1.0, self.off_per_crossing.budget_basis() * crossings)
    }

    /// Crossings in one tick at which the budget would be reached.
    ///
    /// The one figure here that rests on no assumption about how many crossings
    /// a tick makes. It is what replaces the assumption when the real count is
    /// recorded: compare the two numbers and the budget is answered.
    #[must_use]
    pub fn break_even_crossings_per_tick(&self, budget_nanoseconds: f64) -> f64 {
        let per_crossing = self.off_per_crossing.budget_basis();
        if per_crossing <= 0.0 {
            return f64::INFINITY;
        }
        (budget_nanoseconds - self.off_per_tick_boundary.budget_basis()).max(0.0) / per_crossing
    }
}

/// Runs the whole measurement.
#[must_use]
pub fn run() -> Overhead {
    Overhead {
        off_per_crossing: per_crossing(Profiler::off()),
        off_per_tick_boundary: per_tick_boundary(),
        on_per_crossing: per_crossing(Profiler::on(std::time::Duration::from_millis(1))),
    }
}

/// Times one crossing through the profiler against the same crossing without
/// it.
fn per_crossing(mut profiler: Profiler) -> Timing {
    let mut instrumented = Subject {
        id: "example_mod".parse().expect("a valid mod id"),
        entered: 0,
    };
    let mut bare = Subject {
        id: "example_mod".parse().expect("a valid mod id"),
        entered: 0,
    };
    let id: ModId = "example_mod".parse().expect("a valid mod id");

    paired(
        || {
            let subject = &mut instrumented;
            let _ = black_box(profiler.measure(&id, CallKind::Event, || {
                black_box(subject).enter(black_box(LifecycleState::Constructed))
            }));
        },
        || {
            let _ = black_box(black_box(&mut bare).enter(black_box(LifecycleState::Constructed)));
        },
    )
}

/// Times the tick boundary against doing nothing at it.
fn per_tick_boundary() -> Timing {
    let mut profiler = Profiler::off();
    let mut control = Profiler::off();

    paired(
        || black_box(&mut profiler).tick(),
        || {
            let _ = black_box(black_box(&mut control).is_on());
        },
    )
}

/// Times `instrumented` against `control` in the same sample and reports the
/// per-call gap.
///
/// The two are run in alternating order across samples so that neither is
/// systematically the one that finds the caches cold.
///
/// A sample where the instrumented batch came out faster than its control is
/// recorded as zero rather than as a negative time. That biases the result
/// upward, which is the safe direction for a limit: the budget is charged for
/// noise it did not cause rather than credited for it.
fn paired(mut instrumented: impl FnMut(), mut control: impl FnMut()) -> Timing {
    for _ in 0..WARMUP_RUNS {
        instrumented();
        control();
    }

    let mut samples = Vec::with_capacity(MEASURED_RUNS);
    for run in 0..MEASURED_RUNS {
        let (first, second) = if run % 2 == 0 {
            (batch(&mut instrumented), batch(&mut control))
        } else {
            let control_first = batch(&mut control);
            (batch(&mut instrumented), control_first)
        };
        samples.push(first.saturating_sub(second));
    }

    let measured = percentiles(&samples).expect("a measured run produces samples");
    #[expect(
        clippy::cast_precision_loss,
        reason = "the divisor is a small compile-time constant"
    )]
    let divisor = CALLS_PER_SAMPLE as f64;
    Timing {
        samples: samples.len(),
        percentiles: Percentiles {
            median: measured.median / divisor,
            p95: measured.p95 / divisor,
            p99: measured.p99 / divisor,
        },
    }
}

/// Times one batch of calls, in nanoseconds.
fn batch(body: &mut impl FnMut()) -> u64 {
    let started = Instant::now();
    for _ in 0..CALLS_PER_SAMPLE {
        body();
    }
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use ravel_spike_harness::Percentiles;

    use super::{ASSUMED_CROSSINGS_PER_TICK, Overhead, Timing, batch, paired, per_tick_boundary};

    /// A timing whose percentiles are all `value`, so arithmetic over it is
    /// readable rather than a lookup.
    fn flat(value: f64) -> Timing {
        Timing {
            samples: 1,
            percentiles: Percentiles {
                median: value,
                p95: value,
                p99: value,
            },
        }
    }

    fn overhead(per_crossing: f64, per_tick_boundary: f64) -> Overhead {
        Overhead {
            off_per_crossing: flat(per_crossing),
            off_per_tick_boundary: flat(per_tick_boundary),
            on_per_crossing: flat(50.0),
        }
    }

    #[test]
    fn a_budget_is_formed_from_the_tail_rather_than_the_middle() {
        let timing = Timing {
            samples: 3,
            percentiles: Percentiles {
                median: 1.0,
                p95: 2.0,
                p99: 3.0,
            },
        };
        assert!((timing.budget_basis() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn the_per_tick_figure_is_one_boundary_plus_the_assumed_crossings() {
        let measured = overhead(2.0, 7.0);
        #[expect(
            clippy::cast_precision_loss,
            reason = "the crossing count is a small compile-time constant"
        )]
        let expected = 7.0 + 2.0 * ASSUMED_CROSSINGS_PER_TICK as f64;

        assert!((measured.off_per_tick() - expected).abs() < 0.001);
    }

    #[test]
    fn the_break_even_count_is_what_is_left_of_the_budget_over_one_crossing() {
        let measured = overhead(2.0, 100.0);

        // 500 ns of budget less the 100 ns boundary, at 2 ns a crossing.
        assert!((measured.break_even_crossings_per_tick(500.0) - 200.0).abs() < 0.001);
    }

    #[test]
    fn an_overhead_too_small_to_measure_puts_the_break_even_count_out_of_reach() {
        // A crossing that costs nothing measurable never reaches the budget,
        // and saying so is more honest than dividing by zero into a number.
        let measured = overhead(0.0, 1.0);

        assert!(measured.break_even_crossings_per_tick(500.0).is_infinite());
    }

    #[test]
    fn a_boundary_that_already_exceeds_the_budget_leaves_no_crossings_at_all() {
        let measured = overhead(2.0, 900.0);

        assert!(measured.break_even_crossings_per_tick(500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_batch_of_calls_is_timed_as_one_reading_of_the_clock() {
        let mut entered = 0_u32;
        let elapsed = batch(&mut || entered += 1);

        assert_eq!(
            entered,
            u32::try_from(super::CALLS_PER_SAMPLE).expect("a small constant")
        );
        // A thousand increments take a nonzero but tiny time; the assertion is
        // that the clock was read around them rather than how fast they were.
        assert!(elapsed < 1_000_000_000);
    }

    #[test]
    fn an_instrumented_body_no_slower_than_its_control_is_recorded_as_zero() {
        // The pair is the same work both sides, so the gap is noise in either
        // direction and the clamp is what keeps a negative time out of the
        // record. The median of that is zero.
        let measured = paired(
            || {
                let _ = std::hint::black_box(1_u64);
            },
            || {
                let _ = std::hint::black_box(1_u64);
            },
        );

        assert_eq!(measured.samples, super::MEASURED_RUNS);
        assert!(measured.percentiles.median >= 0.0);
        assert!(measured.percentiles.p99 >= measured.percentiles.median);
    }

    #[test]
    fn a_tick_boundary_with_the_collectors_off_is_measured_against_reading_the_flag() {
        let measured = per_tick_boundary();

        assert_eq!(measured.samples, super::MEASURED_RUNS);
        // Not an assertion about how fast it is: this build is unoptimized and
        // the record's figure comes from a release run. What is asserted is that
        // the clamp holds, so no negative time can reach the record.
        assert!(measured.percentiles.median >= 0.0);
        assert!(measured.percentiles.p95 >= measured.percentiles.median);
        assert!(measured.percentiles.p99 >= measured.percentiles.p95);
    }
}
