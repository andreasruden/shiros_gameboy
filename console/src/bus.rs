use crate::Cartridge;

pub struct Bus {
    pub cartridge: Option<Cartridge>,
}

impl Bus {
    pub fn new() -> Self {
        Self {
            cartridge: None,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        if (0x0000..0x4000).contains(&addr) {
            return self.cartridge.as_ref().unwrap().rom[addr as usize];
        }
        todo!()
    }

    pub fn write(&mut self, addr: u16, byte: u8) {
        todo!()
    }
}
