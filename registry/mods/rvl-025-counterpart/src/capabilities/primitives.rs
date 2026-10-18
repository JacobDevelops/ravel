//! Instruction declarations and execution, behind the surface's registration window.

use std::collections::BTreeMap;

use super::SurfaceRefused;

/// What kind of value an instruction's parameter resolves to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterKind {
    /// A whole number.
    Amount,
    /// Text.
    Text,
    /// A truth value.
    Flag,
}

/// One declared parameter of an instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Parameter {
    /// What the body expects it to resolve to.
    pub kind: ParameterKind,
    /// Whether the body asked for the form that may not be absent.
    pub required: bool,
}

impl Parameter {
    /// Declares the value kind and whether an argument must be present.
    #[must_use]
    pub const fn new(kind: ParameterKind, required: bool) -> Self {
        Self { kind, required }
    }
}

/// One value an instruction was called with.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Argument {
    /// A whole number.
    Amount(i64),
    /// Text.
    Text(String),
    /// A truth value.
    Flag(bool),
    /// Nothing resolved.
    Absent,
}

impl Argument {
    /// Returns the kind this argument resolves to, or nothing when it is absent.
    #[must_use]
    pub const fn kind(&self) -> Option<ParameterKind> {
        match self {
            Self::Amount(_) => Some(ParameterKind::Amount),
            Self::Text(_) => Some(ParameterKind::Text),
            Self::Flag(_) => Some(ParameterKind::Flag),
            Self::Absent => None,
        }
    }
}

/// One step of an instruction's body.
///
/// §15 has a dependent supply a runnable body. Ravel's cross-mod seam carries
/// data rather than a Rust value — a consumer that named the provider's function
/// type would depend on the provider's build — so a body is declared as the
/// steps a processor takes against a program rather than as a closure. That is
/// the same trade the service seam makes, and it is what lets
/// [`super::Surface::run`] actually write the program instead of looking as though it
/// had.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    /// Produce the value the parameter at this index resolved to.
    Produce {
        /// Which parameter.
        index: usize,
    },
    /// Produce a value the declaration carries.
    Constant(Argument),
    /// Suspend the program for this many ticks.
    Suspend {
        /// How many ticks the body asked for.
        ticks: u32,
    },
    /// Wait on the craft the parameter at this index names.
    Await {
        /// Which parameter.
        index: usize,
    },
}

/// One instruction or function a dependent supplies.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Instruction {
    /// The instruction's own name.
    pub name: String,
    /// Its declared parameters, in order.
    pub parameters: Vec<Parameter>,
    /// What it produces.
    pub output: ParameterKind,
    /// The categories it is listed under.
    pub categories: Vec<String>,
    /// The steps the processor takes when it runs.
    pub body: Vec<Effect>,
}

impl Instruction {
    /// Declares an instruction with no parameters, categories or body steps yet.
    #[must_use]
    pub fn new(name: impl Into<String>, output: ParameterKind) -> Self {
        Self {
            name: name.into(),
            parameters: Vec::new(),
            output,
            categories: Vec::new(),
            body: Vec::new(),
        }
    }
}

/// The program an instruction runs against.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct Program {
    /// The last value produced.
    pub last: Option<Argument>,
    /// A craft the program is waiting on, where it has one.
    pub ticket: Option<String>,
    /// How many ticks the body asked to suspend itself for.
    pub delay: u32,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Runtime {
    instructions: BTreeMap<String, Instruction>,
}

impl Runtime {
    pub(super) fn register(&mut self, instruction: Instruction) -> Result<(), SurfaceRefused> {
        for effect in &instruction.body {
            let named = match effect {
                Effect::Produce { index } | Effect::Await { index } => Some(*index),
                Effect::Constant(_) | Effect::Suspend { .. } => None,
            };
            if let Some(index) = named
                && index >= instruction.parameters.len()
            {
                return Err(SurfaceRefused::BadBody { index });
            }
        }
        self.instructions
            .insert(instruction.name.clone(), instruction);
        Ok(())
    }

    pub(super) fn run(
        &self,
        program: &mut Program,
        name: &str,
        arguments: &[Argument],
    ) -> Result<(), SurfaceRefused> {
        let instruction =
            self.instructions
                .get(name)
                .ok_or_else(|| SurfaceRefused::UnknownInstruction {
                    name: name.to_owned(),
                })?;
        if instruction.parameters.len() != arguments.len() {
            return Err(SurfaceRefused::ParameterCount {
                declared: instruction.parameters.len(),
                given: arguments.len(),
            });
        }
        for (index, (parameter, argument)) in
            instruction.parameters.iter().zip(arguments).enumerate()
        {
            match argument.kind() {
                None if parameter.required => {
                    return Err(SurfaceRefused::AbsentParameter { index });
                }
                None => {}
                Some(kind) if kind == parameter.kind => {}
                Some(_) => return Err(SurfaceRefused::WrongParameter { index }),
            }
        }
        let mut produced: Option<Argument> = None;
        let mut ticket = None;
        let mut delay = 0;
        for effect in &instruction.body {
            match effect {
                Effect::Produce { index } => {
                    produced = arguments.get(*index).cloned();
                }
                Effect::Constant(value) => produced = Some(value.clone()),
                Effect::Suspend { ticks } => delay = *ticks,
                Effect::Await { index } => {
                    if let Some(Argument::Text(name)) = arguments.get(*index) {
                        ticket = Some(name.clone());
                    }
                }
            }
        }
        if let Some(value) = &produced
            && value.kind().is_some_and(|kind| kind != instruction.output)
        {
            return Err(SurfaceRefused::WrongOutput {
                declared: instruction.output,
            });
        }
        program.last = produced;
        program.ticket = ticket;
        program.delay = delay;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Argument, Effect, Instruction, Parameter, ParameterKind, Program, Runtime, SurfaceRefused,
    };

    #[test]
    fn a_wrong_output_preserves_every_field_of_an_existing_program() {
        let mut runtime = Runtime::default();
        let mut instruction = Instruction::new("wrong", ParameterKind::Text);
        instruction.body = vec![
            Effect::Suspend { ticks: 99 },
            Effect::Constant(Argument::Flag(true)),
        ];
        runtime
            .register(instruction)
            .expect("a body with no parameter references");
        let mut program = Program {
            last: Some(Argument::Amount(42)),
            ticket: Some("existing craft".to_owned()),
            delay: 7,
        };
        let before = program.clone();
        assert_eq!(
            runtime.run(&mut program, "wrong", &[]),
            Err(SurfaceRefused::WrongOutput {
                declared: ParameterKind::Text,
            })
        );
        assert_eq!(program, before);
    }

    #[test]
    fn an_empty_successful_body_clears_previous_program_effects() {
        let mut runtime = Runtime::default();
        runtime
            .register(Instruction::new("clear", ParameterKind::Text))
            .expect("an empty body is valid");
        let mut program = Program {
            last: Some(Argument::Text("old".to_owned())),
            ticket: Some("old craft".to_owned()),
            delay: 7,
        };
        runtime
            .run(&mut program, "clear", &[])
            .expect("no arguments are required");
        assert_eq!(program, Program::default());
    }

    #[test]
    fn constructed_parameters_enforce_presence_and_kind_before_producing_a_value() {
        let mut instruction = Instruction::new("echo", ParameterKind::Text);
        instruction.parameters = vec![
            Parameter::new(ParameterKind::Text, true),
            Parameter::new(ParameterKind::Flag, false),
        ];
        instruction.body.push(Effect::Produce { index: 0 });
        let mut runtime = Runtime::default();
        runtime.register(instruction).expect("a declared parameter");
        let mut program = Program::default();
        runtime
            .run(
                &mut program,
                "echo",
                &[Argument::Text("answer".to_owned()), Argument::Absent],
            )
            .expect("an optional flag may be absent");
        assert_eq!(program.last, Some(Argument::Text("answer".to_owned())));
        assert_eq!(
            runtime.run(&mut program, "echo", &[Argument::Absent, Argument::Absent]),
            Err(SurfaceRefused::AbsentParameter { index: 0 })
        );
        assert_eq!(
            runtime.run(
                &mut program,
                "echo",
                &[Argument::Text("answer".to_owned()), Argument::Amount(1)],
            ),
            Err(SurfaceRefused::WrongParameter { index: 1 })
        );
    }
}
