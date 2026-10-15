use crate::Cartridge;

pub struct CpuBus<'a> {
    pub cartridge: &'a mut Option<Cartridge>,
    pub wram: &'a mut [u8; 0x2000],
}

pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, byte: u8);
}

impl Bus for CpuBus<'_> {
    fn read(&mut self, addr: u16) -> u8 {
        if (0x0000..0x4000).contains(&addr) {
            return match self.cartridge.as_ref() {
                None => 0,
                Some(c) => c.rom[addr as usize],
            };
        }
        todo!()
    }

    fn write(&mut self, addr: u16, byte: u8) {
        match addr {
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = byte,
            _ => todo!(),
        };
    }
}
