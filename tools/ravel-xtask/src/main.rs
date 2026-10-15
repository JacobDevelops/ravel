#![forbid(unsafe_code)]

use std::env;
use std::process::ExitCode;

mod commands;

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match env::current_dir() {
        Ok(root) => commands::dispatch(&args, &root),
        Err(error) => {
            eprintln!("operation failed: cannot read current directory: {error}");
            ExitCode::FAILURE
        }
    }
}
