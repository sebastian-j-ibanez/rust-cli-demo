use term_manager::TermManager;

type Result<T> = std::result::Result<T, Error>;

pub enum Error {
    InitFail(String),
}

enum InputState {
    Normal,
    Escape,
    BracketedEscape,
}

struct Repl {
    tmanager: TermManager,
    prompt: String,
    line: String,
    cursor_pos: usize,
    lines: Vec<String>,
    lines_pos: usize,
    escape_buffer: Vec<u8>,
    input_state: InputState,
}

impl Repl {
    pub fn new(prompt: String) -> Result<Self> {
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
            prompt,
            line,
            cursor_pos,
            lines,
            lines_pos,
            escape_buffer,
            input_state,
        })
    }

    pub fn start() -> Result<()> {
        todo!()
    }
}
