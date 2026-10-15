//! The client presentation surface, from §10.
//!
//! A decoration dependent is the main consumer. What is implemented is the part
//! with behaviour of its own:
//! the scissor stack's intersection rule, the pose scope, the cursor resolution,
//! the screen history's priority rule, the memory that restores a screen, and
//! the system-information builder the one client command formats.
//!
//! **Host-blocked.** Ravel-owned screens are declared in data and drawn in Java,
//! so the widget catalogue and the cursor shape catalogue are declared rather
//! than implemented. The widget layout arithmetic and the selection list's
//! scrolling are not specified either, and a parity test over them would be
//! testing pixels.
//!
//! # The change declared here
//!
//! **Screen state is cleared on disconnect.** The original's map from screen
//! identifier to state object is never cleared, including on disconnect, so
//! reopening a screen after joining a different world restores what it looked
//! like in the previous one. Carrying one world's screen state into another is a
//! bug rather than a feature.

use std::collections::BTreeMap;

use ravel_sdk::JsonValue;

/// A rectangle, in the coordinates a clip is expressed in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect {
    /// The left edge.
    pub left: i32,
    /// The top edge.
    pub top: i32,
    /// The right edge, exclusive.
    pub right: i32,
    /// The bottom edge, exclusive.
    pub bottom: i32,
}

impl Rect {
    /// Builds a rectangle from its edges.
    #[must_use]
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// Returns whether the rectangle encloses anything.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }

    /// Returns the overlap of two rectangles.
    ///
    /// An overlap that encloses nothing is still a rectangle: a clip that
    /// excludes everything is what nesting two disjoint clips means, and
    /// answering nothing would let a caller draw outside both.
    #[must_use]
    pub fn intersect(self, other: Self) -> Self {
        Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        }
    }

    /// Returns whether a point is inside.
    #[must_use]
    pub const fn holds(self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

/// How deep clips and poses may nest.
///
/// A stack that a drawing loop pushes to without popping is a leak, and a
/// ceiling turns it into a refusal that names the depth.
pub const MAX_DEPTH: usize = 256;

/// Why a stack refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StackError {
    /// The stack is already [`MAX_DEPTH`] deep.
    TooDeep(usize),
    /// A pop was asked for with nothing pushed.
    Empty,
}

impl std::fmt::Display for StackError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooDeep(ceiling) => write!(formatter, "a stack is already {ceiling} deep"),
            Self::Empty => write!(formatter, "a stack was popped with nothing on it"),
        }
    }
}

impl std::error::Error for StackError {}

/// Nested clip rectangles.
///
/// Each new clip is intersected with the one below, and popping restores the
/// previous. The original ties a clip to a lexical scope with a closeable
/// wrapper; in Rust that scoping is what a guard type or a plain block does, and
/// it is language-specific rather than behaviour.
#[derive(Clone, Debug, Default)]
pub struct ScissorStack {
    clips: Vec<Rect>,
}

impl ScissorStack {
    /// Builds an empty stack, which clips nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes a clip, intersected with the one below.
    ///
    /// # Errors
    ///
    /// Returns [`StackError::TooDeep`] at [`MAX_DEPTH`].
    pub fn push(&mut self, clip: Rect) -> Result<Rect, StackError> {
        if self.clips.len() >= MAX_DEPTH {
            return Err(StackError::TooDeep(MAX_DEPTH));
        }
        let effective = self
            .clips
            .last()
            .map_or(clip, |below| below.intersect(clip));
        self.clips.push(effective);
        Ok(effective)
    }

    /// Pops the top clip, restoring the previous.
    ///
    /// # Errors
    ///
    /// Returns [`StackError::Empty`] when nothing is pushed.
    pub fn pop(&mut self) -> Result<Rect, StackError> {
        self.clips.pop().ok_or(StackError::Empty)
    }

    /// Returns the clip in force.
    #[must_use]
    pub fn current(&self) -> Option<Rect> {
        self.clips.last().copied()
    }

    /// Returns how deep the stack is.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.clips.len()
    }
}

/// One transform on the pose stack.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Pose {
    /// How far the transform moves along each axis.
    pub offset: (f32, f32, f32),
    /// How much the transform scales.
    pub scale: f32,
}

/// The transform stack, which is the scissor stack's shape for poses.
#[derive(Clone, Debug, Default)]
pub struct PoseStack {
    poses: Vec<Pose>,
}

impl PoseStack {
    /// Builds an empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes a transform, composed with the one below.
    ///
    /// # Errors
    ///
    /// Returns [`StackError::TooDeep`] at [`MAX_DEPTH`].
    pub fn push(&mut self, pose: Pose) -> Result<Pose, StackError> {
        if self.poses.len() >= MAX_DEPTH {
            return Err(StackError::TooDeep(MAX_DEPTH));
        }
        let composed = self.poses.last().map_or(pose, |below| {
            let offset = (
                pose.offset.0 * below.scale,
                pose.offset.1 * below.scale,
                pose.offset.2 * below.scale,
            );
            Pose {
                offset: (
                    below.offset.0 + offset.0,
                    below.offset.1 + offset.1,
                    below.offset.2 + offset.2,
                ),
                scale: below.scale * pose.scale,
            }
        });
        self.poses.push(composed);
        Ok(composed)
    }

    /// Pops the top transform.
    ///
    /// # Errors
    ///
    /// Returns [`StackError::Empty`] when nothing is pushed.
    pub fn pop(&mut self) -> Result<Pose, StackError> {
        self.poses.pop().ok_or(StackError::Empty)
    }

    /// Returns the transform in force.
    #[must_use]
    pub fn current(&self) -> Option<Pose> {
        self.poses.last().copied()
    }
}

/// Which pointer shape applies over a region.
///
/// **Host-blocked.** The shape catalogue is the host's, so a shape is a name a
/// dependent supplies and this package decides only which region the pointer is
/// in.
#[derive(Clone, Debug, Default)]
pub struct Cursors {
    regions: Vec<(Rect, String)>,
}

/// The most cursor regions one screen will declare.
pub const MAX_REGIONS: usize = 1024;

impl Cursors {
    /// Builds a screen with no declared region.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares that a shape applies over a region.
    ///
    /// # Errors
    ///
    /// Returns [`StackError::TooDeep`] at [`MAX_REGIONS`].
    pub fn declare(&mut self, region: Rect, shape: &str) -> Result<(), StackError> {
        if self.regions.len() >= MAX_REGIONS {
            return Err(StackError::TooDeep(MAX_REGIONS));
        }
        self.regions.push((region, shape.to_owned()));
        Ok(())
    }

    /// Resolves the shape under a pointer, as a frame does.
    ///
    /// The last region declared over a point wins, so a dependent that declares
    /// a region inside another gets the inner one by declaring it after.
    #[must_use]
    pub fn at(&self, x: i32, y: i32) -> Option<&str> {
        self.regions
            .iter()
            .rev()
            .find(|(region, _)| region.holds(x, y))
            .map(|(_, shape)| shape.as_str())
    }

    /// Returns how many regions are declared.
    #[must_use]
    pub fn len(&self) -> usize {
        self.regions.len()
    }

    /// Returns whether nothing is declared.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }
}

/// Where a screen came from, and which of two screens is shown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct History {
    /// The screen to go back to, when there is one.
    pub previous: Option<String>,
    /// The priority that decides between two competing screens.
    pub priority: i32,
}

impl History {
    /// A screen with nowhere to go back to.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            previous: None,
            priority: 0,
        }
    }

    /// A screen that came from another.
    #[must_use]
    pub fn after(previous: &str, priority: i32) -> Self {
        Self {
            previous: Some(previous.to_owned()),
            priority,
        }
    }

    /// Returns whether this screen is shown rather than `other`.
    ///
    /// The higher priority wins. A tie keeps what is already shown, which is
    /// what makes the rule a rule rather than a coin toss.
    #[must_use]
    pub const fn wins_over(&self, other: &Self) -> bool {
        self.priority > other.priority
    }

    /// Returns whether there is a way back.
    #[must_use]
    pub const fn has_way_back(&self) -> bool {
        self.previous.is_some()
    }
}

/// The most screens the memory will hold.
pub const MAX_REMEMBERED: usize = 512;

/// A memory that restores a screen.
///
/// A map from screen identifier to a state object, so reopening a screen
/// restores what it looked like. The accessor takes the expected state kind and
/// replaces a state of the wrong kind rather than failing.
#[derive(Clone, Debug, Default)]
pub struct ScreenMemory {
    states: BTreeMap<String, (String, JsonValue)>,
}

impl ScreenMemory {
    /// Builds an empty memory.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the state held for a screen under the expected kind.
    ///
    /// A state of another kind is replaced with `fresh` rather than refused,
    /// which is what a dependent that changed its own state shape relies on.
    ///
    /// # Panics
    ///
    /// Panics if the screen state is absent after it was held or inserted.
    pub fn state(&mut self, screen: &str, kind: &str, fresh: JsonValue) -> &JsonValue {
        let replace = match self.states.get(screen) {
            Some((held, _)) => held != kind,
            None => true,
        };
        if replace {
            self.make_room_for(screen);
            self.states
                .insert(screen.to_owned(), (kind.to_owned(), fresh));
        }
        &self
            .states
            .get(screen)
            .expect("the state was just held or just inserted")
            .1
    }

    /// Replaces the state held for a screen.
    pub fn remember(&mut self, screen: &str, kind: &str, state: JsonValue) {
        self.make_room_for(screen);
        self.states
            .insert(screen.to_owned(), (kind.to_owned(), state));
    }

    /// Frees one place for a screen not already held.
    ///
    /// At [`MAX_REMEMBERED`] one screen is forgotten, not all of them: this
    /// memory exists so a reopened screen looks as it did, and emptying it to
    /// make room for one would lose every other screen's state, which is the
    /// loss the disconnect rule above exists to prevent. Which one goes is the
    /// first in identifier order, because nothing here records when a screen was
    /// last seen; the upgrade path is a least-recently-used rule.
    fn make_room_for(&mut self, screen: &str) {
        while self.states.len() >= MAX_REMEMBERED && !self.states.contains_key(screen) {
            self.states.pop_first();
        }
    }

    /// Returns whether anything is held for a screen.
    #[must_use]
    pub fn holds(&self, screen: &str) -> bool {
        self.states.contains_key(screen)
    }

    /// Returns how many screens are remembered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// Returns whether nothing is remembered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Forgets everything, which is what a disconnect does.
    ///
    /// **Declared change.** The original never clears this map, so state
    /// survives joining a different world.
    pub fn on_disconnect(&mut self) {
        self.states.clear();
    }
}

/// The categories the system-information builder collects facts under.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub enum Category {
    /// What the machine runs.
    OperatingSystem,
    /// What the game runs on.
    Runtime,
    /// The game itself.
    Game,
    /// What started the game.
    Launcher,
}

impl Category {
    /// The four built-in categories, in report order.
    pub const BUILT_IN: [Self; 4] = [
        Self::OperatingSystem,
        Self::Runtime,
        Self::Game,
        Self::Launcher,
    ];

    /// Returns the heading this category is reported under.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OperatingSystem => "operating system",
            Self::Runtime => "runtime",
            Self::Game => "game",
            Self::Launcher => "launcher",
        }
    }
}

/// The most facts one report will carry.
pub const MAX_FACTS: usize = 512;

/// Categorised facts about the running machine.
///
/// A dependent may add its own facts, which is the registration point §10
/// names. Everything here is something the game already knows about its own
/// machine, and nothing here reaches a network.
#[derive(Clone, Debug, Default)]
pub struct SystemFacts {
    facts: Vec<(Category, String, String)>,
    extra: Vec<(String, String, String)>,
}

impl SystemFacts {
    /// Builds an empty report.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a fact under one of the four built-in categories.
    ///
    /// # Errors
    ///
    /// Returns [`StackError::TooDeep`] at [`MAX_FACTS`].
    pub fn add(&mut self, category: Category, name: &str, value: &str) -> Result<(), StackError> {
        if self.count() >= MAX_FACTS {
            return Err(StackError::TooDeep(MAX_FACTS));
        }
        self.facts
            .push((category, name.to_owned(), value.to_owned()));
        Ok(())
    }

    /// Adds a fact under a dependent's own category.
    ///
    /// # Errors
    ///
    /// Returns [`StackError::TooDeep`] at [`MAX_FACTS`].
    pub fn add_own(&mut self, category: &str, name: &str, value: &str) -> Result<(), StackError> {
        if self.count() >= MAX_FACTS {
            return Err(StackError::TooDeep(MAX_FACTS));
        }
        self.extra
            .push((category.to_owned(), name.to_owned(), value.to_owned()));
        Ok(())
    }

    /// Returns how many facts are held.
    #[must_use]
    pub fn count(&self) -> usize {
        self.facts.len() + self.extra.len()
    }

    /// Returns the facts held under one built-in category.
    #[must_use]
    pub fn under(&self, category: Category) -> Vec<(&str, &str)> {
        self.facts
            .iter()
            .filter(|(held, _, _)| *held == category)
            .map(|(_, name, value)| (name.as_str(), value.as_str()))
            .collect()
    }

    /// Formats the report the one client command produces.
    ///
    /// The four built-in categories come first, in order, then a dependent's
    /// own. A category with no fact is omitted rather than printed empty.
    ///
    /// **Host-blocked.** The original puts this on the clipboard for a user to
    /// paste into a support channel. Ravel publishes no clipboard, so this
    /// package produces the text and the placing is the host's. What is kept
    /// either way is that the command is explicitly user-invoked, reads only
    /// what the game already knows about its own machine, and sends nothing.
    #[must_use]
    pub fn report(&self) -> String {
        let mut lines = Vec::new();
        for category in Category::BUILT_IN {
            let facts = self.under(category);
            if facts.is_empty() {
                continue;
            }
            lines.push(format!("[{}]", category.as_str()));
            for (name, value) in facts {
                lines.push(format!("{name}: {value}"));
            }
        }
        let mut own: Vec<&str> = Vec::new();
        for (category, _, _) in &self.extra {
            // First-appearance order, and named once however far apart two facts
            // under one category were added. Sorting would print a dependent's
            // categories in an order it did not choose.
            if !own.contains(&category.as_str()) {
                own.push(category.as_str());
            }
        }
        for category in own {
            lines.push(format!("[{category}]"));
            for (held, name, value) in &self.extra {
                if held == category {
                    lines.push(format!("{name}: {value}"));
                }
            }
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::json;

    use super::{
        Category, Cursors, History, MAX_FACTS, MAX_REMEMBERED, Pose, PoseStack, Rect, ScissorStack,
        ScreenMemory, StackError, SystemFacts,
    };

    #[test]
    fn a_nested_clip_is_the_overlap_with_the_one_below() {
        let mut stack = ScissorStack::new();
        let outer = stack
            .push(Rect::new(0, 0, 100, 100))
            .expect("the first clip");
        assert_eq!(outer, Rect::new(0, 0, 100, 100));
        let inner = stack
            .push(Rect::new(50, 50, 200, 200))
            .expect("the second clip");
        assert_eq!(
            inner,
            Rect::new(50, 50, 100, 100),
            "the new clip never reaches outside the one below"
        );
        assert_eq!(stack.current(), Some(inner));
    }

    #[test]
    fn popping_restores_the_previous_clip() {
        let mut stack = ScissorStack::new();
        stack.push(Rect::new(0, 0, 100, 100)).expect("a clip");
        stack.push(Rect::new(10, 10, 20, 20)).expect("a clip");
        assert_eq!(stack.depth(), 2);
        stack.pop().expect("two are pushed");
        assert_eq!(stack.current(), Some(Rect::new(0, 0, 100, 100)));
        stack.pop().expect("one is pushed");
        assert_eq!(stack.current(), None);
        assert_eq!(stack.pop(), Err(StackError::Empty));
    }

    #[test]
    fn two_disjoint_clips_nest_to_a_clip_that_encloses_nothing() {
        let mut stack = ScissorStack::new();
        stack.push(Rect::new(0, 0, 10, 10)).expect("a clip");
        let overlap = stack.push(Rect::new(50, 50, 60, 60)).expect("a clip");
        assert!(
            overlap.is_empty(),
            "answering nothing would let a caller draw outside both"
        );
    }

    #[test]
    fn a_clip_stack_deeper_than_the_ceiling_is_refused() {
        let mut stack = ScissorStack::new();
        for _ in 0..super::MAX_DEPTH {
            stack
                .push(Rect::new(0, 0, 100, 100))
                .expect("inside the ceiling");
        }
        assert_eq!(
            stack.push(Rect::new(0, 0, 1, 1)),
            Err(StackError::TooDeep(super::MAX_DEPTH))
        );
    }

    #[test]
    fn the_pose_stack_composes_and_restores_the_same_way() {
        let mut stack = PoseStack::new();
        stack
            .push(Pose {
                offset: (10.0, 0.0, 0.0),
                scale: 2.0,
            })
            .expect("the first pose");
        let inner = stack
            .push(Pose {
                offset: (5.0, 0.0, 0.0),
                scale: 3.0,
            })
            .expect("the second pose");
        assert_eq!(inner.offset.0, 20.0);
        assert_eq!(inner.scale, 6.0);
        stack.pop().expect("two are pushed");
        assert_eq!(stack.current().map(|pose| pose.scale), Some(2.0));
        stack.pop().expect("one is pushed");
        assert_eq!(stack.pop(), Err(StackError::Empty));
    }

    #[test]
    fn a_cursor_shape_is_resolved_from_the_region_under_the_pointer() {
        let mut cursors = Cursors::new();
        cursors
            .declare(Rect::new(0, 0, 100, 100), "arrow")
            .expect("the first region");
        cursors
            .declare(Rect::new(10, 10, 20, 20), "hand")
            .expect("the second region");
        assert_eq!(cursors.at(5, 5), Some("arrow"));
        assert_eq!(cursors.at(15, 15), Some("hand"), "the later region wins");
        assert_eq!(cursors.at(500, 500), None);
        assert_eq!(cursors.len(), 2);
        assert_ne!(cursors.len(), 0);
    }

    #[test]
    fn a_priority_rule_decides_between_two_competing_screens() {
        let low = History::after("menu", 1);
        let high = History::after("menu", 5);
        assert!(high.wins_over(&low));
        assert!(!low.wins_over(&high));
        assert!(
            !high.wins_over(&History::after("other", 5)),
            "a tie keeps what is already shown"
        );
        assert!(high.has_way_back());
        assert!(!History::none().has_way_back());
    }

    #[test]
    fn reopening_a_screen_restores_what_it_looked_like() {
        let mut memory = ScreenMemory::new();
        memory.remember("catalogue", "scroll", json!({ "row": 12 }));
        assert_eq!(
            memory.state("catalogue", "scroll", json!({ "row": 0 })),
            &json!({ "row": 12 })
        );
        assert!(memory.holds("catalogue"));
    }

    #[test]
    fn a_state_of_the_wrong_kind_is_replaced_rather_than_refused() {
        let mut memory = ScreenMemory::new();
        memory.remember("catalogue", "scroll", json!({ "row": 12 }));
        assert_eq!(
            memory.state("catalogue", "grid", json!({ "page": 0 })),
            &json!({ "page": 0 }),
            "the accessor takes the expected kind and replaces a mismatch"
        );
        assert_eq!(
            memory.state("catalogue", "grid", json!({ "page": 9 })),
            &json!({ "page": 0 }),
            "and the replacement is then the state, so a second ask keeps it"
        );
    }

    #[test]
    fn an_unremembered_screen_takes_the_fresh_state() {
        let mut memory = ScreenMemory::new();
        assert_eq!(memory.len(), 0);
        assert_eq!(
            memory.state("new", "scroll", json!({ "row": 0 })),
            &json!({ "row": 0 })
        );
        assert_eq!(memory.len(), 1);
    }

    #[test]
    fn a_disconnect_clears_the_memory_so_no_state_crosses_worlds() {
        let mut memory = ScreenMemory::new();
        memory.remember("catalogue", "scroll", json!({ "row": 12 }));
        memory.on_disconnect();
        assert_eq!(
            memory.len(),
            0,
            "this is the declared change: the original never clears it"
        );
        assert_eq!(
            memory.state("catalogue", "scroll", json!({ "row": 0 })),
            &json!({ "row": 0 }),
            "the next world starts fresh"
        );
    }

    #[test]
    fn the_memory_is_bounded_rather_than_growing_for_the_life_of_the_process() {
        let mut memory = ScreenMemory::new();
        for index in 0..MAX_REMEMBERED {
            memory.remember(&format!("s{index}"), "scroll", json!({ "row": index }));
        }
        assert_eq!(memory.len(), MAX_REMEMBERED);
        memory.remember("one_more", "scroll", json!({ "row": 0 }));
        assert_eq!(
            memory.len(),
            MAX_REMEMBERED,
            "one screen is forgotten to make room, not every screen"
        );
        assert!(memory.holds("one_more"));
        assert!(!memory.holds("s0"), "the first in identifier order goes");
        assert!(
            memory.holds(&format!("s{}", MAX_REMEMBERED - 1)),
            "every other screen keeps the state it was remembered with"
        );
        memory.state("s1", "scroll", json!({ "row": 0 }));
        assert_eq!(
            memory.len(),
            MAX_REMEMBERED,
            "reading a held screen frees nothing and loses nothing"
        );
    }

    #[test]
    fn there_are_four_built_in_categories_and_a_registration_point_for_a_dependents_own() {
        assert_eq!(Category::BUILT_IN.len(), 4);
        let mut facts = SystemFacts::new();
        for category in Category::BUILT_IN {
            facts
                .add(category, "name", category.as_str())
                .expect("inside the ceiling");
        }
        facts
            .add_own("dependent", "version", "1.0")
            .expect("inside the ceiling");
        assert_eq!(facts.count(), 5);
        assert_eq!(facts.under(Category::Game), [("name", "game")]);
    }

    #[test]
    fn the_report_puts_the_built_in_categories_first_and_omits_the_empty_ones() {
        let mut facts = SystemFacts::new();
        facts
            .add(Category::Game, "version", "1.21.1")
            .expect("inside the ceiling");
        facts
            .add_own("dependent", "build", "7")
            .expect("inside the ceiling");
        let report = facts.report();
        assert_eq!(
            report, "[game]\nversion: 1.21.1\n[dependent]\nbuild: 7",
            "a category with no fact is omitted rather than printed empty"
        );
    }

    #[test]
    fn a_dependents_category_is_printed_once_however_far_apart_its_facts_were_added() {
        let mut facts = SystemFacts::new();
        for (category, name, value) in [("mine", "a", "1"), ("other", "b", "2"), ("mine", "c", "3")]
        {
            facts
                .add_own(category, name, value)
                .expect("inside the ceiling");
        }
        assert_eq!(
            facts.report(),
            "[mine]\na: 1\nc: 3\n[other]\nb: 2",
            "a heading printed twice would print its facts twice with it"
        );
    }

    #[test]
    fn an_empty_report_is_empty_rather_than_four_headings() {
        assert_eq!(SystemFacts::new().report(), "");
    }

    #[test]
    fn a_report_past_the_ceiling_is_refused() {
        let mut facts = SystemFacts::new();
        for index in 0..MAX_FACTS {
            facts
                .add(Category::Game, &format!("f{index}"), "x")
                .expect("inside the ceiling");
        }
        assert_eq!(
            facts.add(Category::Game, "one_more", "x"),
            Err(StackError::TooDeep(MAX_FACTS))
        );
        assert_eq!(
            facts.add_own("dependent", "one_more", "x"),
            Err(StackError::TooDeep(MAX_FACTS))
        );
    }
}
