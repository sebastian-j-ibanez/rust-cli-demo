use term_manager::TermManager;

pub type Result<T> = std::result::Result<T, Error>;
pub type ProcessFunc = fn(String) -> Result<String>;
pub type LineEndFunc = fn(String) -> bool;

pub enum Error {
    InitFail(String),
    IoFlush(String),
    IoRead(String),
    IoWrite(String),
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

    pub fn get_line(&mut self) -> Result<String> {
        loop {
            let mut buf = [0u8; 1];
            match self.tmanager.read(&mut buf) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Error reading from tmanager.stdin: {:?}", e);
                    return Err(Error::IoFlush(format!("unable to flush stdout")));
                }
            };
            let c = buf[0];

            match self.input_state {
                InputState::Escape => {
                    self.escape_buffer.push(c);
                    match c {
                        b'[' => {
                            self.input_state = InputState::BracketedEscape;
                        }
                        _ => {
                            self.input_state = InputState::Normal;
                            self.escape_buffer.clear();
                        }
                    }
                }
                InputState::BracketedEscape => {
                    self.escape_buffer.push(c);
                    match c {
                        // Up
                        b'A' => {
                            if self.lines.len() > 0 && self.lines_pos > 0 {
                                self.line = self.lines[self.lines_pos - 1].clone();
                                self.lines_pos -= 1;
                                print!("\r{}{}\x1b[K", self.prompt, self.line);
                                if let Err(e) = self.tmanager.flush() {
                                    eprintln!("{}", e);
                                    return Err(Error::IoFlush(format!("unable to flush stdout")));
                                };
                                self.cursor_pos = 0;
                            }
                            self.input_state = InputState::Normal;
                            self.escape_buffer.clear();
                        }
                        // Down
                        b'B' => {
                            if self.lines.len() > 0 && (self.lines_pos + 1) < self.lines.len() {
                                self.lines_pos += 1;
                                self.line = self.lines[self.lines_pos].clone();
                                print!("\r{}{}\x1b[K", self.prompt, self.line);
                                if let Err(e) = self.tmanager.flush() {
                                    eprintln!("{}", e);
                                    return Err(Error::IoFlush(format!("unable to flush stdout")));
                                };
                                self.cursor_pos = 0;
                            }
                            self.input_state = InputState::Normal;
                            self.escape_buffer.clear();
                        }
                        // Right
                        b'C' => {
                            if self.cursor_pos < self.line.chars().count() {
                                if let Err(e) = self.tmanager.write("\x1b[1C".as_bytes()) {
                                    eprintln!("{}", e);
                                    return Err(Error::IoWrite(format!(
                                        "unable to write to stdout"
                                    )));
                                }

                                if let Err(e) = self.tmanager.flush() {
                                    eprintln!("{}", e);
                                    return Err(Error::IoFlush(format!("unable to flush stdout")));
                                }

                                self.cursor_pos += 1;
                            }
                            self.input_state = InputState::Normal;
                            self.escape_buffer.clear();
                        }
                        // Left
                        b'D' => {
                            if self.cursor_pos > 0 {
                                if let Err(e) = self.tmanager.write("\x1b[1D".as_bytes()) {
                                    eprintln!("{}", e);
                                    return Err(Error::IoWrite(format!(
                                        "unable to write to stdout"
                                    )));
                                }
                                if let Err(e) = self.tmanager.flush() {
                                    eprintln!("{}", e);
                                    return Err(Error::IoFlush(format!("unable to flush stdout")));
                                };
                                self.cursor_pos -= 1;
                            }
                            self.input_state = InputState::Normal;
                            self.escape_buffer.clear();
                        }
                        _ => {}
                    }
                }
                InputState::Normal => match c {
                    b'\x1b' => {
                        self.input_state = InputState::Escape;
                        self.escape_buffer.clear();
                    }
                    b'q' | b'\x03' => break,
                    b'\n' | b'\r' => {
                        println!("\r\n{}", self.line);
                        self.lines.push(self.line.clone());
                        self.lines_pos += 1;
                        self.line.clear();
                        self.cursor_pos = 0;
                        print!("{}", self.prompt);
                        if let Err(e) = self.tmanager.flush() {
                            eprintln!("{}", e);
                            return Err(Error::IoFlush(format!("unable to flush stdout")));
                        };
                    }
                    b'\x08' | b'\x7f' => {
                        if self.cursor_pos > 0 {
                            let mut byte_idx_to_remove = 0;
                            let mut current_char_count = 0;
                            for (idx, _) in self.line.char_indices() {
                                if current_char_count == self.cursor_pos - 1 {
                                    byte_idx_to_remove = idx;
                                    break;
                                }
                                current_char_count += 1;
                            }
                            self.line.remove(byte_idx_to_remove);

                            self.cursor_pos -= 1;

                            if let Err(e) = self.tmanager.write("\x1b[1D".as_bytes()) {
                                eprintln!("{}", e);
                                return Err(Error::IoWrite(format!("unable to write to stdout")));
                            }
                            let clear_line_cmd =
                                format!("{}\x1b[K", &self.line[byte_idx_to_remove..]);
                            if let Err(e) = self.tmanager.write(clear_line_cmd.as_bytes()) {
                                eprintln!("{}", e);
                                return Err(Error::IoWrite(format!("unable to write to stdout")));
                            }
                            let chars_after_cursor =
                                self.line.chars().skip(self.cursor_pos).count();
                            if chars_after_cursor > 0 {
                                let move_cursor_left = format!("\x1b[{}D", chars_after_cursor);
                                if let Err(e) = self.tmanager.write(move_cursor_left.as_bytes()) {
                                    eprintln!("{}", e);
                                    return Err(Error::IoWrite(format!(
                                        "unable to write to stdout"
                                    )));
                                }
                            }
                            if let Err(e) = self.tmanager.flush() {
                                eprintln!("{}", e);
                                return Err(Error::IoFlush(format!("unable to flush stdout")));
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
                                if self.cursor_pos == self.line.chars().count() {
                                    print!("{}", char_byte);
                                    self.line.push(char_byte);
                                } else {
                                    let mut byte_idx = 0;
                                    for (idx, _) in self.line.char_indices().take(self.cursor_pos) {
                                        byte_idx = idx;
                                    }
                                    self.line.insert(byte_idx, char_byte);
                                    let move_cursor_left = format!("\x1b[{}D", self.cursor_pos);
                                    if let Err(e) = self.tmanager.write(move_cursor_left.as_bytes())
                                    {
                                        eprintln!("{}", e);
                                        return Err(Error::IoWrite(format!(
                                            "unable to write to stdout"
                                        )));
                                    }
                                    let clear_line_cmd = format!("{}\x1b[K", self.line);
                                    if let Err(e) = self.tmanager.write(clear_line_cmd.as_bytes()) {
                                        eprintln!("{}", e);
                                        return Err(Error::IoWrite(format!(
                                            "unable to write to stdout"
                                        )));
                                    }
                                    let chars_after_new_cursor =
                                        self.line.chars().skip(self.cursor_pos + 1).count();
                                    if chars_after_new_cursor > 0 {
                                        let move_cursor_left =
                                            format!("\x1b[{}D", chars_after_new_cursor);
                                        if let Err(e) =
                                            self.tmanager.write(move_cursor_left.as_bytes())
                                        {
                                            eprintln!("{}", e);
                                            return Err(Error::IoWrite(format!(
                                                "unable to write to stdout"
                                            )));
                                        }
                                    }
                                }
                                self.cursor_pos += 1;
                                if let Err(e) = self.tmanager.flush() {
                                    eprintln!("{}", e);
                                    return Err(Error::IoFlush(format!("unable to flush stdout")));
                                };
                            }
                        }
                    }
                },
            }
        }

        Ok(self.line.clone())
    }
}
