#![forbid(unsafe_code)]

//! Reading a Prism launch log without keeping anything the account owns.
//!
//! The gate runner is the binary beside this file. The reading itself is a
//! library because M2 records its own launch through the same ADG-5 mechanism
//! and redaction that is duplicated is redaction that drifts.

pub mod launch;
