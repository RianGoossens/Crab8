#[derive(Debug, Clone, Default)]
pub struct State {
    pub v0: u8,
    pub v1: u8,
    pub v2: u8,
    pub v3: u8,
    pub v4: u8,
    pub v5: u8,
    pub v6: u8,
    pub v7: u8,
    pub v8: u8,
    pub v9: u8,
    pub va: u8,
    pub vb: u8,
    pub vc: u8,
    pub vd: u8,
    pub ve: u8,
    pub vf: u8,
    pub output: u8,
}

pub trait OpCode {
    fn apply(&self, state: &mut State);
}

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
