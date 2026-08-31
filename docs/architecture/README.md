# Architecture

These documents record constraints already fixed by the specification. They do
not resolve ADG-1 through ADG-9. Decisions that select a mechanism belong in
reviewed ADRs under `docs/decisions/` and must cite comparable evidence.

[`m1-measurement-gate.md`](m1-measurement-gate.md) defines the Linux evidence
and failure actions every ADG-1 through ADG-5 candidate must use. It does not
select a candidate.

[`agentic-acceptance-harness.md`](agentic-acceptance-harness.md) records how an
acceptance run drives a modded pack through a running game — the mechanism M8-W4
and M10 are verified by — and which parts of it are built, decided, or owned but
unstarted. The mechanism it selects is decided in
[`../decisions/driving-acceptance-through-loader-crossings.md`](../decisions/driving-acceptance-through-loader-crossings.md);
this document is the comparison behind it.
