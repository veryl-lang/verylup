use crate::toolchain::{ToolChain, TOOLS};
use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use console::Style;
use fern::Dispatch;
use log::{Level, LevelFilter};
use std::env;
use std::fs;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Opt {
    /// No output printed to stdout
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Use verbose output
    #[arg(long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Show(OptShow),
    Update(OptUpdate),
    Install(OptInstall),
    Uninstall(OptUninstall),
    Setup(OptSetup),
}

/// Show installed toolchains
#[derive(Args)]
pub struct OptShow {}

/// Update Veryl toolchains and verylup
#[derive(Args)]
pub struct OptUpdate {}

/// Install or update a given toolchain
#[derive(Args)]
pub struct OptInstall {
    target: String,
}

/// Uninstall a given toolchain
#[derive(Args)]
pub struct OptUninstall {
    target: String,
}

/// Setup Veryl toolchain
#[derive(Args)]
pub struct OptSetup {}

pub async fn main() -> Result<()> {
    let opt = Opt::parse();

    let level = if opt.verbose {
        LevelFilter::Debug
    } else if opt.quiet {
        LevelFilter::Warn
    } else {
        LevelFilter::Info
    };

    Dispatch::new()
        .format(|out, message, record| {
            let style = match record.level() {
                Level::Error => Style::new().red().bright(),
                Level::Warn => Style::new().yellow().bright(),
                Level::Info => Style::new().green().bright(),
                Level::Debug => Style::new().cyan().bright(),
                Level::Trace => Style::new().magenta().bright(),
            };
            out.finish(format_args!(
                "{} {}{}",
                style.apply_to(format!("[{:<5}]", record.level())),
                " ".repeat(
                    12 - format!("{message}")
                        .split_ascii_whitespace()
                        .next()
                        .unwrap()
                        .len()
                ),
                message
            ))
        })
        .level(level)
        .chain(std::io::stderr())
        .apply()?;

    match opt.command {
        Commands::Show(_) => {
            println!("installed toolchains");
            println!("--------------------\n");

            let list = ToolChain::list();
            for (i, x) in list.iter().enumerate() {
                if i == list.len() - 1 {
                    println!("{x} (default)");
                } else {
                    println!("{x}");
                }
            }
        }
        Commands::Update(_) => {
            let toolchain = ToolChain::Latest;
            toolchain.install().await?;
        }
        Commands::Install(x) => {
            let toolchain = ToolChain::try_from(&x.target)?;
            toolchain.install().await?;
        }
        Commands::Uninstall(x) => {
            let toolchain = ToolChain::try_from(&x.target)?;
            toolchain.uninstall().await?;
        }
        Commands::Setup(_) => {
            let toolchain = ToolChain::Latest;
            toolchain.install().await?;

            let self_path = env::current_exe()?;
            let self_path = self_path.canonicalize()?;

            for tool in TOOLS {
                let mut tool_path = self_path.parent().unwrap().join(tool);
                if cfg!(target_os = "windows") {
                    tool_path.set_extension("exe");
                }
                if tool_path.exists() {
                    fs::remove_file(&tool_path)?;
                    fs::hard_link(&self_path, &tool_path)?;
                } else {
                    fs::hard_link(&self_path, &tool_path)?;
                }
            }
        }
    }

    Ok(())
}
