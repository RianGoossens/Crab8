use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute, queue,
    style::{self, Stylize},
    terminal,
};
use std::{
    fs,
    io::{self, stdout, Stdout, Write},
    time::{Duration, Instant},
};

pub struct Chip8State {
    pub data_registers: [u8; 16],
    pub index_register: u16,
    pub program_counter: u16,
    pub stack_pointer: u8,
    pub ram: [u8; 4096],
    pub stack: [u16; 256],
}

impl Default for Chip8State {
    fn default() -> Self {
        Self {
            data_registers: [0; 16],
            index_register: 0,
            program_counter: 0x200,
            stack_pointer: 0,
            ram: [0; 4096],
            stack: [0; 256],
        }
    }
}

impl Chip8State {
    pub fn load_program(&mut self, program: Vec<u8>) {
        for (i, byte) in program.iter().enumerate() {
            self.ram[0x200 + i] = *byte;
        }
    }

    pub fn register(&self, register_index: u8) -> u8 {
        self.data_registers[register_index as usize]
    }

    pub fn register_mut(&mut self, register_index: u8) -> &mut u8 {
        &mut self.data_registers[register_index as usize]
    }

    pub fn set_flag(&mut self, flag: bool) {
        *self.register_mut(0xF) = flag as u8;
    }
}

pub trait Chip8Display {
    fn new() -> Self;
    fn clear(&mut self) -> io::Result<()>;
    fn draw(&mut self, x: u8, y: u8, data: &[u8]) -> io::Result<bool>;
    fn flush(&mut self) -> io::Result<()>;
}

pub struct CrossTermDisplay {
    stdout: Stdout,
    display: [bool; 64 * 32],
}

impl Chip8Display for CrossTermDisplay {
    fn new() -> Self {
        let mut stdout = stdout();
        execute!(
            stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::Hide
        )
        .expect("Could not use stdout");

        Self {
            stdout,
            display: [false; 64 * 32],
        }
    }

    fn clear(&mut self) -> io::Result<()> {
        self.display = [false; 64 * 32];
        queue!(self.stdout, terminal::Clear(terminal::ClearType::All))
    }

    fn draw(&mut self, x: u8, y: u8, data: &[u8]) -> io::Result<bool> {
        let mut pixel_cleared = false;
        for (i, to_draw) in data.iter().enumerate() {
            let row = y as usize + i;
            for j in 0..8 {
                let col = x + j;
                let flip = to_draw & (1 << (7 - j)) > 0;

                let display_index = row * 64 + col as usize;
                if display_index >= self.display.len() {
                    break;
                }
                if self.display[display_index] && flip {
                    pixel_cleared = true;
                }
                self.display[display_index] ^= flip;
                queue!(self.stdout, cursor::MoveTo(col as u16 * 2, row as u16))?;
                if self.display[display_index] {
                    queue!(self.stdout, style::PrintStyledContent("██".yellow()))?
                } else {
                    queue!(self.stdout, style::PrintStyledContent("  ".black()))?
                }
            }
        }
        Ok(pixel_cleared)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}

pub trait Chip8Keyboard {
    fn new() -> Self;
    fn update_keystates(&mut self, max_duration_secs: f64) -> io::Result<()>;
    fn is_key_down(&self, key: u8) -> bool;
    fn block_until_key(&mut self) -> io::Result<u8>;
}

pub struct CrossTermKeyboard {
    key_states: [bool; 16],
}

fn crossterm_keymap(keycode: KeyCode) -> Option<u8> {
    match keycode {
        KeyCode::Char('1') => Some(0x0),
        KeyCode::Char('2') => Some(0x1),
        KeyCode::Char('3') => Some(0x2),
        KeyCode::Char('4') => Some(0x3),
        KeyCode::Char('q') => Some(0x4),
        KeyCode::Char('w') => Some(0x5),
        KeyCode::Char('e') => Some(0x6),
        KeyCode::Char('r') => Some(0x7),
        KeyCode::Char('a') => Some(0x8),
        KeyCode::Char('s') => Some(0x9),
        KeyCode::Char('d') => Some(0xA),
        KeyCode::Char('f') => Some(0xB),
        KeyCode::Char('z') => Some(0xC),
        KeyCode::Char('x') => Some(0xD),
        KeyCode::Char('c') => Some(0xE),
        KeyCode::Char('v') => Some(0xF),
        _ => None,
    }
}

impl Chip8Keyboard for CrossTermKeyboard {
    fn new() -> Self {
        Self {
            key_states: [false; 16],
        }
    }

    fn update_keystates(&mut self, max_duration_secs: f64) -> io::Result<()> {
        let start_time = Instant::now();
        loop {
            let leftover_time = max_duration_secs - start_time.elapsed().as_secs_f64();
            //println!("{leftover_time}");
            if leftover_time <= 0. {
                break;
            }
            let duration = Duration::from_secs_f64(leftover_time);
            if event::poll(duration)? {
                if let Event::Key(KeyEvent { code, kind, .. }) = event::read()? {
                    if let Some(key) = crossterm_keymap(code) {
                        match kind {
                            KeyEventKind::Press => self.key_states[key as usize] = true,
                            KeyEventKind::Repeat => self.key_states[key as usize] = true,
                            KeyEventKind::Release => self.key_states[key as usize] = false,
                        }
                    }
                }
            };
        }
        Ok(())
    }

    fn is_key_down(&self, key: u8) -> bool {
        self.key_states[key as usize]
    }

    fn block_until_key(&mut self) -> io::Result<u8> {
        loop {
            let event = event::read()?;
            if let Event::Key(KeyEvent {
                code,
                kind: KeyEventKind::Press,
                ..
            }) = event
            {
                if let Some(result) = crossterm_keymap(code) {
                    return Ok(result);
                }
            }
        }
    }
}

pub struct Chip8Interpreter {
    pub max_clock_speed: u32,
}

impl Default for Chip8Interpreter {
    fn default() -> Self {
        Self {
            max_clock_speed: 700,
        }
    }
}

impl Chip8Interpreter {
    pub fn run<D: Chip8Display, K: Chip8Keyboard>(self, path: &str) -> io::Result<()> {
        let program = fs::read(path).expect("Could not read file.");

        let mut state = Chip8State::default();

        state.load_program(program);

        let cpu_frame_time = 1. / self.max_clock_speed as f64;
        let mut last_timer_frame = Instant::now();

        let mut display = D::new();
        let mut keyboard = K::new();

        loop {
            let start_cpu_frame_time = Instant::now();

            //fetch
            let byte_a = state.ram[state.program_counter as usize];
            let byte_b = state.ram[state.program_counter as usize + 1];
            state.program_counter += 2;

            //decode
            let nibble_0 = (byte_a & 0xF0) >> 4;
            let nibble_1 = byte_a & 0x0F;
            let nibble_2 = (byte_b & 0xF0) >> 4;
            let nibble_3 = byte_b & 0x0F;

            let address = ((nibble_1 as u16) << 8) | byte_b as u16;

            let vx = nibble_1;
            let vy = nibble_2;
            let immediate_value = byte_b;

            match [nibble_0, nibble_1, nibble_2, nibble_3] {
                //clear display
                [0x0, 0x0, 0xE, 0x0] => {
                    display.clear()?;
                }
                //return
                [0x0, 0x0, 0xE, 0xE] => {
                    state.program_counter = state.stack[state.stack_pointer as usize];
                    state.stack_pointer -= 1;
                }
                //jump to address
                [0x1, _, _, _] => state.program_counter = address,
                //call subroutine
                [0x2, _, _, _] => {
                    state.stack_pointer += 1;
                    state.stack[state.stack_pointer as usize] = state.program_counter;
                    state.program_counter = address;
                }
                //skip if Vx == NN
                [0x3, _, _, _] => {
                    if state.register(vx) == byte_b {
                        state.program_counter += 2;
                    }
                }
                //skip if Vx != NN
                [0x4, _, _, _] => {
                    if state.register(vx) != byte_b {
                        state.program_counter += 2;
                    }
                }
                //skip if Vx == Vy
                [0x5, _, _, 0x0] => {
                    if state.register(vx) == state.register(vy) {
                        state.program_counter += 2;
                    }
                }
                //Vx = value
                [0x6, _, _, _] => *state.register_mut(vx) = immediate_value,
                //Vx += value
                [0x7, _, _, _] => {
                    *state.register_mut(vx) = state.register(vx).wrapping_add(immediate_value)
                }
                //Vx = Vy
                [0x8, _, _, 0x0] => *state.register_mut(vx) = state.register(vy),
                //Vx |= Vy
                [0x8, _, _, 0x1] => *state.register_mut(vx) |= state.register(vy),
                //Vx &= Vy
                [0x8, _, _, 0x2] => *state.register_mut(vx) &= state.register(vy),
                //Vx ^= Vy
                [0x8, _, _, 0x3] => *state.register_mut(vx) ^= state.register(vy),
                //Vx += Vy
                [0x8, _, _, 0x4] => {
                    let (result, overflow) = state.register(vx).overflowing_add(state.register(vy));
                    *state.register_mut(vx) = result;
                    state.set_flag(overflow);
                }
                //Vx -= Vy
                [0x8, _, _, 0x5] => {
                    let (result, borrow) = state.register(vx).overflowing_sub(state.register(vy));
                    *state.register_mut(vx) = result;
                    state.set_flag(!borrow);
                }
                //Vx >>= 1
                [0x8, _, _, 0x6] => {
                    let (result, borrow) = state.register(vx).overflowing_shr(1);
                    *state.register_mut(vx) = result;
                    state.set_flag(!borrow);
                }
                //Vx = Vy - Vx
                [0x8, _, _, 0x7] => {
                    let (result, borrow) = state.register(vy).overflowing_sub(state.register(vx));
                    *state.register_mut(vx) = result;
                    state.set_flag(!borrow);
                }
                //Vx <<= 1
                [0x8, _, _, 0xE] => {
                    let (result, borrow) = state.register(vx).overflowing_shl(1);
                    *state.register_mut(vx) = result;
                    state.set_flag(!borrow);
                }
                // Skip if Vx != Vy
                [0x9, _, _, 0x0] => {
                    if state.register(vx) != state.register(vy) {
                        state.program_counter += 2;
                    }
                }
                //I = address
                [0xA, _, _, _] => state.index_register = address,
                //Display sprite
                [0xD, _, _, _] => {
                    let vx = state.register(vx);
                    let vy = state.register(vy);
                    let data = &state.ram[state.index_register as usize
                        ..state.index_register as usize + nibble_3 as usize];

                    let flag = display.draw(vx, vy, data)?;

                    state.set_flag(flag);
                }
                // skip if key()
                [0xE, _, 0x9, 0xE] => {
                    if keyboard.is_key_down(state.register(vx)) {
                        state.program_counter += 2;
                    }
                }
                // skip if !key()
                [0xE, _, 0xA, 0x1] => {
                    if !keyboard.is_key_down(state.register(vx)) {
                        state.program_counter += 2;
                    }
                }
                // Vx = get_key()
                [0xF, _, 0x0, 0xA] => {
                    *state.register_mut(vx) = keyboard.block_until_key()?;
                }
                // I += Vx
                [0xF, _, 0x1, 0xE] => {
                    let (result, overflow) = state
                        .index_register
                        .overflowing_add(state.register(vx) as u16);
                    state.index_register = result;
                    state.set_flag(overflow);
                }
                // Convert and store Vx to decimal
                [0xF, _, 0x3, 0x3] => {
                    let value = state.register(vx);
                    state.ram[state.index_register as usize] = value / 100;
                    state.ram[state.index_register as usize + 1] = value / 10 % 10;
                    state.ram[state.index_register as usize + 2] = value % 10;
                }
                // Store everything up until Vx
                [0xF, _, 0x5, 0x5] => {
                    for i in 0..=vx {
                        state.ram[(state.index_register + i as u16) as usize] = state.register(i);
                    }
                }
                // Load everything up until Vx
                [0xF, _, 0x6, 0x5] => {
                    for i in 0..=vx {
                        *state.register_mut(i) =
                            state.ram[(state.index_register + i as u16) as usize];
                    }
                }
                _ => {
                    panic!(
                        "Unknown instruction {:01x}{:01x}{:01x}{:01x}",
                        nibble_0, nibble_1, nibble_2, nibble_3
                    )
                }
            }

            if last_timer_frame.elapsed().as_secs_f32() > 1. / 60. {
                display.flush()?;
                last_timer_frame = Instant::now();
            }

            let time_passed = start_cpu_frame_time.elapsed().as_secs_f64();

            let wait_time = (cpu_frame_time - time_passed).max(0.);

            if wait_time >= 0. {
                keyboard.update_keystates(wait_time)?;
                //thread::sleep(Duration::from_secs_f32(wait_time));
            }
        }
    }
}

fn main() -> io::Result<()> {
    let interpreter = Chip8Interpreter {
        max_clock_speed: 1_000_000,
    };

    interpreter.run::<CrossTermDisplay, CrossTermKeyboard>(
        "testroms/Sierpinski [Sergey Naydenov, 2010].ch8",
    )?;

    Ok(())
}
