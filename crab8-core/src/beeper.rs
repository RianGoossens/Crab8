pub trait Chip8Beeper {
    fn new(volume: f32) -> Self;
    fn play(&mut self);
    fn pause(&mut self);
}
