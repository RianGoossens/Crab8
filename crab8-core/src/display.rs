use std::io;

pub trait Chip8Display {
    fn new() -> Self;
    fn clear(&mut self) -> io::Result<()>;
    fn draw(&mut self, x: u8, y: u8, data: &[u8]) -> io::Result<bool>;
    fn flush(&mut self) -> io::Result<()>;
}
