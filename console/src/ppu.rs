pub struct PPU {
    mode: u8,
}

impl PPU {
    pub fn new() -> Self {
        PPU {
            mode: 0,
        }
    }
}
