use crossterm::{
    cursor, queue,
    style::{self, Stylize},
    terminal,
};
use std::{
    fs,
    io::{self, stdout, Write},
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
}

pub struct Chip8Interpreter;

impl Chip8Interpreter {
    pub fn run(&self, path: &str) -> io::Result<()> {
        let program = fs::read(path).expect("Could not read file.");
        let mut stdout = stdout();

        let mut state = Chip8State::default();

        state.load_program(program);

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

            let register_x = nibble_1;
            let register_y = nibble_2;
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
                    if state.data_registers[register_x as usize] == byte_b {
                        state.program_counter += 2;
                    }
                }
                //skip if Vx != NN
                [0x4, _, _, _] => {
                    if state.data_registers[register_x as usize] != byte_b {
                        state.program_counter += 2;
                    }
                }
                //skip if Vx == Vy
                [0x5, _, _, _] => {
                    if state.data_registers[register_x as usize]
                        == state.data_registers[register_y as usize]
                    {
                        state.program_counter += 2;
                    }
                }
                //Vx = value
                [0x6, _, _, _] => state.data_registers[register_x as usize] = immediate_value,
                //Vx += value
                [0x7, _, _, _] => {
                    state.data_registers[register_x as usize] =
                        state.data_registers[register_x as usize].wrapping_add(immediate_value)
                }
                //Vx = Vy
                [0x8, _, _, 0x0] => {
                    state.data_registers[register_x as usize] =
                        state.data_registers[register_y as usize]
                }
                //Vx |= Vy
                [0x8, _, _, 0x1] => {
                    state.data_registers[register_x as usize] |=
                        state.data_registers[register_y as usize]
                }
                //Vx &= Vy
                [0x8, _, _, 0x2] => {
                    state.data_registers[register_x as usize] &=
                        state.data_registers[register_y as usize]
                }
                //Vx ^= Vy
                [0x8, _, _, 0x3] => {
                    state.data_registers[register_x as usize] ^=
                        state.data_registers[register_y as usize]
                }
                //Vx += Vy
                [0x8, _, _, 0x4] => {
                    let (result, overflow) = state.data_registers[register_x as usize]
                        .overflowing_add(state.data_registers[register_y as usize]);
                    state.data_registers[register_x as usize] = result;
                    state.data_registers[0xF] = overflow as u8;
                }
                //Vx -= Vy
                [0x8, _, _, 0x5] => {
                    let (result, borrow) = state.data_registers[register_x as usize]
                        .overflowing_sub(state.data_registers[register_y as usize]);
                    state.data_registers[register_x as usize] = result;
                    state.data_registers[0xF] = !borrow as u8;
                }
                //Vx >>= 1
                [0x8, _, _, 0x6] => {
                    let (result, borrow) =
                        state.data_registers[register_x as usize].overflowing_shr(1);
                    state.data_registers[register_x as usize] = result;
                    state.data_registers[0xF] = !borrow as u8;
                }
                //Vx = Vy - Vx
                [0x8, _, _, 0x7] => {
                    let (result, borrow) = state.data_registers[register_y as usize]
                        .overflowing_sub(state.data_registers[register_x as usize]);
                    state.data_registers[register_x as usize] = result;
                    state.data_registers[0xF] = !borrow as u8;
                }
                //Vx <<= 1
                [0x8, _, _, 0xE] => {
                    let (result, borrow) =
                        state.data_registers[register_x as usize].overflowing_shl(1);
                    state.data_registers[register_x as usize] = result;
                    state.data_registers[0xF] = !borrow as u8;
                }
                //I = address
                [0xA, _, _, _] => state.index_register = address,
                //Display sprite
                [0xD, _, _, _] => {
                    let vx = state.data_registers[register_x as usize];
                    let vy = state.data_registers[register_y as usize];
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
                        }
                    }
                    state.data_registers[0xF] = pixel_cleared as u8;

                    queue!(stdout, terminal::Clear(terminal::ClearType::All))?;

                    for row in 0..32 {
                        for col in 0..64 {
                            let display_index = (row as usize) * 64 + col as usize;
                            queue!(stdout, cursor::MoveTo(col * 2, row))?;
                            if state.display[display_index] {
                                queue!(stdout, style::PrintStyledContent("██".yellow()))?
                            } else {
                                queue!(stdout, style::PrintStyledContent("  ".black()))?
                            }
                        }
                    }

                    stdout.flush()?;
                }
                // I += Vx
                [0xF, _, 0x1, 0xE] => {
                    let (result, overflow) = state
                        .index_register
                        .overflowing_add(state.data_registers[register_x as usize] as u16);
                    state.index_register = result;
                    state.data_registers[0xF] = overflow as u8;
                }
                // Convert and store Vx to decimal
                [0xF, _, 0x3, 0x3] => {
                    let value = state.data_registers[register_x as usize];
                    state.ram[state.index_register as usize] = value / 100;
                    state.ram[state.index_register as usize + 1] = value / 10 % 10;
                    state.ram[state.index_register as usize + 2] = value % 10;
                }
                // Store everything up until Vx
                [0xF, _, 0x5, 0x5] => {
                    for i in 0..=register_x {
                        state.ram[(state.index_register + i as u16) as usize] =
                            state.data_registers[i as usize];
                    }
                }
                // Load everything up until Vx
                [0xF, _, 0x6, 0x5] => {
                    for i in 0..=register_x {
                        state.data_registers[i as usize] =
                            state.ram[(state.index_register + i as u16) as usize];
                    }
                }
                _ => {}
            }
        }
    }
}

fn main() -> io::Result<()> {
    let interpreter = Chip8Interpreter;

    interpreter.run("testroms/4-flags.ch8")?;

    Ok(())
}
