use std::io;
use std::process::{Command, ExitStatus};

#[cfg(unix)]
pub fn exec(cmd: &mut Command) -> io::Result<ExitStatus> {
    use std::os::unix::prelude::CommandExt;
    Err(cmd.exec())
}

#[cfg(windows)]
pub fn exec(cmd: &mut Command) -> io::Result<ExitStatus> {
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{FALSE, TRUE};
    use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;

    unsafe extern "system" fn ctrlc_handler(_: u32) -> BOOL {
        // Do nothing. Let the child process handle it.
        TRUE
    }
    unsafe {
        if SetConsoleCtrlHandler(Some(ctrlc_handler), TRUE) == FALSE {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Unable to set console handler",
            ));
        }
    }

    cmd.status()
}
