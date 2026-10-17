use crate::Cartridge;

pub struct IoRegisters {
    i_f: u8,  // interrupt flag
    serial_1: u8,
    serial_2: u8,

    // LCD registers
    lcd_ctrl: u8,
    lcd_scy: u8,
    lcd_scx: u8,
    lcd_stat: u8,
    lcd_y: u8,
}

pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, byte: u8);
}

pub struct CpuBus<'a> {
    pub cartridge: &'a mut Option<Cartridge>,
    pub wram: &'a mut [u8; 0x2000],
    pub io_reg: &'a mut IoRegisters,
    pub ie_reg: &'a mut u8,
    pub hram: &'a mut [u8; 127],
}

impl IoRegisters {
    pub fn new() -> Self {
        IoRegisters {
            i_f: 0,
            serial_1: 0,
            serial_2: 0,
            lcd_ctrl: 0,
            lcd_scy: 0,
            lcd_scx: 0,
            lcd_stat: 0,
            lcd_y: 0,
        }
    }

    pub fn get(&mut self, addr: u16) -> u8 {
        match addr {
            0xFF0F => self.i_f,
            0xFF01 => self.serial_1,
            0xFF02 => self.serial_2,
            0xFF40 => self.lcd_ctrl,
            0xFF41 => self.lcd_stat,
            0xFF42 => self.lcd_scy,
            0xFF43 => self.lcd_scx,
            0xFF44 => self.lcd_y,
            _ => todo!("IoRegisters::get for {addr:04X} not handled"),
        }
    }

    pub fn set(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF0F => self.i_f = val,
            0xFF01 => self.serial_1 = val,
            0xFF02 => self.serial_2 = val,
            0xFF40 => self.lcd_ctrl = val,
            0xFF41 => self.lcd_stat = val,
            0xFF42 => self.lcd_scy = val,
            0xFF43 => self.lcd_scx = val,
            0xFF44 => { /* read only */ },
            _ => todo!("IoRegisters::set for {addr:04X} not handled"),
        }
    }
}

impl Bus for CpuBus<'_> {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => match self.cartridge.as_ref() {
                None => 0,
                Some(c) => c.rom[addr as usize],
            },
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
            0xFF00..=0xFF7F => self.io_reg.get(addr),
            0xFF80..=0xFFFe => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => *self.ie_reg,
            _ => todo!("CpuBus::read for {addr:04X} not handled"),
        }
    }

    fn write(&mut self, addr: u16, byte: u8) {
        match addr {
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = byte,
            0xFF00..=0xFF7F => self.io_reg.set(addr, byte),
            0xFF80..=0xFFFe => self.hram[(addr - 0xFF80) as usize] = byte,
            0xFFFF => *self.ie_reg = byte,
            _ => todo!("CpuBus::write for {addr:04X} not handled"),
        };
    }
}
