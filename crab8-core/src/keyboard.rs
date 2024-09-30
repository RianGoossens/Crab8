use std::io;

pub trait Chip8Keyboard {
    fn new() -> Self;
    fn update_keystates(&mut self, max_duration_microseconds: u64) -> io::Result<()>;
    fn is_key_down(&self, key: u8) -> bool;
    fn last_key_pressed(&self) -> Option<u8>;
}
