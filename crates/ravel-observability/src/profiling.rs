use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use ravel_core::ModId;

/// The default above which a callback is reported as slow.
///
/// A twentieth of a tick. At 20 TPS a tick is 50 ms, so a single callback that
/// takes longer than 2.5 ms has spent more of the tick than any one mod should
/// and is worth a player's attention. It is a reporting threshold and not a
/// limit: nothing is refused for crossing it.
pub const DEFAULT_SLOW_CALLBACK: Duration = Duration::from_micros(2_500);

/// How well a collector can measure one of `SPEC.md` §9.3's facts.
///
/// The three arms exist because two different things can be wrong with a
/// number. `SPEC.md` §9.3 requires proving "what can be attributed accurately
/// rather than presenting estimates as exact measurements", and a metric can
/// fail that either by not being measurable at all or by being measurable as
/// something adjacent to what its name promises. Naming both keeps a metric
/// from being quietly absent or reported as a zero that reads as "cost
/// nothing".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Support {
    /// Measured, and the number means what the metric's name says.
    Exact,
    /// Measured, and approximate in the way the note states.
    Approximate(&'static str),
    /// Not measurable at M3, for the stated reason.
    Unsupported(&'static str),
}

impl Support {
    /// Whether a collector produces a number for this at all.
    #[must_use]
    pub const fn is_measured(self) -> bool {
        matches!(self, Self::Exact | Self::Approximate(_))
    }

    /// The word a record or a screen labels this support with.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Approximate(_) => "approximate",
            Self::Unsupported(_) => "unsupported",
        }
    }

    /// The sentence that qualifies the number, or `None` when it is exact.
    #[must_use]
    pub const fn detail(self) -> Option<&'static str> {
        match self {
            Self::Exact => None,
            Self::Approximate(detail) | Self::Unsupported(detail) => Some(detail),
        }
    }
}

/// One fact profiling is asked for, and what M3 can say about it.
///
/// The list is `SPEC.md` §9.3's, item for item, plus [`Self::InclusiveTime`],
/// which is the quantity reported where §9.3 asks for CPU time and is named
/// separately rather than being reported under that name. Wall-clock time
/// spent inside a callback is not an imprecise CPU time — it is a different
/// quantity that can be arbitrarily larger — so labelling it "estimated CPU
/// time" would be exactly the presentation §9.3 forbids.
///
/// Every entry but [`Self::TickCount`] is per mod. That one is a fact about the
/// run rather than about a mod, and it is in this table anyway because this
/// table is what a surface generates its "not measurable in this build" list
/// out of: a figure the report cannot produce belongs on that list whatever its
/// scope, and the alternative is a clause that quietly disappears.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Metric {
    /// Bytes still held by a mod's own allocations.
    RetainedMemory,
    /// Bytes a mod has allocated over the process run.
    AllocatedMemory,
    /// Processor time a mod's own code consumed.
    CpuTime,
    /// Wall-clock time between entering and leaving a mod's callback.
    InclusiveTime,
    /// How many game-hook invocations reached a mod.
    HookCalls,
    /// How many of those carried a game event rather than a command.
    EventCalls,
    /// How many callbacks ran longer than the slow-callback threshold.
    SlowCallbacks,
    /// How many calls crossed the Java/Rust boundary into a mod.
    BoundaryCalls,
    /// How many bytes those crossings carried.
    TransferredBytes,
    /// How much of the registries a mod claimed.
    RegistryFootprint,
    /// How much saved data a mod owns in a world.
    SavedDataFootprint,
    /// How many game ticks the collectors saw.
    TickCount,
}

impl Metric {
    /// Every metric, in the order a record and a screen list them.
    pub const ALL: [Self; 12] = [
        Self::RetainedMemory,
        Self::AllocatedMemory,
        Self::CpuTime,
        Self::InclusiveTime,
        Self::HookCalls,
        Self::EventCalls,
        Self::SlowCallbacks,
        Self::BoundaryCalls,
        Self::TransferredBytes,
        Self::RegistryFootprint,
        Self::SavedDataFootprint,
        Self::TickCount,
    ];

    /// The name a record and a screen call this metric by.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::RetainedMemory => "retained-memory",
            Self::AllocatedMemory => "allocated-memory",
            Self::CpuTime => "cpu-time",
            Self::InclusiveTime => "inclusive-time",
            Self::HookCalls => "hook-calls",
            Self::EventCalls => "event-calls",
            Self::SlowCallbacks => "slow-callbacks",
            Self::BoundaryCalls => "boundary-calls",
            Self::TransferredBytes => "transferred-bytes",
            Self::RegistryFootprint => "registry-footprint",
            Self::SavedDataFootprint => "saved-data-footprint",
            Self::TickCount => "tick-count",
        }
    }

    /// What the metric is counted in.
    #[must_use]
    pub const fn unit(self) -> &'static str {
        match self {
            Self::RetainedMemory
            | Self::AllocatedMemory
            | Self::TransferredBytes
            | Self::SavedDataFootprint => "bytes",
            Self::CpuTime | Self::InclusiveTime => "nanoseconds",
            Self::HookCalls
            | Self::EventCalls
            | Self::SlowCallbacks
            | Self::BoundaryCalls
            | Self::RegistryFootprint
            | Self::TickCount => "count",
        }
    }

    /// What M3's collectors can say about this metric.
    ///
    /// This is the table `ROADMAP.md`'s M3-W6 row means by "unsupported metrics
    /// are explicit". It is one table rather than prose in three places: the
    /// evidence record, the in-game report, and the tests all read it, so a
    /// metric cannot become supported in one of them and not the others.
    #[must_use]
    pub const fn support(self) -> Support {
        match self {
            Self::RetainedMemory => Support::Unsupported(
                "Rust attributes no allocation to a mod. Every mod is compiled into the host \
                 process under ADG-2, so its allocations come from the same global allocator as \
                 Ravel's and carry no identity; separating them needs a `GlobalAlloc` shim, and \
                 implementing that trait is unsafe, which every maintained crate here forbids.",
            ),
            Self::AllocatedMemory => Support::Unsupported(
                "The same reason retained memory is unsupported. A running total would need the \
                 same per-mod allocator hook, and counting only the allocations Ravel's own API \
                 makes on a mod's behalf would report a number far below the truth under a name \
                 that promises the whole of it.",
            ),
            Self::CpuTime => Support::Unsupported(
                "A per-thread processor clock is an operating-system call this workspace cannot \
                 reach: it needs libc, and no maintained crate holds unsafe code. \
                 `inclusive-time` is measured instead and is reported under its own name, because \
                 wall-clock time inside a callback is not an imprecise CPU time but a different \
                 quantity that scheduling, garbage collection, and blocking make arbitrarily \
                 larger.",
            ),
            Self::InclusiveTime => Support::Approximate(
                "The clock reading is exact and the attribution is not. The window is entry to \
                 return on the calling thread, so it charges a mod for host work it calls back \
                 into, for time the scheduler gives to something else, and for any JVM pause that \
                 lands inside it; and it charges a mod nothing for work it defers to another \
                 thread or a later tick.",
            ),
            Self::EventCalls | Self::RegistryFootprint => Support::Exact,
            Self::HookCalls => Support::Approximate(
                "Counted per mod rather than per crossing. One game-hook invocation fans out to \
                 every mod in the audience and is counted once for each of them, so this column \
                 sums to more than the number of crossings the process actually made.",
            ),
            Self::SavedDataFootprint => Support::Unsupported(
                "The loader records which saved-data versions a mod declares, not how many bytes a \
                 world holds for it. Those bytes exist — M3-W4 moved Ravel's own aggregate through \
                 an official world save and reload — but they are `ravel-save`'s aggregate, \
                 written at the ADG-7 persistence seam once per world save, and this crate depends \
                 on `ravel-core` alone and sees only what crosses a profiled callback. No stored \
                 byte ever reaches the collector path, and reporting the declared version count \
                 under the word `footprint` would answer a different question.",
            ),
            Self::SlowCallbacks => Support::Approximate(
                "A count of exact comparisons against an inexact number. The threshold is \
                 compared against `inclusive-time`, so a callback that waited rather than worked \
                 is counted as slow.",
            ),
            Self::BoundaryCalls => Support::Approximate(
                "Equal to `hook-calls` and inexact the same way. Every call that reaches a mod \
                 from the game at M3 arrives through a rewritten call site and Ravel makes none in \
                 the other direction, so the only crossings are hooks — but a crossing shared by an \
                 audience is charged to each of them.",
            ),
            Self::TransferredBytes => Support::Unsupported(
                "A hook carries primitives and borrowed handles rather than a buffer. A primitive \
                 has a width but is not transferred — the call site passes it in registers — and a \
                 handle refers to an object the JVM owns whose size Ravel cannot ask for without \
                 the call into Minecraft no accepted ADR provides. Summing argument widths would \
                 be a number about the descriptor, not about traffic.",
            ),
            Self::TickCount => Support::Unsupported(
                "Nothing marks a tick boundary. `Profiler::tick` increments a counter and no call \
                 site on the game's tick path calls it at M3, so the number is structurally zero \
                 rather than measured, and a zero drawn beside per-mod times that are real states \
                 a measurement where the truth is that there was none. Wiring a boundary is a \
                 change to what the collectors do per tick, which is the quantity \
                 `evidence/m3/profiling-overhead.json` measures and bounds, so it is a measured \
                 change rather than a display fix and it is not made here.",
            ),
        }
    }
}

/// What a game hook asked one mod to do.
///
/// Both arms are downstream of a rewritten call site: at M3 every delivery to a
/// mod begins as a hook, and a command is the one that begins as a hook carrying
/// something a player typed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallKind {
    /// A game event, shown to every mod in the audience.
    Event,
    /// A command, routed to the one mod that claimed it.
    Command,
}

/// What one mod cost, as far as M3's collectors can tell.
///
/// Every field is a metric [`Metric::support`] reports as measured. The
/// unsupported ones have no field, which is deliberate: an absent number cannot
/// be read as a zero.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ModCost {
    /// Game events delivered to this mod.
    pub event_calls: u64,
    /// Commands routed to this mod.
    pub command_calls: u64,
    /// Wall-clock nanoseconds spent inside this mod's callbacks.
    pub inclusive_nanos: u64,
    /// Callbacks that ran longer than the slow-callback threshold.
    pub slow_callbacks: u64,
    /// Registry identifiers this mod owns.
    pub registry_claims: u64,
}

impl ModCost {
    /// Game-hook invocations that reached this mod.
    ///
    /// Every delivery begins as one, so this is the two counts added.
    /// [`Metric::HookCalls`] records why the column sums to more than the
    /// crossings the process made.
    #[must_use]
    pub const fn hook_calls(&self) -> u64 {
        self.event_calls.saturating_add(self.command_calls)
    }

    /// Calls that reached this mod across the Java boundary.
    ///
    /// [`Metric::BoundaryCalls`] records why this is the hook count rather than
    /// a counter of its own.
    #[must_use]
    pub const fn boundary_calls(&self) -> u64 {
        self.hook_calls()
    }
}

/// The opt-in profiling coordinator.
///
/// Off is the state normal play runs in, and off means the collectors read no
/// clock and touch no map: [`Self::measure`] tests one flag and calls the body.
/// `evidence/m3/profiling-overhead.json` is what that costs, measured, and the
/// budget it is held to.
///
/// The coordinator is owned by whoever drives mods and is passed around as
/// `&mut`, because a shared profiler would need a lock on the tick path and a
/// lock is the thing a normal-mode budget cannot afford.
///
/// # Examples
///
/// ```
/// use ravel_observability::{CallKind, Profiler};
///
/// let mut profiler = Profiler::on(std::time::Duration::from_millis(1));
/// let mod_id = "example_mod".parse()?;
/// let doubled = profiler.measure(&mod_id, CallKind::Event, || 21 * 2);
///
/// assert_eq!(doubled, 42);
/// assert_eq!(profiler.report().mods()[0].cost.event_calls, 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Profiler {
    on: bool,
    slow_callback: Duration,
    ticks: u64,
    costs: BTreeMap<ModId, ModCost>,
}

impl Profiler {
    /// The profiler normal play gets: collecting nothing.
    #[must_use]
    pub const fn off() -> Self {
        Self {
            on: false,
            slow_callback: DEFAULT_SLOW_CALLBACK,
            ticks: 0,
            costs: BTreeMap::new(),
        }
    }

    /// A collecting profiler, with the threshold that makes a callback slow.
    #[must_use]
    pub const fn on(slow_callback: Duration) -> Self {
        Self {
            on: true,
            slow_callback,
            ticks: 0,
            costs: BTreeMap::new(),
        }
    }

    /// The profiler an opt-in switch asks for.
    ///
    /// The switch is the host's to read. `ravel-observability` is
    /// version-neutral and knows nothing about JVM system properties or command
    /// lines, so it takes the answer rather than the question.
    #[must_use]
    pub const fn from_switch(requested: bool) -> Self {
        if requested {
            Self::on(DEFAULT_SLOW_CALLBACK)
        } else {
            Self::off()
        }
    }

    /// Whether the collectors are running.
    #[must_use]
    pub const fn is_on(&self) -> bool {
        self.on
    }

    /// Runs one mod call, attributing it only when profiling is on.
    ///
    /// Off, this is one flag test and the call itself: no clock is read, no map
    /// is touched, and nothing is allocated. That is the whole normal-mode cost
    /// and it is what the budget bounds.
    pub fn measure<T>(&mut self, mod_id: &ModId, kind: CallKind, body: impl FnOnce() -> T) -> T {
        if !self.on {
            return body();
        }
        let started = Instant::now();
        let value = body();
        let elapsed = started.elapsed();
        self.charge(mod_id, kind, elapsed);
        value
    }

    /// Marks a tick boundary, which is what per-tick figures are divided by.
    pub const fn tick(&mut self) {
        if self.on {
            self.ticks = self.ticks.saturating_add(1);
        }
    }

    /// Records how many registry identifiers a mod owns.
    ///
    /// Told rather than counted, because the claims live in the loader's own
    /// table and counting them there is exact whether or not anything is
    /// profiling. That is why this replaces rather than accumulates: it is a
    /// reading of a table, not a running total of events.
    pub fn registry_claims(&mut self, mod_id: &ModId, claims: u64) {
        if self.on {
            self.cost_of(mod_id).registry_claims = claims;
        }
    }

    /// Renders everything the collectors hold.
    #[must_use]
    pub fn report(&self) -> ProfilingReport {
        let mut mods = self
            .costs
            .iter()
            .map(|(mod_id, cost)| ModReport {
                mod_id: mod_id.clone(),
                cost: *cost,
            })
            .collect::<Vec<_>>();
        // Costliest first, because that is the question a player opens the
        // screen with. Ties fall back to the id so two runs of the same set
        // render in the same order.
        mods.sort_by(|left, right| {
            right
                .cost
                .inclusive_nanos
                .cmp(&left.cost.inclusive_nanos)
                .then_with(|| left.mod_id.cmp(&right.mod_id))
        });

        ProfilingReport {
            on: self.on,
            ticks: self.ticks,
            slow_callback: self.slow_callback,
            mods,
        }
    }

    /// Adds one measured call to a mod's running cost.
    fn charge(&mut self, mod_id: &ModId, kind: CallKind, elapsed: Duration) {
        let slow = elapsed >= self.slow_callback;
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        let cost = self.cost_of(mod_id);
        match kind {
            CallKind::Event => cost.event_calls = cost.event_calls.saturating_add(1),
            CallKind::Command => cost.command_calls = cost.command_calls.saturating_add(1),
        }
        cost.inclusive_nanos = cost.inclusive_nanos.saturating_add(nanos);
        if slow {
            cost.slow_callbacks = cost.slow_callbacks.saturating_add(1);
        }
    }

    /// The running cost of one mod, started at zero the first time it is asked
    /// for.
    fn cost_of(&mut self, mod_id: &ModId) -> &mut ModCost {
        if !self.costs.contains_key(mod_id) {
            self.costs.insert(mod_id.clone(), ModCost::default());
        }
        self.costs
            .get_mut(mod_id)
            .expect("the entry was just inserted")
    }
}

impl Default for Profiler {
    /// Off, because off is what a host that never asked for profiling gets.
    fn default() -> Self {
        Self::off()
    }
}

/// What one mod cost, ready to be shown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModReport {
    /// The mod the numbers are about.
    pub mod_id: ModId,
    /// What it cost.
    pub cost: ModCost,
}

/// Everything opt-in profiling has to say about a run.
///
/// The report is data and rendered lines rather than a widget, for the reason
/// [`crate::PreWorldReport`] is: every screen Ravel owns is drawn in Java and
/// Ravel never renders from Rust. What a screen needs is [`Self::mods`] and
/// [`Self::unsupported`], and what a log needs is [`Self::lines`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfilingReport {
    on: bool,
    ticks: u64,
    slow_callback: Duration,
    mods: Vec<ModReport>,
}

impl ProfilingReport {
    /// Whether the collectors were running for this report.
    ///
    /// A report from a normal run says no and holds no mods, which is the
    /// honest answer rather than a page of zeroes.
    #[must_use]
    pub const fn is_on(&self) -> bool {
        self.on
    }

    /// Ticks the collectors saw.
    ///
    /// What the collectors saw is not what the run did: [`Metric::TickCount`]
    /// records that nothing calls [`Profiler::tick`] at M3, so this is zero on
    /// every real launch. It is here because a surface has to carry the figure
    /// once something produces it, and it is not what a surface decides to draw
    /// from — that decision is [`Metric::TickCount`]'s, so one table answers it
    /// for the log line below and for the screen alike.
    #[must_use]
    pub const fn ticks(&self) -> u64 {
        self.ticks
    }

    /// The threshold above which a callback was counted as slow.
    #[must_use]
    pub const fn slow_callback(&self) -> Duration {
        self.slow_callback
    }

    /// Every mod that was entered, costliest first.
    #[must_use]
    pub fn mods(&self) -> &[ModReport] {
        &self.mods
    }

    /// What a number in this report is and is not, one sentence per hazard.
    ///
    /// `ROADMAP.md`'s M3-W6 row asks for per-mod attribution accuracy to be
    /// explicit, and this is the statement in the form a surface can show. The
    /// per-metric half is [`Metric::support`].
    #[must_use]
    pub const fn attribution_accuracy() -> &'static [&'static str] {
        ATTRIBUTION_ACCURACY
    }

    /// Every metric a collector cannot measure at M3, and why.
    #[must_use]
    pub fn unsupported() -> Vec<(Metric, &'static str)> {
        Metric::ALL
            .into_iter()
            .filter_map(|metric| match metric.support() {
                Support::Unsupported(reason) => Some((metric, reason)),
                Support::Exact | Support::Approximate(_) => None,
            })
            .collect()
    }

    /// Renders the report, one line per entry.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        if !self.on {
            return vec![
                "profiling is off, so nothing was collected and no number below it would be zero"
                    .to_owned(),
            ];
        }

        // No tick count. `Metric::TickCount` is unsupported, so the only number
        // this could print is a zero that reads as a measurement, and it is
        // named on the unsupported list below instead of being drawn here.
        let mut lines = vec![format!(
            "profiled {} mod{}, slow above {} us",
            self.mods.len(),
            plural(self.mods.len() as u64),
            self.slow_callback.as_micros()
        )];
        lines.extend(self.mods.iter().map(|entry| {
            format!(
                "`{}` {} ns inclusive over {} hook calls ({} events, {} commands), \
                 {} slow, {} registry claims",
                entry.mod_id,
                entry.cost.inclusive_nanos,
                entry.cost.hook_calls(),
                entry.cost.event_calls,
                entry.cost.command_calls,
                entry.cost.slow_callbacks,
                entry.cost.registry_claims,
            )
        }));
        lines.extend(
            Self::unsupported()
                .into_iter()
                .map(|(metric, reason)| format!("`{}` is unsupported: {reason}", metric.name())),
        );
        lines
    }
}

/// What a per-mod number here cannot tell apart.
///
/// `SPEC.md` §9.3 requires proving what can be attributed accurately rather
/// than presenting estimates as exact. These are the cases where a number is
/// charged to a mod that did not spend it, or not charged to one that did.
const ATTRIBUTION_ACCURACY: &[&str] = &[
    "A mod is charged for host work it calls back into. The window is entry to return, so a mod \
     that asks Ravel to resolve a registry key is charged for the lookup as well as for its own \
     code.",
    "A mod is charged for time it did not run. A scheduler preemption, a JVM safepoint, or a \
     garbage-collection pause that lands inside the window is inside the number, and none of them \
     is the mod's doing.",
    "A mod is not charged for work it defers. Anything it schedules onto another thread or onto a \
     later tick is charged to whatever runs it, and if nothing Ravel drives runs it, to nobody.",
    "A mod that calls another mod's service is charged for it. The inner call is inside the outer \
     window, so the same nanoseconds appear once in the caller's total and once in the callee's, \
     and the column does not sum to the run.",
    "Nothing here is memory. No number in this report is bytes a mod allocated or still holds, and \
     the two memory metrics say so rather than reading as zero.",
    "The numbers are from a profiled run and not from a normal one. Reading a clock twice per \
     callback costs something, so a mod's measured time includes being measured.",
];

/// The plural suffix for a count.
const fn plural(count: u64) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use ravel_core::ModId;

    use super::{
        ATTRIBUTION_ACCURACY, CallKind, DEFAULT_SLOW_CALLBACK, Metric, ModCost, Profiler,
        ProfilingReport, Support,
    };

    fn id(text: &str) -> ModId {
        text.parse().expect("a valid mod id")
    }

    #[test]
    fn a_profiler_that_was_never_asked_for_collects_nothing() {
        let mut profiler = Profiler::default();
        assert!(!profiler.is_on());

        let mut entered = 0;
        profiler.measure(&id("example_mod"), CallKind::Event, || entered += 1);
        profiler.tick();
        profiler.registry_claims(&id("example_mod"), 3);

        let report = profiler.report();
        assert_eq!(entered, 1, "the body still runs with profiling off");
        assert!(!report.is_on());
        assert_eq!(report.ticks(), 0);
        assert!(report.mods().is_empty());
    }

    #[test]
    fn an_off_report_says_so_rather_than_showing_a_page_of_zeroes() {
        let report = Profiler::off().report();
        assert_eq!(
            report.lines(),
            ["profiling is off, so nothing was collected and no number below it would be zero"]
        );
    }

    #[test]
    fn the_switch_decides_whether_anything_is_collected() {
        assert!(!Profiler::from_switch(false).is_on());
        let on = Profiler::from_switch(true);
        assert!(on.is_on());
        assert_eq!(on.report().slow_callback(), DEFAULT_SLOW_CALLBACK);
    }

    #[test]
    fn a_measured_call_returns_its_own_value_and_is_counted_against_its_mod() {
        let mut profiler = Profiler::on(Duration::from_secs(1));
        let doubled = profiler.measure(&id("example_mod"), CallKind::Event, || 21 * 2);
        profiler.measure(&id("example_mod"), CallKind::Event, || ());

        assert_eq!(doubled, 42);
        let report = profiler.report();
        assert_eq!(report.mods().len(), 1);
        assert_eq!(report.mods()[0].mod_id, id("example_mod"));
        assert_eq!(report.mods()[0].cost.event_calls, 2);
        assert_eq!(report.mods()[0].cost.command_calls, 0);
        assert_eq!(report.mods()[0].cost.hook_calls(), 2);
        assert_eq!(report.mods()[0].cost.boundary_calls(), 2);
        assert_eq!(report.mods()[0].cost.slow_callbacks, 0);
    }

    #[test]
    fn a_callback_over_the_threshold_is_counted_as_slow() {
        let mut profiler = Profiler::on(Duration::from_millis(1));
        profiler.measure(&id("slow_mod"), CallKind::Event, || {
            thread::sleep(Duration::from_millis(3));
        });

        let report = profiler.report();
        assert_eq!(report.mods()[0].cost.slow_callbacks, 1);
        assert!(report.mods()[0].cost.inclusive_nanos >= 1_000_000);
        assert_eq!(report.slow_callback(), Duration::from_millis(1));
    }

    #[test]
    fn ticks_and_registry_claims_are_collected_only_when_on() {
        let mut profiler = Profiler::on(DEFAULT_SLOW_CALLBACK);
        profiler.tick();
        profiler.tick();
        profiler.registry_claims(&id("example_mod"), 9);
        profiler.registry_claims(&id("example_mod"), 4);

        let report = profiler.report();
        assert_eq!(report.ticks(), 2);
        assert_eq!(
            report.mods()[0].cost.registry_claims,
            4,
            "a claim count is a reading of a table, not a running total"
        );
    }

    #[test]
    fn a_command_and_an_event_are_counted_apart_and_added_for_the_hook_count() {
        let mut profiler = Profiler::on(Duration::from_secs(1));
        profiler.measure(&id("example_mod"), CallKind::Event, || ());
        profiler.measure(&id("example_mod"), CallKind::Command, || ());
        profiler.measure(&id("example_mod"), CallKind::Command, || ());

        let cost = profiler.report().mods()[0].cost;
        assert_eq!(cost.event_calls, 1);
        assert_eq!(cost.command_calls, 2);
        assert_eq!(cost.hook_calls(), 3);
    }

    #[test]
    fn mods_are_reported_costliest_first_and_ties_break_on_identity() {
        let mut profiler = Profiler::on(Duration::from_secs(1));
        profiler.measure(&id("cheap_mod"), CallKind::Event, || ());
        profiler.measure(&id("costly_mod"), CallKind::Event, || {
            thread::sleep(Duration::from_millis(2));
        });
        profiler.registry_claims(&id("aardvark_mod"), 1);

        let report = profiler.report();
        let order = report
            .mods()
            .iter()
            .map(|entry| entry.mod_id.to_string())
            .collect::<Vec<_>>();
        assert_eq!(order[0], "costly_mod");
        assert!(order.contains(&"aardvark_mod".to_owned()));
        assert_eq!(report.mods().len(), 3);
    }

    #[test]
    fn every_spec_metric_states_what_m3_can_say_about_it() {
        assert_eq!(Metric::ALL.len(), 12);
        for metric in Metric::ALL {
            assert!(!metric.name().is_empty());
            assert!(matches!(metric.unit(), "bytes" | "nanoseconds" | "count"));
            match metric.support() {
                Support::Exact => assert!(metric.support().detail().is_none()),
                Support::Approximate(detail) | Support::Unsupported(detail) => {
                    assert!(
                        detail.len() > 40,
                        "{} explains itself briefly",
                        metric.name()
                    );
                }
            }
        }
    }

    #[test]
    fn the_metrics_m3_cannot_measure_are_named_rather_than_absent() {
        let unsupported = ProfilingReport::unsupported();
        let names = unsupported
            .iter()
            .map(|(metric, _)| metric.name())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "retained-memory",
                "allocated-memory",
                "cpu-time",
                "transferred-bytes",
                "saved-data-footprint",
                "tick-count"
            ]
        );
        for (metric, _) in &unsupported {
            assert!(!metric.support().is_measured());
        }
    }

    #[test]
    fn a_cost_that_was_never_charged_is_zero_across_every_measured_metric() {
        let cost = ModCost::default();
        assert_eq!(cost.hook_calls(), 0);
        assert_eq!(cost.boundary_calls(), 0);
        assert_eq!(cost.inclusive_nanos, 0);

        let measured = Metric::ALL
            .into_iter()
            .filter(|metric| metric.support().is_measured())
            .map(Metric::name)
            .collect::<Vec<_>>();
        assert_eq!(
            measured,
            [
                "inclusive-time",
                "hook-calls",
                "event-calls",
                "slow-callbacks",
                "boundary-calls",
                "registry-footprint"
            ]
        );
    }

    #[test]
    fn the_attribution_statement_names_each_way_a_number_misattributes() {
        assert_eq!(
            ProfilingReport::attribution_accuracy(),
            ATTRIBUTION_ACCURACY
        );
        assert_eq!(ATTRIBUTION_ACCURACY.len(), 6);
        for sentence in ATTRIBUTION_ACCURACY {
            assert!(sentence.len() > 60);
        }
    }

    #[test]
    fn an_on_report_renders_its_mods_and_repeats_what_it_cannot_measure() {
        let mut profiler = Profiler::on(DEFAULT_SLOW_CALLBACK);
        profiler.tick();
        profiler.measure(&id("example_mod"), CallKind::Event, || ());

        let lines = profiler.report().lines();
        assert!(
            lines[0].starts_with("profiled 1 mod, slow above "),
            "{lines:?}"
        );
        assert!(lines[1].starts_with("`example_mod` "), "{lines:?}");
        assert_eq!(
            lines
                .iter()
                .filter(|line| line.contains("unsupported"))
                .count(),
            6
        );
    }

    #[test]
    fn a_tick_count_nothing_produces_is_named_rather_than_drawn_as_a_zero() {
        let mut profiler = Profiler::on(DEFAULT_SLOW_CALLBACK);
        profiler.tick();
        profiler.measure(&id("example_mod"), CallKind::Event, || ());

        let lines = profiler.report().lines();
        assert!(
            !lines[0].contains("tick"),
            "the summary states a tick count no call site produces: {lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("`tick-count` is unsupported")),
            "the count vanished instead of being named as unmeasurable: {lines:?}"
        );
        assert!(!Metric::TickCount.support().is_measured());
    }

    #[test]
    fn a_report_is_comparable_so_two_runs_can_be_shown_to_say_the_same_thing() {
        let first = Profiler::on(DEFAULT_SLOW_CALLBACK).report();
        assert_eq!(first.clone(), first);
        assert_eq!(Profiler::off(), Profiler::default());
    }
}
