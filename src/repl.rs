use term_manager::TermManager;

pub type Result<T> = std::result::Result<T, Error>;
pub type ProcessFunc = fn(String) -> Result<String>;
pub type LineEndFunc = fn(String) -> bool;

pub enum Error {
    InitFail(String),
}

pub enum InputState {
    Normal,
    Escape,
    BracketedEscape,
}

pub enum LineEndFuncField {
    Func(LineEndFunc),
    None,
}

pub struct Repl {
    tmanager: TermManager,
    process_line: ProcessFunc,
    line_end: LineEndFuncField,
    line: String,
    lines: Vec<String>,
    cursor_pos: usize,
    lines_pos: usize,
    escape_buffer: Vec<u8>,
    input_state: InputState,
    prompt: String,
}

impl Repl {
    pub fn new(
        prompt: String,
        process_line: ProcessFunc,
        line_end: LineEndFuncField,
    ) -> Result<Self> {
        let tmanager = TermManager::new().or_else(|e| {
            let msg = format!("failed to initialized Repl: {}", e);
            Err(Error::InitFail(msg))
        })?;
        let line = String::new();
        let cursor_pos: usize = 0;
        let lines: Vec<String> = Vec::new();
        let lines_pos: usize = 0;
        let escape_buffer = Vec::new();
        let input_state = InputState::Normal;

        Ok(Repl {
            tmanager,
            process_line,
            line_end,
            line,
            cursor_pos,
            lines,
            lines_pos,
            escape_buffer,
            input_state,
            prompt,
        })
    }

    pub fn get_line() -> Result<String> {
        let mut buf = [0u8; 1];
        match tmanager.read(&mut buf) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("Error reading from tmanager.stdin: {:?}", e);
                return Err(e);
            }
        };
        let c = buf[0];

        match input_state {
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
                            if let Err(e) = tmanager.flush() {
                                eprintln!("{}", e);
                                return Err(());
                            };
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
                            if let Err(e) = tmanager.flush() {
                                eprintln!("{}", e);
                                return Err(());
                            };
                            cursor_pos = 0;
                        }
                        input_state = InputState::Normal;
                        escape_buffer.clear();
                    }
                    // Right
                    b'C' => {
                        if cursor_pos < line.chars().count() {
                            if let Err(e) = tmanager.write("\x1b[1C".as_bytes()) {
                                eprintln!("{}", e);
                                return Err(());
                            }

                            if let Err(e) = tmanager.flush() {
                                eprintln!("{}", e);
                                return Err(());
                            }

                            cursor_pos += 1;
                        }
                        input_state = InputState::Normal;
                        escape_buffer.clear();
                    }
                    // Left
                    b'D' => {
                        if cursor_pos > 0 {
                            if let Err(e) = tmanager.write("\x1b[1D".as_bytes()) {
                                eprintln!("{}", e);
                                return Err(());
                            }
                            if let Err(e) = tmanager.flush() {
                                eprintln!("{}", e);
                                return Err(());
                            };
                            cursor_pos -= 1;
                        }
                        input_state = InputState::Normal;
                        escape_buffer.clear();
                    }
                    _ => {}
                }
            }
            InputState::Normal => match c {
                b'\x1b' => {
                    input_state = InputState::Escape;
                    escape_buffer.clear();
                }
                b'q' | b'\x03' => {}
                b'\n' | b'\r' => {
                    println!("\r\n{}", line);
                    lines.push(line.clone());
                    lines_pos += 1;
                    line.clear();
                    cursor_pos = 0;
                    print!("{}", PROMPT);
                    if let Err(e) = tmanager.flush() {
                        eprintln!("{}", e);
                        return Err(());
                    };
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

                        if let Err(e) = tmanager.write("\x1b[1D".as_bytes()) {
                            eprintln!("{}", e);
                            return Err(());
                        }
                        let clear_line_cmd = format!("{}\x1b[K", &line[byte_idx_to_remove..]);
                        if let Err(e) = tmanager.write(clear_line_cmd.as_bytes()) {
                            eprintln!("{}", e);
                            return Err(());
                        }
                        let chars_after_cursor = line.chars().skip(cursor_pos).count();
                        if chars_after_cursor > 0 {
                            let move_cursor_left = format!("\x1b[{}D", chars_after_cursor);
                            if let Err(e) = tmanager.write(move_cursor_left.as_bytes()) {
                                eprintln!("{}", e);
                                return Err(());
                            }
                        }
                        if let Err(e) = tmanager.flush() {
                            eprintln!("{}", e);
                            return Err(());
                        };
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
                                let move_cursor_left = format!("\x1b[{}D", cursor_pos);
                                if let Err(e) = tmanager.write(move_cursor_left.as_bytes()) {
                                    eprintln!("{}", e);
                                    return Err(());
                                }
                                let clear_line_cmd = format!("{}\x1b[K", line);
                                if let Err(e) = tmanager.write(clear_line_cmd.as_bytes()) {
                                    eprintln!("{}", e);
                                    return Err(());
                                }
                                let chars_after_new_cursor =
                                    line.chars().skip(cursor_pos + 1).count();
                                if chars_after_new_cursor > 0 {
                                    let move_cursor_left =
                                        format!("\x1b[{}D", chars_after_new_cursor);
                                    if let Err(e) = tmanager.write(move_cursor_left.as_bytes()) {
                                        eprintln!("{}", e);
                                        return Err(());
                                    }
                                }
                            }
                            cursor_pos += 1;
                            if let Err(e) = tmanager.flush() {
                                eprintln!("{}", e);
                                return Err(());
                            };
                        }
                    }
                }
            },
        }
    }
}
