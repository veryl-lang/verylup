use crate::config::Config;
use crate::exec::exec;
use crate::toolchain::{ToolChain, TOOLS};
use crate::utils::*;
use anyhow::{anyhow, bail, Result};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::aot::Shell;
use console::Style;
use fern::Dispatch;
use log::{info, Level, LevelFilter};
use semver::Version;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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
    Default(OptDefault),
    Override(OptOverride),
    Setup(OptSetup),
    Completion(OptCompletion),
    Config(OptConfig),
}

/// Show installed toolchains
#[derive(Args)]
pub struct OptShow {}

/// Update Veryl toolchains and verylup
#[derive(Args)]
pub struct OptUpdate {
    /// Toolchain package path for offline installation
    #[arg(long)]
    pkg: Option<PathBuf>,
}

/// Install or update a given toolchain
#[derive(Args)]
pub struct OptInstall {
    target: String,

    /// Toolchain package path for offline installation
    #[arg(long)]
    pkg: Option<PathBuf>,

    /// Debug build for local install
    #[arg(long)]
    debug: bool,
}

/// Uninstall a given toolchain
#[derive(Args)]
pub struct OptUninstall {
    target: String,
}

/// Set a given toolchain as default
#[derive(Args)]
pub struct OptDefault {
    target: String,
}

/// Modify toolchain overrides for directories
#[derive(Args)]
pub struct OptOverride {
    #[command(subcommand)]
    command: OverrideCommand,
}

#[derive(Subcommand)]
pub enum OverrideCommand {
    List(OptOverrideList),
    Set(OptOverrideSet),
    Unset(OptOverrideUnset),
}

/// List directory toolchain overrides
#[derive(Args)]
pub struct OptOverrideList {}

/// Set the override toolchain for a directory
#[derive(Args)]
pub struct OptOverrideSet {
    target: String,
}

/// Remove the override toolchain for a directory
#[derive(Args)]
pub struct OptOverrideUnset {}

/// Setup Veryl toolchain
#[derive(Args)]
pub struct OptSetup {
    /// Offline mode
    #[arg(long)]
    offline: bool,

    /// Toolchain package path for offline installation
    #[arg(long)]
    pkg: Option<PathBuf>,

    /// Disable self-update
    #[arg(long)]
    no_self_update: bool,
}

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

/// Modify verylup configuration
#[derive(Args)]
pub struct OptConfig {
    #[command(subcommand)]
    command: ConfigCommand,
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    Show(OptConfigShow),
    Set(OptConfigSet),
    Unset(OptConfigUnset),
}

/// Show the current configuration
#[derive(Args)]
pub struct OptConfigShow {}

/// Modify an entry of the configuration
#[derive(Args)]
pub struct OptConfigSet {
    key: String,
    value: String,
}

/// Remove an entry of the configuration
#[derive(Args)]
pub struct OptConfigUnset {
    key: String,
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
        .level_for("reqwest", LevelFilter::Warn)
        .chain(std::io::stderr())
        .apply()?;

    match opt.command {
        Commands::Show(_) => {
            println!("installed toolchains");
            println!("--------------------\n");

            let default_toolchain = ToolChain::default_toolchain();
            for x in ToolChain::list() {
                let text = if x == ToolChain::Latest {
                    if let Ok(version) = x.get_actual_version() {
                        format!("{x}: {version}")
                    } else {
                        x.to_string()
                    }
                } else {
                    x.to_string()
                };
                if Some(&x) == default_toolchain.as_ref() {
                    println!("{text} (default)");
                } else {
                    println!("{text}");
                }
            }
        }
        Commands::Update(x) => {
            let config = Config::load();
            if x.pkg.is_none() && config.offline {
                bail!("\"--pkg\" is required in offline mode");
            }

            let toolchain = ToolChain::Latest;
            toolchain.install(&x.pkg, false, &config).await?;

            if ToolChain::list().contains(&ToolChain::Nightly) {
                if config.offline {
                    info!("nightly toolchain is ignored in offline mode");
                } else {
                    let toolchain = ToolChain::Nightly;
                    toolchain.install(&None, false, &config).await?;
                }
            }

            if !config.offline {
                self_update().await?;
            }
        }
        Commands::Install(x) => {
            let config = Config::load();
            if x.pkg.is_none() && config.offline {
                bail!("\"--pkg\" is required in offline mode");
            }

            let toolchain = ToolChain::try_from(&x.target)?;
            toolchain.install(&x.pkg, x.debug, &config).await?;
        }
        Commands::Uninstall(x) => {
            let toolchain = ToolChain::try_from(&x.target)?;
            toolchain.uninstall()?;
        }
        Commands::Default(x) => {
            let toolchain = ToolChain::try_from(&x.target)?;
            let mut config = Config::load();
            config.default_toolchain = Some(toolchain.to_string());
            config.save()?;
        }
        Commands::Override(x) => {
            let mut config = Config::load();

            match x.command {
                OverrideCommand::List(_) => {
                    for (path, toolchain) in &config.overrides {
                        println!("{} {}", path.to_string_lossy(), toolchain);
                    }
                }
                OverrideCommand::Set(x) => {
                    let toolchain = ToolChain::try_from(&x.target)?;
                    let dir = search_project()?;
                    config.overrides.insert(dir.clone(), toolchain.to_string());
                    info!("adding toolchain override for {}", dir.to_string_lossy());
                    config.save()?;
                }
                OverrideCommand::Unset(_) => {
                    let dir = search_project()?;
                    if config.overrides.remove(&dir).is_some() {
                        info!("removing toolchain override for {}", dir.to_string_lossy());
                        config.save()?;
                    } else {
                        info!("no toolchain override for {}", dir.to_string_lossy());
                    }
                }
            }
        }
        Commands::Setup(x) => {
            let mut config = Config::load();
            if x.offline {
                if x.pkg.is_none() {
                    bail!("\"--pkg\" is required in offline mode");
                }

                config.offline = true;
                config.save()?;
            }
            if x.no_self_update {
                config.self_update = false;
                config.save()?;
            }

            let toolchain = ToolChain::Latest;
            toolchain.install(&x.pkg, false, &config).await?;
            let self_path = env::current_exe()?;
            update_link(&self_path)?;
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
        Commands::Config(x) => match x.command {
            ConfigCommand::Show(_) => {
                let config = Config::load();
                println!("{config}");
            }
            ConfigCommand::Set(x) => {
                let mut config = Config::load();
                config.set(&x.key, &x.value)?;
                config.save()?;
            }
            ConfigCommand::Unset(x) => {
                let mut config = Config::load();
                config.unset(&x.key)?;
                config.save()?;
            }
        },
    }

    Ok(())
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

async fn self_update() -> Result<()> {
    let config = Config::load();
    let latest_version = get_latest_version("verylup", &config).await?;
    let self_version = Version::parse(VERSION)?;

    if config.self_update {
        if latest_version > self_version {
            info!("downloading verylup: {latest_version}");

            let url = get_archive_url("verylup", &latest_version)?;
            let data = download(&url, &config).await?;
            let mut file = tempfile::tempfile()?;
            file.write_all(&data)?;

            info!("installing verylup: {latest_version}");

            let dir = tempfile::tempdir()?;

            unzip(&file, dir.path())?;

            let binary = dir.path().join("verylup");

            // save self_path before replacing
            let self_path = env::current_exe()?;

            self_replace::self_replace(binary)?;
            update_link(&self_path)?;
        } else {
            info!("checking verylup: {self_version} (up-to-date)");
        }
    } else {
        info!("self-update is disabled");
    }
    Ok(())
}

fn update_link(self_path: &Path) -> Result<()> {
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

    Ok(())
}
