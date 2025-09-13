use scopeguard::guard;
use std::{
    io::{self, Read, Write},
    os::fd::AsRawFd,
    str,
};

enum InputState {
    Normal,
    Escape,
    BracketedEscape,
}

fn main() -> io::Result<()> {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();
    let fd = stdin.as_raw_fd();

    let orig_termios = unsafe {
        // Initialize a termios struct.
        // See man(3) tcsetattr for more details.
        let mut orig_termios: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut orig_termios) != 0 {
            return Err(io::Error::last_os_error());
        }

        let mut raw = orig_termios;

        // Unset ICANON and ECHO.
        raw.c_lflag &= !(libc::ICANON | libc::ECHO);

        // VMIN is the minimum number of chars to read from stdin.
        // VTIME is the timeout for input. Disabled when 0.
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;

        // Apply settings.
        libc::tcsetattr(fd, libc::TCSANOW, &raw);
        orig_termios
    };

    // Reset termios settings.
    let _restore_termios = guard(orig_termios, |orig_termios| unsafe {
        libc::tcsetattr(fd, libc::TCSANOW, &orig_termios);
    });

    let mut line = String::new();
    let mut cursor_pos: usize = 0;

    let mut lines: Vec<String> = Vec::new();
    let mut lines_pos: usize = 0;

    let mut input_state = InputState::Normal;
    let mut escape_buffer = Vec::new();

    stdout.flush()?;
    const PROMPT: &str = "> ";
    print!("{}", PROMPT);
    stdout.flush()?;

    loop {
        let mut buf = [0u8; 1];
        let _bytes_read = match stdin.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                eprintln!("Error reading from stdin: {}", e);
                break;
            }
        };
        let c = buf[0];

        match input_state {
            InputState::Normal => match c {
                b'\x1b' => {
                    input_state = InputState::Escape;
                    escape_buffer.clear();
                }
                b'q' | b'\x03' => break,
                b'\n' | b'\r' => {
                    println!("\r\n{}", line);
                    lines.push(line.clone());
                    lines_pos += 1;
                    line.clear();
                    cursor_pos = 0;
                    print!("{}", PROMPT);
                    stdout.flush()?;
                }
                b'\x08' | b'\x7f' => {
                    if cursor_pos > 0 {
                        let mut byte_idx_to_remove = 0;
                        let mut current_char_count = 0;
                        for (idx, _) in line.char_indices() {
                            if current_char_count == cursor_pos - 1 {
                                byte_idx_to_remove = idx;
                                break;
                            }
                            current_char_count += 1;
                        }
                        line.remove(byte_idx_to_remove);

                        cursor_pos -= 1;

                        write!(stdout, "\x1b[1D").unwrap();
                        write!(stdout, "{}\x1b[K", &line[byte_idx_to_remove..]).unwrap();
                        let chars_after_cursor = line.chars().skip(cursor_pos).count();
                        if chars_after_cursor > 0 {
                            write!(stdout, "\x1b[{}D", chars_after_cursor).unwrap();
                        }
                        stdout.flush()?;
                    }
                }
                _ => {
                    if let Some(char_byte) =
                        str::from_utf8(&[c]).ok().and_then(|s| s.chars().next())
                    {
                        if char_byte.is_ascii_graphic()
                            || (char_byte.is_whitespace() && char_byte != '\t')
                        {
                            if cursor_pos == line.chars().count() {
                                print!("{}", char_byte);
                                line.push(char_byte);
                            } else {
                                let mut byte_idx = 0;
                                for (idx, _) in line.char_indices().take(cursor_pos) {
                                    byte_idx = idx;
                                }
                                line.insert(byte_idx, char_byte);
                                write!(stdout, "\x1b[{}D", cursor_pos).unwrap();
                                write!(stdout, "{}\x1b[K", line).unwrap();
                                let chars_after_new_cursor =
                                    line.chars().skip(cursor_pos + 1).count();
                                if chars_after_new_cursor > 0 {
                                    write!(stdout, "\x1b[{}D", chars_after_new_cursor).unwrap();
                                }
                            }
                            cursor_pos += 1;
                            stdout.flush()?;
                        }
                    }
                }
            },
            InputState::Escape => {
                escape_buffer.push(c);
                match c {
                    b'[' => {
                        input_state = InputState::BracketedEscape;
                    }
                    _ => {
                        input_state = InputState::Normal;
                        escape_buffer.clear();
                    }
                }
            }
            InputState::BracketedEscape => {
                escape_buffer.push(c);
                match c {
                    // Up
                    b'A' => {
                        if lines.len() > 0 && lines_pos > 0 {
                            line = lines[lines_pos - 1].clone();
                            lines_pos -= 1;
                            print!("\r{}{}\x1b[K", PROMPT, line);
                            stdout.flush()?;
                            cursor_pos = 0;
                        }
                        input_state = InputState::Normal;
                        escape_buffer.clear();
                    }
                    // Down
                    b'B' => {
                        if lines.len() > 0 && (lines_pos + 1) < lines.len() {
                            lines_pos += 1;
                            line = lines[lines_pos].clone();
                            print!("\r{}{}\x1b[K", PROMPT, line);
                            stdout.flush()?;
                            cursor_pos = 0;
                        }
                        input_state = InputState::Normal;
                        escape_buffer.clear();
                    }
                    // Right
                    b'C' => {
                        if cursor_pos < line.chars().count() {
                            write!(stdout, "\x1b[1C")?;
                            stdout.flush()?;
                            cursor_pos += 1;
                        }
                        input_state = InputState::Normal;
                        escape_buffer.clear();
                    }
                    // Left
                    b'D' => {
                        if cursor_pos > 0 {
                            write!(stdout, "\x1b[1D")?;
                            stdout.flush()?;
                            cursor_pos -= 1;
                        }
                        input_state = InputState::Normal;
                        escape_buffer.clear();
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
