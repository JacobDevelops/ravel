#![forbid(unsafe_code)]

mod common;
mod fixture;
mod linux_capture;
mod linux_memory;
#[cfg(target_os = "linux")]
mod linux_supervisor;
mod marker_schedule;
mod marker_socket;
mod tracy;
