mod cli;
mod exec;
mod toolchain;
mod utils;

use anyhow::Result;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

// ---------------------------------------------------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------------------------------------------------

fn self_name() -> Option<String> {
    let mut args = env::args();
    let arg0 = args.next().map(PathBuf::from);
    arg0.as_ref()
        .and_then(|x| x.file_stem())
        .and_then(std::ffi::OsStr::to_str)
        .map(String::from)
}

#[tokio::main]
async fn main() -> Result<ExitCode> {
    match self_name().as_deref() {
        Some("verylup") => {
            cli::verylup_mode::main().await?;
        }
        Some(x) => {
            cli::proxy_mode::main(x).await?;
        }
        _ => (),
    }

    Ok(ExitCode::SUCCESS)
}
