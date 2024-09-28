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
    pub fn load_program(&mut self, program: &[u8]) {
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
