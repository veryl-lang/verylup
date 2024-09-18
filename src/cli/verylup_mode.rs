use crate::exec::exec;
use crate::toolchain::{ToolChain, TOOLS};
use crate::utils::*;
use anyhow::{anyhow, Result};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::aot::Shell;
use console::Style;
use fern::Dispatch;
use log::{info, Level, LevelFilter};
use semver::Version;
use std::env;
use std::fs;
use std::io::Write;

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
    Completion(OptCompletion),
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

/// Generate tab-completion scripts for your shell
#[derive(Args)]
pub struct OptCompletion {
    shell: CompletionShell,
    command: CompletionCommand,
}

#[derive(Clone, ValueEnum)]
#[clap(rename_all = "lower")]
pub enum CompletionShell {
    Bash,
    Elvish,
    Fish,
    PowerShell,
    Zsh,
}

impl std::fmt::Display for CompletionShell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            CompletionShell::Bash => "bash",
            CompletionShell::Elvish => "elvish",
            CompletionShell::Fish => "fish",
            CompletionShell::PowerShell => "powershell",
            CompletionShell::Zsh => "zsh",
        };
        text.fmt(f)
    }
}

#[derive(Clone, ValueEnum)]
pub enum CompletionCommand {
    Verylup,
    Veryl,
}

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
            self_update().await?;
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
                info!("creating hardlink: {tool}");

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
        Commands::Completion(x) => match x.command {
            CompletionCommand::Verylup => {
                let shell = match x.shell {
                    CompletionShell::Bash => Shell::Bash,
                    CompletionShell::Elvish => Shell::Elvish,
                    CompletionShell::Fish => Shell::Fish,
                    CompletionShell::PowerShell => Shell::PowerShell,
                    CompletionShell::Zsh => Shell::Zsh,
                };
                clap_complete::generate(
                    shell,
                    &mut Opt::command(),
                    "verylup",
                    &mut std::io::stdout(),
                );
            }
            CompletionCommand::Veryl => {
                let toolchain =
                    ToolChain::default_toolchain().ok_or(anyhow!("no toolchain is found"))?;
                let mut cmd = std::process::Command::new(toolchain.get_path("veryl"));
                cmd.arg("check")
                    .arg("--completion")
                    .arg(x.shell.to_string());
                exec(&mut cmd)?;
            }
        },
    }

    Ok(())
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

async fn self_update() -> Result<()> {
    let latest_version = get_latest_version("verylup").await?;
    let self_version = Version::parse(VERSION)?;

    if latest_version > self_version {
        info!("downloading verylup: {latest_version}");

        let url = get_archive_url("verylup", &latest_version)?;
        let data = download(&url).await?;
        let mut file = tempfile::tempfile()?;
        file.write_all(&data)?;

        info!("installing verylup: {latest_version}");

        let dir = tempfile::tempdir()?;

        unzip(&file, dir.path())?;

        let binary = dir.path().join("verylup");

        self_replace::self_replace(binary)?;
    } else {
        info!("checking verylup: {self_version} (up-to-date)");
    }
    Ok(())
}
