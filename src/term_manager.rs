use std::{
    io::{self, Stdin, Stdout},
    os::fd::{AsRawFd, RawFd},
};

/// Manipulates terminal state via libc.
pub struct TermManager {
    pub stdin: Stdin,
    pub stdout: Stdout,
    pub fd: RawFd,
    pub termios: libc::termios,
}

impl TermManager {
    pub fn init() -> Result<TermManager, io::Error> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let fd = stdin.as_raw_fd();
        let termios = init_termios(fd)?;

        Ok(TermManager {
            stdin,
            stdout,
            fd,
            termios,
        })
    }
}

impl Drop for TermManager {
    // Restore terminal settings when TermManager is destructed.
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(self.fd, libc::TCSANOW, &self.termios);
        }
    }
}

/// Init termios struct.
/// Disables canonical mode and echo.
fn init_termios(fd: RawFd) -> Result<libc::termios, io::Error> {
    unsafe {
        // Initialize a termios struct.
        // See man(3) tcsetattr for more details.
        let mut termios: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut termios) != 0 {
            return Err(io::Error::last_os_error());
        }

        let mut raw = termios;

        // Unset ICANON and ECHO.
        raw.c_lflag &= !(libc::ICANON | libc::ECHO);

        // VMIN is the minimum number of chars to read from stdin.
        // VTIME is the timeout for input. Disabled when 0.
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;

        // Apply settings.
        libc::tcsetattr(fd, libc::TCSANOW, &raw);
        Ok(termios)
    }
}
