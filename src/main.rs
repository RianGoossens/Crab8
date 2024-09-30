use cpal::{FromSample, Sample, Stream, StreamConfig};
use crab8_core::{Chip8Beeper, Chip8Display, Chip8Interpreter, Chip8Keyboard};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute, queue,
    style::{self, Stylize},
    terminal,
};
use std::{
    f32::consts::TAU,
    fs,
    io::{self, stdout, ErrorKind, Stdout, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

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

pub struct CpalBeeper {
    stream: Stream,
}

impl Chip8Beeper for CpalBeeper {
    fn new(volume: f32) -> Self {
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

    fn play(&mut self) {
        self.stream.play().unwrap()
    }

    fn pause(&mut self) {
        self.stream.pause().unwrap()
    }
}

impl Drop for CpalBeeper {
    fn drop(&mut self) {
        self.pause();
    }
}

fn main() -> io::Result<()> {
    let path = rom_selector("./testroms")?;

    let display = CrossTermDisplay::new();
    let keyboard = CrossTermKeyboard::new();
    let beeper = CpalBeeper::new(0.1);
    let interpreter = Chip8Interpreter::new(1000, display, keyboard, beeper);

    interpreter.run(path)?;

    Ok(())
}
