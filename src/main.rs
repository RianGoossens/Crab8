use cpal::{FromSample, Sample, Stream, StreamConfig};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute, queue,
    style::{self, Stylize},
    terminal,
};
use rand::{thread_rng, Rng};
use std::{
    f32::consts::TAU,
    fs,
    io::{self, stdout, ErrorKind, Stdout, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub mod state;

pub use state::Chip8State;

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
        queue!(
            self.stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0)
        )
    }

    // fn draw(&mut self, x: u8, y: u8, data: &[u8]) -> io::Result<bool> {
    //     let mut pixel_cleared = false;
    //     for (i, to_draw) in data.iter().enumerate() {
    //         let row = y as usize + i;
    //         for j in 0..8 {
    //             let col = x + j;
    //             let flip = to_draw & (1 << (7 - j)) > 0;

    //             let display_index = row * 64 + col as usize;
    //             if display_index >= self.display.len() {
    //                 break;
    //             }
    //             if self.display[display_index] && flip {
    //                 pixel_cleared = true;
    //             }
    //             self.display[display_index] ^= flip;
    //             queue!(self.stdout, cursor::MoveTo(col as u16 * 2, row as u16))?;
    //             if self.display[display_index] {
    //                 queue!(self.stdout, style::PrintStyledContent("██".yellow()))?
    //             } else {
    //                 queue!(self.stdout, style::PrintStyledContent("  ".black()))?
    //             }
    //         }
    //     }
    //     Ok(pixel_cleared)
    // }
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
            }
        }
        for hrow in 0..16 {
            for hcol in 0..32 {
                let mut block_index: u8 = 0;

                for i in 0..=1 {
                    for j in 0..=1 {
                        let display_index = ((2 * hrow + i) * 64 + (2 * hcol + j)) as usize;
                        if self.display[display_index] {
                            block_index ^= 1 << (i * 2 + j);
                        }
                    }
                }
                assert!(block_index < 16, "{block_index:04b}");

                const BLOCK_CHARACTERS: [&str; 16] = [
                    "  ", "▀ ", " ▀", "▀▀", "▄ ", "█ ", "▄▀", "█▀", " ▄", "▀▄", " █", "▀█", "▄▄",
                    "█▄", "▄█", "██",
                ];
                let block_character = BLOCK_CHARACTERS[block_index as usize];
                queue!(
                    self.stdout,
                    cursor::MoveTo(hcol as u16 * 2, hrow as u16),
                    style::PrintStyledContent(block_character.yellow())
                )?;
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
    fn update_keystates(&mut self, max_duration_microseconds: u64) -> io::Result<()>;
    fn is_key_down(&self, key: u8) -> bool;
    fn last_key_pressed(&self) -> Option<u8>;
}

pub struct CrossTermKeyboard {
    key_states: u16,
    last_key_pressed: Option<u8>,
}

fn crossterm_keymap(keycode: KeyCode) -> Option<u8> {
    match keycode {
        KeyCode::Char('1') => Some(0x1),
        KeyCode::Char('2') => Some(0x2),
        KeyCode::Char('3') => Some(0x3),
        KeyCode::Char('4') => Some(0xC),
        KeyCode::Char('q') => Some(0x4),
        KeyCode::Char('w') => Some(0x5),
        KeyCode::Char('e') => Some(0x6),
        KeyCode::Char('r') => Some(0xD),
        KeyCode::Char('a') => Some(0x7),
        KeyCode::Char('s') => Some(0x8),
        KeyCode::Char('d') => Some(0x9),
        KeyCode::Char('f') => Some(0xE),
        KeyCode::Char('z') => Some(0xA),
        KeyCode::Char('x') => Some(0x0),
        KeyCode::Char('c') => Some(0xB),
        KeyCode::Char('v') => Some(0xF),
        _ => None,
    }
}

impl Chip8Keyboard for CrossTermKeyboard {
    fn new() -> Self {
        Self {
            key_states: 0,
            last_key_pressed: None,
        }
    }

    fn update_keystates(&mut self, max_duration_microseconds: u64) -> io::Result<()> {
        let start_time = Instant::now();
        self.last_key_pressed = None;
        loop {
            let leftover_time =
                max_duration_microseconds.saturating_sub(start_time.elapsed().as_micros() as u64);
            if leftover_time == 0 {
                break;
            }
            let duration = Duration::from_micros(leftover_time);
            if event::poll(duration)? {
                if let Event::Key(KeyEvent { code, kind, .. }) = event::read()? {
                    if let Some(key) = crossterm_keymap(code) {
                        match kind {
                            KeyEventKind::Press => {
                                if self.key_states & 1 << key == 0 {
                                    self.last_key_pressed = Some(key);
                                }
                                self.key_states |= 1 << key;
                            }
                            KeyEventKind::Release => self.key_states &= !(1 << key),
                            KeyEventKind::Repeat => {}
                        }
                    }
                }
            };
        }
        Ok(())
    }

    fn is_key_down(&self, key: u8) -> bool {
        self.key_states & (1 << key) > 0
    }

    fn last_key_pressed(&self) -> Option<u8> {
        self.last_key_pressed
    }
}

pub struct Timer {
    interval: Duration,
    last_tick: Instant,
}

impl Timer {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_tick: Instant::now(),
        }
    }

    pub fn tick(&mut self) -> bool {
        if self.last_tick.elapsed() >= self.interval {
            self.last_tick += self.interval;
            true
        } else {
            false
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
    pub fn run<D: Chip8Display, K: Chip8Keyboard, P: AsRef<Path>>(self, path: P) -> io::Result<()> {
        let program = fs::read(path).expect("Could not read file.");
        self.run_program::<D, K>(&program)
    }
    pub fn run_program<D: Chip8Display, K: Chip8Keyboard>(self, program: &[u8]) -> io::Result<()> {
        let mut state = Chip8State::default();

        state.load_program(program);

        let cpu_frame_time_micros = (1_000_000. / self.max_clock_speed as f64) as u64;
        let mut next_cpu_frame = Instant::now() + Duration::from_micros(cpu_frame_time_micros);
        let mut timer = Timer::new(Duration::from_secs_f32(1. / 60.));

        let mut display = D::new();
        let mut keyboard = K::new();
        let beeper = Beeper::new(0.1);
        let mut rng = thread_rng();

        loop {
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
                // Jump to NNN + v0
                [0xB, _, _, _] => state.program_counter = state.register(0x0) as u16 + address,
                // Vx = rand() & NN
                [0xC, _, _, _] => *state.register_mut(vx) = byte_b & rng.gen::<u8>(),
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
                // Vx = delay timer
                [0xF, _, 0x0, 0x7] => {
                    *state.register_mut(vx) = state.delay_timer;
                }
                // Vx = get_key()
                [0xF, _, 0x0, 0xA] => {
                    if let Some(last_key) = keyboard.last_key_pressed() {
                        *state.register_mut(vx) = last_key;
                    } else {
                        state.program_counter -= 2;
                    }
                }
                // Set delay timer to vx
                [0xF, _, 0x1, 0x5] => {
                    state.delay_timer = state.register(vx);
                }
                // Set sound timer to vx
                [0xF, _, 0x1, 0x8] => {
                    state.sound_timer = state.register(vx);
                }
                // I += Vx
                [0xF, _, 0x1, 0xE] => {
                    let (result, overflow) = state
                        .index_register
                        .overflowing_add(state.register(vx) as u16);
                    state.index_register = result;
                    state.set_flag(overflow);
                }
                // I = Vx'th character index
                [0xF, _, 0x2, 0x9] => {
                    state.index_register = state.register(vx) as u16 * 5;
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
                    display.clear()?;
                    display.flush()?;
                    panic!(
                        "Unknown instruction {:01x}{:01x}{:01x}{:01x}",
                        nibble_0, nibble_1, nibble_2, nibble_3
                    )
                }
            }

            if timer.tick() {
                if state.delay_timer > 0 {
                    state.delay_timer -= 1;
                }
                if state.sound_timer > 0 {
                    state.sound_timer -= 1;
                    beeper.play();
                } else {
                    beeper.pause();
                }
                display.flush()?;
            }

            let now = Instant::now();

            let time_left = next_cpu_frame - now;

            let time_left = time_left.max(Duration::ZERO);
            next_cpu_frame += Duration::from_micros(cpu_frame_time_micros);

            keyboard.update_keystates(time_left.as_micros() as u64)?;
        }
    }
}

fn rom_selector<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    let rom_paths = fs::read_dir(path)?;

    let paths: Vec<_> = rom_paths
        .map(|x| x.unwrap().path())
        .filter(|x| {
            if let Some(extension) = x.extension() {
                extension == "ch8"
            } else {
                false
            }
        })
        .collect();

    let mut stdout = stdout();
    execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::Hide
    )?;

    let mut selected_index: usize = 0;
    let mut scroll_value = 0;
    let mut needs_redraw = true;
    loop {
        let (_cols, rows) = terminal::size()?;
        if needs_redraw {
            queue!(
                stdout,
                terminal::Clear(terminal::ClearType::All),
                cursor::MoveTo(0, 0),
                style::PrintStyledContent(
                    "🦀🎱 Crab8 by Rian Goossens
----------------------------"
                        .bold()
                )
            )?;
            for i in 0..(rows as usize - 2) {
                let index = scroll_value + i;
                if index >= paths.len() {
                    break;
                }
                let filename = paths[index].file_name().unwrap().to_str().unwrap();
                let line = format!("> {filename}");
                let mut content = line.white();
                if scroll_value + i == selected_index {
                    content = content.black().on_white();
                }
                queue!(
                    stdout,
                    cursor::MoveTo(0, i as u16 + 2),
                    style::PrintStyledContent(content)
                )?;
            }
        }

        stdout.flush()?;
        needs_redraw = false;

        if event::poll(Duration::from_secs(1))? {
            needs_redraw = true;
            if let Event::Key(KeyEvent {
                code,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            }) = event::read()?
            {
                match code {
                    KeyCode::Char('w') | KeyCode::Up => {
                        if scroll_value == 0 && selected_index == 0 {
                            scroll_value = (paths.len() as i32 - rows as i32 + 2).max(0) as usize;
                        } else if scroll_value == selected_index {
                            scroll_value = scroll_value.saturating_sub(1);
                        }
                        if selected_index == 0 {
                            selected_index = paths.len() - 1;
                        } else {
                            selected_index -= 1;
                        }
                    }
                    KeyCode::Char('s') | KeyCode::Down => {
                        selected_index = (selected_index + 1) % paths.len();

                        if selected_index == 0 {
                            scroll_value = 0;
                        } else if selected_index >= scroll_value + rows as usize - 2 {
                            scroll_value += 1;
                        }
                    }
                    KeyCode::Enter => {
                        break;
                    }
                    KeyCode::Esc => {
                        return Err(ErrorKind::Interrupted.into());
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(paths[selected_index].clone())
}

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
pub struct Beeper {
    stream: Stream,
}

impl Beeper {
    pub fn new(volume: f32) -> Self {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("no output device available");
        let mut supported_configs_range = device
            .supported_output_configs()
            .expect("error while querying configs");
        let supported_config = supported_configs_range
            .next()
            .expect("no supported config?!")
            .with_max_sample_rate();

        let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);
        let sample_format = supported_config.sample_format();
        let config: StreamConfig = supported_config.into();

        const FREQ: u32 = 440;

        let num_samples_per_second = config.sample_rate.0;
        let num_samples_per_repetition = num_samples_per_second / FREQ;

        fn create_callback<T: Sample + FromSample<f32>>(
            volume: f32,
            num_samples_per_repetition: u32,
        ) -> impl FnMut(&mut [T], &cpal::OutputCallbackInfo) {
            let mut index = 0;
            let float_samples: Vec<_> = (0..num_samples_per_repetition)
                .map(|i| (i as f32 / num_samples_per_repetition as f32 * TAU).sin() * volume)
                .collect();
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                for sample in data {
                    *sample = T::from_sample(float_samples[index as usize]);
                    index = (index + 1) % num_samples_per_repetition;
                }
            }
        }

        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                &config,
                create_callback::<f32>(volume, num_samples_per_repetition),
                err_fn,
                None,
            ),
            SampleFormat::I16 => device.build_output_stream(
                &config,
                create_callback::<i16>(volume, num_samples_per_repetition),
                err_fn,
                None,
            ),
            SampleFormat::U16 => device.build_output_stream(
                &config,
                create_callback::<u16>(volume, num_samples_per_repetition),
                err_fn,
                None,
            ),
            SampleFormat::U8 => device.build_output_stream(
                &config,
                create_callback::<u8>(volume, num_samples_per_repetition),
                err_fn,
                None,
            ),
            sample_format => panic!("Unsupported sample format '{sample_format}'"),
        }
        .unwrap();

        Self { stream }
    }

    pub fn play(&self) {
        self.stream.play().unwrap()
    }

    pub fn pause(&self) {
        self.stream.pause().unwrap()
    }
}

impl Drop for Beeper {
    fn drop(&mut self) {
        self.pause();
    }
}

fn main() -> io::Result<()> {
    let path = rom_selector("./testroms")?;

    let interpreter = Chip8Interpreter {
        max_clock_speed: 1000,
    };

    interpreter.run::<CrossTermDisplay, CrossTermKeyboard, _>(path)?;

    Ok(())
}
