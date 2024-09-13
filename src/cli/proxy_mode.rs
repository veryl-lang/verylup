use crate::exec::exec;
use crate::toolchain::ToolChain;
use anyhow::{anyhow, bail, Result};
use std::env;
use std::process::Command;

pub async fn main(arg0: &str) -> Result<()> {
    let arg1 = env::args().nth(1);
    let toolchain = arg1
        .as_ref()
        .filter(|x| x.starts_with('+'))
        .map(|x| ToolChain::try_from(&x[1..]))
        .transpose()?;

    let cmd_args: Vec<_> = env::args_os()
        .skip(1 + toolchain.is_some() as usize)
        .collect();

    let default_toolchain =
        ToolChain::default_toolchain().ok_or(anyhow!("no toolchain is found"))?;

    let toolchain = toolchain.unwrap_or(default_toolchain);
    if !toolchain.exists() {
        bail!("toolchain \"{toolchain}\" is not found");
    }

    let mut cmd = Command::new(toolchain.get_path(arg0));
    cmd.args(cmd_args);
    exec(&mut cmd)?;

    Ok(())
}
