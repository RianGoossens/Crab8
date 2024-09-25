use crossterm::{
    cursor, execute, queue,
    style::{self, Stylize},
    terminal,
};
use std::{
    fs,
    io::{self, stdout, Write},
    thread,
    time::{Duration, Instant},
};

pub struct Chip8State {
    pub data_registers: [u8; 16],
    pub index_register: u16,
    pub program_counter: u16,
    pub stack_pointer: u8,
    pub ram: [u8; 4096],
    pub stack: [u16; 256],
    pub display: [bool; 64 * 32],
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
            display: [false; 64 * 32],
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
    pub fn run(&self, path: &str) -> io::Result<()> {
        let program = fs::read(path).expect("Could not read file.");
        let mut stdout = stdout();

        let mut state = Chip8State::default();

        state.load_program(program);

        let cpu_frame_time = 1. / self.max_clock_speed as f32;

        execute!(
            stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::Hide
        )?;

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
                [0x0, 0x0, 0xE, 0x0] => state.display = [false; 64 * 32],
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
                [0x5, _, _, _] => {
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
                //I = address
                [0xA, _, _, _] => state.index_register = address,
                //Display sprite
                [0xD, _, _, _] => {
                    let vx = state.register(vx);
                    let vy = state.register(vy);
                    let mut pixel_cleared = false;
                    for i in 0..nibble_3 {
                        let to_draw = state.ram[state.index_register as usize + i as usize];
                        let row = vy + i;
                        for j in 0..8 {
                            let col = vx + j;
                            let flip = to_draw & (1 << (7 - j)) > 0;

                            let display_index = (row as usize) * 64 + col as usize;
                            if state.display[display_index] && flip {
                                pixel_cleared = true;
                            }
                            state.display[display_index] ^= flip;
                            queue!(stdout, cursor::MoveTo(col as u16 * 2, row as u16))?;
                            if state.display[display_index] {
                                queue!(stdout, style::PrintStyledContent("██".yellow()))?
                            } else {
                                queue!(stdout, style::PrintStyledContent("  ".black()))?
                            }
                        }
                    }

                    state.set_flag(pixel_cleared);

                    stdout.flush()?;
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
                _ => {}
            }

            let time_passed = start_cpu_frame_time.elapsed().as_secs_f32();

            let wait_time = (cpu_frame_time - time_passed).max(0.);

            if wait_time > 0. {
                thread::sleep(Duration::from_secs_f32(wait_time));
            }
        }
    }
}

fn main() -> io::Result<()> {
    let interpreter = Chip8Interpreter {
        max_clock_speed: 700,
    };

    interpreter.run("testroms/3-corax+.ch8")?;

    Ok(())
}
