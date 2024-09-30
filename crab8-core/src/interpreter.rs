use std::{
    fs, io,
    path::Path,
    time::{Duration, Instant},
};

use rand::{thread_rng, Rng};

use crate::{Chip8Beeper, Chip8Display, Chip8Keyboard, Chip8State};

struct Timer {
    interval: Duration,
    last_tick: Instant,
}

impl Timer {
    fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_tick: Instant::now(),
        }
    }

    fn tick(&mut self) -> bool {
        if self.last_tick.elapsed() >= self.interval {
            self.last_tick += self.interval;
            true
        } else {
            false
        }
    }
}

pub struct Chip8Interpreter<D: Chip8Display, K: Chip8Keyboard, B: Chip8Beeper> {
    pub max_clock_speed: u32,
    pub display: D,
    pub keyboard: K,
    pub beeper: B,
}

impl<D: Chip8Display, K: Chip8Keyboard, B: Chip8Beeper> Chip8Interpreter<D, K, B> {
    pub fn new(max_clock_speed: u32, display: D, keyboard: K, beeper: B) -> Self {
        Self {
            max_clock_speed,
            display,
            keyboard,
            beeper,
        }
    }

    pub fn run<P: AsRef<Path>>(self, path: P) -> io::Result<()> {
        let program = fs::read(path).expect("Could not read file.");
        self.run_program(&program)
    }
    pub fn run_program(mut self, program: &[u8]) -> io::Result<()> {
        let mut state = Chip8State::default();

        state.load_program(program);

        let cpu_frame_time_micros = (1_000_000. / self.max_clock_speed as f64) as u64;
        let mut next_cpu_frame = Instant::now() + Duration::from_micros(cpu_frame_time_micros);
        let mut timer = Timer::new(Duration::from_secs_f32(1. / 60.));

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

            let immediate_value = byte_b;

            match [nibble_0, nibble_1, nibble_2, nibble_3] {
                //clear display
                [0x0, 0x0, 0xE, 0x0] => {
                    self.display.clear()?;
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
                [0x3, vx, _, _] => {
                    if state.register(vx) == immediate_value {
                        state.program_counter += 2;
                    }
                }
                //skip if Vx != NN
                [0x4, vx, _, _] => {
                    if state.register(vx) != immediate_value {
                        state.program_counter += 2;
                    }
                }
                //skip if Vx == Vy
                [0x5, vx, vy, 0x0] => {
                    if state.register(vx) == state.register(vy) {
                        state.program_counter += 2;
                    }
                }
                //Vx = value
                [0x6, vx, _, _] => *state.register_mut(vx) = immediate_value,
                //Vx += value
                [0x7, vx, _, _] => {
                    *state.register_mut(vx) = state.register(vx).wrapping_add(immediate_value)
                }
                //Vx = Vy
                [0x8, vx, vy, 0x0] => *state.register_mut(vx) = state.register(vy),
                //Vx |= Vy
                [0x8, vx, vy, 0x1] => *state.register_mut(vx) |= state.register(vy),
                //Vx &= Vy
                [0x8, vx, vy, 0x2] => *state.register_mut(vx) &= state.register(vy),
                //Vx ^= Vy
                [0x8, vx, vy, 0x3] => *state.register_mut(vx) ^= state.register(vy),
                //Vx += Vy
                [0x8, vx, vy, 0x4] => {
                    let (result, overflow) = state.register(vx).overflowing_add(state.register(vy));
                    *state.register_mut(vx) = result;
                    state.set_flag(overflow);
                }
                //Vx -= Vy
                [0x8, vx, vy, 0x5] => {
                    let (result, borrow) = state.register(vx).overflowing_sub(state.register(vy));
                    *state.register_mut(vx) = result;
                    state.set_flag(!borrow);
                }
                //Vx >>= 1
                [0x8, vx, _, 0x6] => {
                    let (result, borrow) = state.register(vx).overflowing_shr(1);
                    *state.register_mut(vx) = result;
                    state.set_flag(!borrow);
                }
                //Vx = Vy - Vx
                [0x8, vx, vy, 0x7] => {
                    let (result, borrow) = state.register(vy).overflowing_sub(state.register(vx));
                    *state.register_mut(vx) = result;
                    state.set_flag(!borrow);
                }
                //Vx <<= 1
                [0x8, vx, _, 0xE] => {
                    let (result, borrow) = state.register(vx).overflowing_shl(1);
                    *state.register_mut(vx) = result;
                    state.set_flag(!borrow);
                }
                // Skip if Vx != Vy
                [0x9, vx, vy, 0x0] => {
                    if state.register(vx) != state.register(vy) {
                        state.program_counter += 2;
                    }
                }
                //I = address
                [0xA, _, _, _] => state.index_register = address,
                // Jump to NNN + v0
                [0xB, _, _, _] => state.program_counter = state.register(0x0) as u16 + address,
                // Vx = rand() & NN
                [0xC, vx, _, _] => *state.register_mut(vx) = immediate_value & rng.gen::<u8>(),
                //Display sprite
                [0xD, vx, vy, _] => {
                    let vx = state.register(vx);
                    let vy = state.register(vy);
                    let data = &state.ram[state.index_register as usize
                        ..state.index_register as usize + nibble_3 as usize];

                    let flag = self.display.draw(vx, vy, data)?;

                    state.set_flag(flag);
                }
                // skip if key()
                [0xE, vx, 0x9, 0xE] => {
                    if self.keyboard.is_key_down(state.register(vx)) {
                        state.program_counter += 2;
                    }
                }
                // skip if !key()
                [0xE, vx, 0xA, 0x1] => {
                    if !self.keyboard.is_key_down(state.register(vx)) {
                        state.program_counter += 2;
                    }
                }
                // Vx = delay timer
                [0xF, vx, 0x0, 0x7] => {
                    *state.register_mut(vx) = state.delay_timer;
                }
                // Vx = get_key()
                [0xF, vx, 0x0, 0xA] => {
                    if let Some(last_key) = self.keyboard.last_key_pressed() {
                        *state.register_mut(vx) = last_key;
                    } else {
                        state.program_counter -= 2;
                    }
                }
                // Set delay timer to vx
                [0xF, vx, 0x1, 0x5] => {
                    state.delay_timer = state.register(vx);
                }
                // Set sound timer to vx
                [0xF, vx, 0x1, 0x8] => {
                    state.sound_timer = state.register(vx);
                }
                // I += Vx
                [0xF, vx, 0x1, 0xE] => {
                    let (result, overflow) = state
                        .index_register
                        .overflowing_add(state.register(vx) as u16);
                    state.index_register = result;
                    state.set_flag(overflow);
                }
                // I = Vx'th character index
                [0xF, vx, 0x2, 0x9] => {
                    state.index_register = state.register(vx) as u16 * 5;
                }
                // Convert and store Vx to decimal
                [0xF, vx, 0x3, 0x3] => {
                    let value = state.register(vx);
                    state.ram[state.index_register as usize] = value / 100;
                    state.ram[state.index_register as usize + 1] = value / 10 % 10;
                    state.ram[state.index_register as usize + 2] = value % 10;
                }
                // Store everything up until Vx
                [0xF, vx, 0x5, 0x5] => {
                    for i in 0..=vx {
                        state.ram[(state.index_register + i as u16) as usize] = state.register(i);
                    }
                }
                // Load everything up until Vx
                [0xF, vx, 0x6, 0x5] => {
                    for i in 0..=vx {
                        *state.register_mut(i) =
                            state.ram[(state.index_register + i as u16) as usize];
                    }
                }
                _ => {
                    self.display.clear()?;
                    self.display.flush()?;
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
                    self.beeper.play();
                } else {
                    self.beeper.pause();
                }
                self.display.flush()?;
            }

            let now = Instant::now();

            let time_left = next_cpu_frame - now;

            let time_left = time_left.max(Duration::ZERO);
            next_cpu_frame += Duration::from_micros(cpu_frame_time_micros);

            self.keyboard
                .update_keystates(time_left.as_micros() as u64)?;
        }
    }
}
