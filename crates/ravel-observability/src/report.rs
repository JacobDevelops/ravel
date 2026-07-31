use std::fmt;

use ravel_core::{
    DiagnosticMessage, FailureAttribution, FailureEnvironment, FailureSurface, ModId,
    StructuredFailure, Version,
};

use crate::redaction::redact;

/// Who a pre-world refusal is about.
///
/// The identity and the version are optional for the same reason
/// [`FailureAttribution`] makes them optional: a manifest that will not parse
/// has neither, and inventing one to fill the field would put a mod id in a
/// report that no package ever claimed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Subject {
    /// The mod the failure is about, absent when nothing established one.
    pub mod_id: Option<ModId>,
    /// The version that mod declared, absent when nothing was accepted.
    pub version: Option<Version>,
    /// The dependency path that reached it, ordered from the mod that required
    /// it.
    pub dependency_chain: Vec<ModId>,
    /// The hook, event, registry, or packet involved, absent before a world.
    pub surface: Option<FailureSurface>,
}

impl Subject {
    /// A subject that is one identified mod and nothing more.
    #[must_use]
    pub fn of(mod_id: ModId) -> Self {
        Self {
            mod_id: Some(mod_id),
            ..Self::default()
        }
    }

    /// A subject nothing has identified, which is what a package that failed
    /// before its manifest parsed is.
    #[must_use]
    pub fn unidentified() -> Self {
        Self::default()
    }

    /// Returns this subject with the version it declared.
    #[must_use]
    pub fn declaring(mut self, version: Version) -> Self {
        self.version = Some(version);
        self
    }

    /// Returns this subject with the dependency path that reached it.
    #[must_use]
    pub fn required_by(mut self, chain: impl IntoIterator<Item = ModId>) -> Self {
        self.dependency_chain = chain.into_iter().collect();
        self
    }
}

/// One refusal, attributed and redacted, ready to be shown to a player.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refusal {
    attribution: FailureAttribution,
    quotes_untrusted_text: bool,
}

impl Refusal {
    /// Returns the `SPEC.md` §9.1 attribution.
    #[must_use]
    pub const fn attribution(&self) -> &FailureAttribution {
        &self.attribution
    }

    /// Returns whether the detail quotes words this project did not write.
    ///
    /// The text was redacted either way. This is what lets a surface tell a
    /// player that the sentence came from their mod, their operating system, or
    /// a parser, rather than from Ravel — which is worth saying, because those
    /// are the sentences a player cannot report as a Ravel wording bug.
    #[must_use]
    pub const fn quotes_untrusted_text(&self) -> bool {
        self.quotes_untrusted_text
    }

    /// Renders the refusal as one log line.
    #[must_use]
    pub fn line(&self) -> String {
        let attribution = &self.attribution;
        let subject = match (&attribution.mod_id, &attribution.version) {
            (Some(id), Some(version)) => format!("`{id}` {version}"),
            (Some(id), None) => format!("`{id}`"),
            (None, _) => "an unidentified package".to_owned(),
        };
        let kind = attribution
            .kind
            .map_or_else(String::new, |kind| format!(" [{}]", kind.name()));
        let chain = if attribution.dependency_chain.is_empty() {
            String::new()
        } else {
            format!(" (required by {})", ids(&attribution.dependency_chain))
        };
        let surface = attribution
            .surface
            .as_ref()
            .map_or_else(String::new, |surface| format!(" on {}", named(surface)));
        let author = if self.quotes_untrusted_text {
            ", quoted rather than written by Ravel"
        } else {
            ""
        };

        format!(
            "refused {subject} at `{}`{kind}{chain}{surface}: {}{author}",
            attribution.state.name(),
            attribution.message.text()
        )
    }
}

/// Everything Ravel has to say before a world may open.
///
/// This is the surface `ROADMAP.md`'s M2-W7 row calls the pre-world UI/log
/// diagnostic path: the report a player is shown, and the gate that decides
/// whether they get a world at all. A refusal from any stage of the pipeline
/// lands here — a manifest that will not parse, a digest that does not match, a
/// package for another platform, a side, a missing or incompatible or circular
/// dependency, or a mod that declined a lifecycle step — because every one of
/// them is a [`StructuredFailure`] and one arm renders all of them.
///
/// A report holds no filesystem path and no credential: every message it takes
/// is run through [`crate::redact`] on the way in, and the only messages it can
/// hold are the ones that came back.
///
/// The report is data and rendered lines rather than a widget. There is no UI
/// framework in this repository and M3-W5 owns the in-game surface; what a
/// surface needs is [`Self::refusals`], and what a log needs is [`Self::lines`].
///
/// # Examples
///
/// ```
/// use ravel_core::{FailureEnvironment, FailureKind, ModId, StructuredFailure};
/// use ravel_observability::{PreWorldReport, Subject};
/// # use std::fmt;
/// # #[derive(Debug)]
/// # struct Missing;
/// # impl fmt::Display for Missing {
/// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
/// #         f.write_str("`example_mod` requires `example_library`, which is not installed")
/// #     }
/// # }
/// # impl std::error::Error for Missing {}
/// # impl StructuredFailure for Missing {
/// #     fn failure_kind(&self) -> Option<FailureKind> {
/// #         Some(FailureKind::MissingRequiredDependency)
/// #     }
/// # }
/// let environment = FailureEnvironment {
///     ravel: "0.1.0".parse()?,
///     minecraft: "26.0.0".to_owned(),
///     platform: "linux".to_owned(),
///     architecture: "x86_64".to_owned(),
/// };
/// let mut report = PreWorldReport::new(environment);
/// report.world_access().expect("nothing has been refused");
///
/// report.refuse(&Subject::of("example_mod".parse()?), &Missing);
/// assert!(report.world_access().is_err());
/// assert_eq!(report.refusals().len(), 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreWorldReport {
    environment: FailureEnvironment,
    refusals: Vec<Refusal>,
    notices: Vec<DiagnosticMessage>,
}

impl PreWorldReport {
    /// Starts an empty report for one running host.
    ///
    /// The environment is supplied rather than discovered: `SPEC.md` §9.1
    /// requires every attribution to name the Minecraft and Ravel versions and
    /// the platform and architecture, and this crate is version-neutral.
    #[must_use]
    pub const fn new(environment: FailureEnvironment) -> Self {
        Self {
            environment,
            refusals: Vec::new(),
            notices: Vec::new(),
        }
    }

    /// Records a refusal from any stage of the pipeline.
    ///
    /// The failure's own text is redacted here rather than by the crate that
    /// raised it, which is what lets `ravel-core` hold the tag and no rules.
    pub fn refuse(&mut self, subject: &Subject, failure: &dyn StructuredFailure) {
        self.refusals.push(Refusal {
            attribution: FailureAttribution {
                mod_id: subject.mod_id.clone(),
                version: subject.version.clone(),
                dependency_chain: subject.dependency_chain.clone(),
                state: failure.lifecycle_state(),
                kind: failure.failure_kind(),
                surface: subject.surface.clone(),
                environment: self.environment.clone(),
                message: redact(failure.diagnostic().text()),
            },
            quotes_untrusted_text: failure.quotes_untrusted_text(),
        });
    }

    /// Records something a player is told without being refused.
    ///
    /// A discouraged combination is the one dependency outcome `SPEC.md` §3.4
    /// names that does not fail the graph, so it belongs in the report the
    /// player reads and not in the list that withholds their world.
    pub fn note(&mut self, text: &str) {
        self.notices.push(redact(text));
    }

    /// Returns the host every attribution in this report names.
    #[must_use]
    pub const fn environment(&self) -> &FailureEnvironment {
        &self.environment
    }

    /// Returns every refusal, in the order it was recorded.
    #[must_use]
    pub fn refusals(&self) -> &[Refusal] {
        &self.refusals
    }

    /// Returns every notice, in the order it was recorded.
    #[must_use]
    pub fn notices(&self) -> &[DiagnosticMessage] {
        &self.notices
    }

    /// Decides whether a world may open.
    ///
    /// This is the gate. `SPEC.md` §9.2 makes Ravel fail closed on an invalid
    /// required-mod graph, and §14 requires a required dependency failure to
    /// prevent world access, so a report holding any refusal withholds the
    /// world — including a refusal from a stage that ran long before any
    /// lifecycle machine existed, which is exactly the case a machine cannot
    /// see for itself. Notices do not withhold anything.
    ///
    /// # Errors
    ///
    /// Returns [`WorldAccessRefused`] naming how many problems there are and
    /// the first of them.
    pub fn world_access(&self) -> Result<(), WorldAccessRefused> {
        match self.refusals.first() {
            None => Ok(()),
            Some(first) => Err(WorldAccessRefused {
                refusals: self.refusals.len(),
                first: first.line(),
            }),
        }
    }

    /// Renders the whole report, one line per entry, refusals before notices.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.refusals
            .iter()
            .map(Refusal::line)
            .chain(
                self.notices
                    .iter()
                    .map(|notice| format!("noted: {}", notice.text())),
            )
            .collect()
    }
}

/// A world was not opened because something was refused before it.
///
/// The error carries the count and the first line rather than the whole report,
/// because a caller that wants the rest already holds the report it came from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldAccessRefused {
    refusals: usize,
    first: String,
}

impl WorldAccessRefused {
    /// Returns how many refusals the report held.
    #[must_use]
    pub const fn refusals(&self) -> usize {
        self.refusals
    }

    /// Returns the rendered first refusal, which is already redacted.
    #[must_use]
    pub fn first(&self) -> &str {
        &self.first
    }
}

impl fmt::Display for WorldAccessRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { refusals, first } = self;
        let plural = if *refusals == 1 { "" } else { "s" };
        write!(
            formatter,
            "no world opens until {refusals} problem{plural} \
             {} fixed; the first is: {first}",
            if *refusals == 1 { "is" } else { "are" }
        )
    }
}

impl std::error::Error for WorldAccessRefused {}

/// Renders a list of mod ids as a dependency path.
fn ids(chain: &[ModId]) -> String {
    chain
        .iter()
        .map(|id| format!("`{id}`"))
        .collect::<Vec<_>>()
        .join(" -> ")
}

/// Names the surface a failure reached, the way `SPEC.md` §9.1 lists them.
fn named(surface: &FailureSurface) -> String {
    match surface {
        FailureSurface::Hook(name) => format!("hook `{name}`"),
        FailureSurface::Event(name) => format!("event `{name}`"),
        FailureSurface::Registry(name) => format!("registry `{name}`"),
        FailureSurface::Packet(name) => format!("packet `{name}`"),
    }
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use ravel_core::{
        FailureEnvironment, FailureKind, FailureSurface, LifecycleState, ModId, Redaction,
        StructuredFailure, Version,
    };

    use super::{PreWorldReport, Subject, WorldAccessRefused};

    /// A failure a test can shape into any stage's report.
    #[derive(Debug)]
    struct Failure {
        text: String,
        kind: Option<FailureKind>,
        state: LifecycleState,
        quoted: bool,
    }

    impl Failure {
        fn new(text: &str) -> Self {
            Self {
                text: text.to_owned(),
                kind: None,
                state: LifecycleState::Discovered,
                quoted: false,
            }
        }

        fn of(mut self, kind: FailureKind, state: LifecycleState) -> Self {
            self.kind = Some(kind);
            self.state = state;
            self
        }

        fn quoted(mut self) -> Self {
            self.quoted = true;
            self
        }
    }

    impl fmt::Display for Failure {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(&self.text)
        }
    }

    impl std::error::Error for Failure {}

    impl StructuredFailure for Failure {
        fn failure_kind(&self) -> Option<FailureKind> {
            self.kind
        }

        fn lifecycle_state(&self) -> LifecycleState {
            self.state
        }

        fn quotes_untrusted_text(&self) -> bool {
            self.quoted
        }
    }

    fn environment() -> FailureEnvironment {
        FailureEnvironment {
            ravel: "0.1.0".parse().expect("a valid version"),
            minecraft: "26.0.0".to_owned(),
            platform: "linux".to_owned(),
            architecture: "x86_64".to_owned(),
        }
    }

    fn report() -> PreWorldReport {
        PreWorldReport::new(environment())
    }

    fn id(text: &str) -> ModId {
        text.parse().expect("a valid mod id")
    }

    fn version(text: &str) -> Version {
        text.parse().expect("a valid version")
    }

    #[test]
    fn an_empty_report_opens_a_world_and_says_nothing() {
        let report = report();
        assert_eq!(report.world_access(), Ok(()));
        assert!(report.refusals().is_empty());
        assert!(report.notices().is_empty());
        assert!(report.lines().is_empty());
        assert_eq!(report.environment(), &environment());
    }

    #[test]
    fn one_refusal_from_any_stage_withholds_the_world() {
        let mut report = report();
        report.refuse(
            &Subject::of(id("example_mod")),
            &Failure::new("`example_mod` requires `example_library`, which is not installed").of(
                FailureKind::MissingRequiredDependency,
                LifecycleState::Validated,
            ),
        );

        let refused = report.world_access().expect_err("a refusal withholds it");
        assert_eq!(refused.refusals(), 1);
        assert_eq!(
            refused.first(),
            "refused `example_mod` at `validated` [missing-required-dependency]: `example_mod` \
             requires `example_library`, which is not installed"
        );
        assert!(
            refused
                .to_string()
                .starts_with("no world opens until 1 problem is fixed")
        );
    }

    #[test]
    fn every_refusal_is_counted_and_the_first_is_the_one_reported() {
        let mut report = report();
        for (index, kind) in FailureKind::ALL.into_iter().enumerate() {
            report.refuse(
                &Subject::of(id("example_mod")),
                &Failure::new(&format!("problem {index}")).of(kind, LifecycleState::Validated),
            );
        }

        let refused = report.world_access().expect_err("seven refusals");
        assert_eq!(refused.refusals(), 7);
        assert!(refused.first().contains("problem 0"), "{refused}");
        assert!(refused.to_string().contains("7 problems are fixed"));
        assert_eq!(report.lines().len(), 7);
        assert_eq!(
            report
                .refusals()
                .iter()
                .filter_map(|refusal| refusal.attribution().kind)
                .collect::<Vec<_>>(),
            FailureKind::ALL,
            "each SPEC section 9.2 outcome stays distinct in the report"
        );
    }

    #[test]
    fn a_notice_is_reported_without_withholding_the_world() {
        let mut report = report();
        report.note(
            "`example_mod` discourages `example_rival`, and `example_rival` 2.0.0 is installed",
        );

        assert_eq!(report.world_access(), Ok(()));
        assert_eq!(report.notices().len(), 1);
        assert_eq!(report.notices()[0].redaction(), Redaction::Applied);
        assert_eq!(
            report.lines(),
            [
                "noted: `example_mod` discourages `example_rival`, and `example_rival` 2.0.0 is \
                 installed"
            ]
        );
    }

    #[test]
    fn every_message_the_report_holds_has_been_through_the_policy() {
        let mut report = report();
        report.refuse(
            &Subject::unidentified(),
            &Failure::new("could not read /home/player/.minecraft/instance/access_token=abc123")
                .quoted(),
        );
        report.note("wrote /home/player/logs/latest.log");

        for message in report
            .refusals()
            .iter()
            .map(|refusal| &refusal.attribution().message)
            .chain(report.notices())
        {
            assert_eq!(message.redaction(), Redaction::Applied);
            assert!(!message.text().contains("/home/"), "{}", message.text());
        }
        assert_eq!(
            report.refusals()[0].attribution().message.text(),
            "could not read access_token=[redacted]"
        );
    }

    #[test]
    fn a_package_that_never_parsed_is_reported_without_a_borrowed_identity() {
        let mut report = report();
        report.refuse(
            &Subject::unidentified(),
            &Failure::new("the manifest is not JSON: expected value at line 1 column 1").quoted(),
        );

        let refusal = &report.refusals()[0];
        assert_eq!(refusal.attribution().mod_id, None);
        assert_eq!(refusal.attribution().kind, None);
        assert!(refusal.quotes_untrusted_text());
        assert_eq!(
            refusal.line(),
            "refused an unidentified package at `discovered`: the manifest is not JSON: expected \
             value at line 1 column 1, quoted rather than written by Ravel"
        );
    }

    #[test]
    fn a_line_carries_every_facet_the_spec_requires_of_an_attribution() {
        let mut report = report();
        let subject = Subject {
            surface: Some(FailureSurface::Registry("example_mod:block".to_owned())),
            ..Subject::of(id("example_mod"))
                .declaring(version("1.4.0"))
                .required_by([id("example_pack"), id("example_bundle")])
        };
        report.refuse(
            &subject,
            &Failure::new("`example_mod` claimed an identifier `example_pack` already owns").of(
                FailureKind::ConflictingRegistryClaim,
                LifecycleState::Constructed,
            ),
        );

        assert_eq!(
            report.refusals()[0].line(),
            "refused `example_mod` 1.4.0 at `constructed` [conflicting-registry-claim] (required \
             by `example_pack` -> `example_bundle`) on registry `example_mod:block`: \
             `example_mod` claimed an identifier `example_pack` already owns"
        );

        let attribution = report.refusals()[0].attribution();
        assert_eq!(attribution.environment, environment());
        assert_eq!(attribution.version, Some(version("1.4.0")));
        assert_eq!(attribution.dependency_chain.len(), 2);
    }

    #[test]
    fn every_surface_the_spec_names_renders_under_its_own_word() {
        for (surface, expected) in [
            (FailureSurface::Hook("world_load".to_owned()), "hook"),
            (FailureSurface::Event("player_tick".to_owned()), "event"),
            (FailureSurface::Registry("a:b".to_owned()), "registry"),
            (FailureSurface::Packet("a:b".to_owned()), "packet"),
        ] {
            let mut report = report();
            report.refuse(
                &Subject {
                    surface: Some(surface),
                    ..Subject::of(id("example_mod"))
                },
                &Failure::new("something"),
            );
            let line = report.refusals()[0].line();
            assert!(line.contains(&format!(" on {expected} `")), "{line}");
        }
    }

    #[test]
    fn a_refusal_that_quotes_someone_else_says_so_and_one_that_does_not_stays_quiet() {
        let mut report = report();
        report.refuse(
            &Subject::of(id("first_mod")),
            &Failure::new("ravel said so"),
        );
        report.refuse(
            &Subject::of(id("second_mod")),
            &Failure::new("the mod said so").quoted(),
        );

        assert!(!report.refusals()[0].quotes_untrusted_text());
        assert!(!report.refusals()[0].line().contains("quoted rather than"));
        assert!(report.refusals()[1].quotes_untrusted_text());
        assert!(
            report.refusals()[1]
                .line()
                .ends_with(", quoted rather than written by Ravel")
        );
    }

    #[test]
    fn a_refusal_is_comparable_so_two_runs_can_be_shown_to_report_the_same_thing() {
        let refused = WorldAccessRefused {
            refusals: 2,
            first: "refused `example_mod` at `validated`: nope".to_owned(),
        };
        assert_eq!(refused.clone(), refused);
        assert_eq!(refused.refusals(), 2);
    }
}
