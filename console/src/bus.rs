use crate::{Cartridge, lcd::LCD};

pub struct HwRegisters {
    pub i_f: u8,            // $FF04 interrupt flag
    pub serial_1: u8,       // $FF01
    pub serial_2: u8,       // $FF02

    // Timer
    pub timer_tma: u8,      // $FF06 timer modulo
    pub timer_tac: u8,      // $FF07 timer control

    // Audio
    pub audio_nr50: u8,     // $FF26: master volume & VIN panning
    pub audio_nr51: u8,     // $FF25: panning
    pub audio_nr52: u8,     // $FF26: on/off

    // LCD registers
    pub lcd_ctrl: u8,       // $FF40 lcd control flags
    pub lcd_stat: u8,       // $FF41 lcd status
    pub lcd_scy: u8,        // $FF42 viewport Y position
    pub lcd_scx: u8,        // $FF43 viewport X position
    pub lcd_y: u8,          // $FF44 LCD Y coodrinate
    pub oam_dma: u8,        // $FF46 OAM DMA (Object Attribute Memory: Direct Memory Access) start
    pub bgp: u8,            // BG palette data
    pub obp0: u8,           // OBJ palette 0 data
    pub obp1: u8,           // OBJ palette 1 data

    pub ie: u8,             // $FFFF interrupt enable
}

pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, byte: u8);
}

pub struct CpuBus<'a> {
    pub cartridge: &'a mut Option<Cartridge>,
    pub vram: &'a mut [u8; 0x2000],
    pub wram: &'a mut [u8; 0x2000],
    pub hw_reg: &'a mut HwRegisters,
    pub hram: &'a mut [u8; 127],
}

pub struct PpuBus<'a> {
    pub hw_reg: &'a mut HwRegisters,
    pub lcd: &'a mut LCD,
    pub vram: &'a mut [u8; 0x2000],
}

impl HwRegisters {
    pub fn new() -> Self {
        HwRegisters {
            i_f: 0,
            serial_1: 0,
            serial_2: 0,
            timer_tma: 0,
            timer_tac: 0,
            audio_nr50: 0,
            audio_nr51: 0,
            audio_nr52: 0,
            lcd_ctrl: 0,
            lcd_stat: 0,
            lcd_scy: 0,
            lcd_scx: 0,
            lcd_y: 0,
            oam_dma: 0,
            bgp: 0,
            obp0: 0,
            obp1: 0,
            ie: 0,
        }
    }

    pub fn get(&mut self, addr: u16) -> u8 {
        match addr {
            0xFF0F => self.i_f,
            0xFF01 => self.serial_1,
            0xFF02 => self.serial_2,
            0xFF06 => self.timer_tma,
            0xFF07 => self.timer_tac,
            0xFF24 => self.audio_nr50,
            0xFF25 => self.audio_nr51,
            0xFF26 => self.audio_nr52,
            0xFF40 => self.lcd_ctrl,
            0xFF41 => self.lcd_stat,
            0xFF42 => self.lcd_scy,
            0xFF43 => self.lcd_scx,
            0xFF44 => self.lcd_y,
            0xFF46 => self.oam_dma,
            0xFF47 => self.bgp,
            0xFF48 => self.obp0,
            0xFF49 => self.obp1,
            0xFFFF => self.ie,
            _ => todo!("HwRegisters::get for {addr:04X} not handled"),
        }
    }

    pub fn set(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF0F => self.i_f = val,
            0xFF01 => self.serial_1 = val,
            0xFF02 => self.serial_2 = val,
            0xFF06 => self.timer_tma = val,
            0xFF07 => self.timer_tac = val,
            0xFF24 => self.audio_nr50 = val,
            0xFF25 => self.audio_nr51 = val,
            0xFF26 => self.audio_nr52 = val,
            0xFF40 => self.lcd_ctrl = val,
            0xFF41 => self.lcd_stat = val,
            0xFF42 => self.lcd_scy = val,
            0xFF43 => self.lcd_scx = val,
            0xFF44 => { /* read only */ },
            0xFF46 => self.oam_dma = val,
            0xFF47 => self.bgp = val,
            0xFF48 => self.obp0 = val,
            0xFF49 => self.obp1 = val,
            0xFFFF => self.ie = val,
            _ => todo!("HwRegisters::set for {addr:04X} not handled"),
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
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize],
            0xFF00..=0xFF7F => self.hw_reg.get(addr),
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.hw_reg.get(addr),
            _ => todo!("CpuBus::read for {addr:04X} not handled"),
        }
    }

    fn write(&mut self, addr: u16, byte: u8) {
        match addr {
            0x0000..=0x7FFF => { /* ignored until MBC is emulated */ },
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = byte,
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = byte,
            0xFEA0..=0xFEFF => { /* writes to not usable section is just ignored */},
            0xFF00..=0xFF7F => self.hw_reg.set(addr, byte),
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = byte,
            0xFFFF => self.hw_reg.set(addr, byte),
            _ => todo!("CpuBus::write for {addr:04X} not handled"),
        };
    }
}

impl Bus for PpuBus<'_> {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize],
            0xFF00..=0xFF7F => self.hw_reg.get(addr),
            0xFFFF => self.hw_reg.get(addr),
            _ => todo!("PpuBus::read for {addr:04X} not handled"),
        }
    }

    fn write(&mut self, addr: u16, byte: u8) {
        match addr {
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = byte,
            0xFF00..=0xFF7F => self.hw_reg.set(addr, byte),
            0xFFFF => self.hw_reg.set(addr, byte),
            _ => todo!("PpuBus::write for {addr:04X} not handled"),
        };
    }
}
