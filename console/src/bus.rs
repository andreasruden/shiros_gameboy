use crate::Cartridge;

pub struct Bus {
    // FIXME: these things should not be owned by the Bus if avoidable
    pub cartridge: Option<Cartridge>,
    pub wram: [u8; 0x2000],
}

impl Bus {
    pub fn new() -> Self {
        Self {
            cartridge: None,
            wram: [0; 0x2000],
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        if (0x0000..0x4000).contains(&addr) {
            return self.cartridge.as_ref().unwrap().rom[addr as usize];
        }
        todo!()
    }

    pub fn write(&mut self, addr: u16, byte: u8) {
        match addr {
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = byte,
            _ => todo!(),
        };
    }
}
