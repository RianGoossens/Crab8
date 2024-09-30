pub struct Chip8State {
    pub data_registers: [u8; 16],
    pub index_register: u16,
    pub program_counter: u16,
    pub stack_pointer: u8,
    pub ram: [u8; 4096],
    pub stack: [u16; 256],
    pub delay_timer: u8,
    pub sound_timer: u8,
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
            delay_timer: 0,
            sound_timer: 0,
        }
    }
}

const FONT: [u8; 16 * 5] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

impl Chip8State {
    pub fn load_font_data(&mut self, fonts: &[u8]) {
        for (i, byte) in fonts.iter().enumerate() {
            self.ram[i] = *byte;
        }
    }
    pub fn load_program(&mut self, program: &[u8]) {
        self.load_font_data(&FONT);
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
