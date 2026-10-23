// Audio Processing Unit

pub struct APU {
    regs: [u8; 0x30],  // $FF10-$FF3F
}

impl APU {
    pub fn new() -> Self {
        APU {
            regs: [0; 0x30],
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        let i = (addr - 0xFF10) as usize;
        self.regs[i]
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        let i = (addr - 0xFF10) as usize;
        self.regs[i] = val;
    }
}
