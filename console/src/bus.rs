use crate::{Cartridge, apu::APU, gameboy::Watchpoints, lcd::LCD};

pub struct HwRegisters {
    pub joypad: u8,         // $FF00
    pub i_f: u8,            // $FF04 interrupt flag
    pub serial_1: u8,       // $FF01
    pub serial_2: u8,       // $FF02

    // Timer
    pub timer_tma: u8,      // $FF06 timer modulo
    pub timer_tac: u8,      // $FF07 timer control

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

    pub wy: u8,             // $FF4A window y position
    pub wx: u8,             // $FF4B window x position + 7

    pub ie: u8,             // $FFFF interrupt enable
}

pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, byte: u8);
    fn hw_reg(&mut self) -> Option<&mut HwRegisters> {
        None
    }

    fn dbg_watchpoints(&mut self) -> Option<&mut Watchpoints> {
        None
    }
}

pub struct CpuBus<'a> {
    pub cartridge: &'a mut Option<Cartridge>,
    pub apu: &'a mut APU,
    pub vram: &'a mut [u8; 0x2000],
    pub wram: &'a mut [u8; 0x2000],
    pub oam: &'a mut [u8; 160],
    pub hw_reg: &'a mut HwRegisters,
    pub hram: &'a mut [u8; 127],

    pub dbg_watchpoints: Option<&'a mut Watchpoints>,
}

pub struct PpuBus<'a> {
    pub hw_reg: &'a mut HwRegisters,
    pub lcd: &'a mut LCD,
    pub vram: &'a mut [u8; 0x2000],

    pub dbg_watchpoints: Option<&'a mut Watchpoints>,
}

impl HwRegisters {
    pub fn new() -> Self {
        HwRegisters {
            joypad: 0,
            i_f: 0,
            serial_1: 0,
            serial_2: 0,
            timer_tma: 0,
            timer_tac: 0,
            lcd_ctrl: 0,
            lcd_stat: 0,
            lcd_scy: 0,
            lcd_scx: 0,
            lcd_y: 0,
            oam_dma: 0,
            bgp: 0,
            obp0: 0,
            obp1: 0,
            wy: 0,
            wx: 0,
            ie: 0,
        }
    }

    pub fn get(&mut self, addr: u16) -> u8 {
        match addr {
            0xFF00 => self.joypad,
            0xFF0F => self.i_f,
            0xFF01 => self.serial_1,
            0xFF02 => self.serial_2,
            0xFF06 => self.timer_tma,
            0xFF07 => self.timer_tac,
            0xFF40 => self.lcd_ctrl,
            0xFF41 => self.lcd_stat,
            0xFF42 => self.lcd_scy,
            0xFF43 => self.lcd_scx,
            0xFF44 => self.lcd_y,
            0xFF46 => self.oam_dma,
            0xFF47 => self.bgp,
            0xFF48 => self.obp0,
            0xFF49 => self.obp1,
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            0xFFFF => self.ie,
            _ => todo!("HwRegisters::get for {addr:04X} not handled"),
        }
    }

    pub fn set(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF00 => self.joypad = (self.joypad & !0x30) | (val | 0x30),  // only bit 4 and 5 writable by game
            0xFF0F => self.i_f = val,
            0xFF01 => self.serial_1 = val,
            0xFF02 => self.serial_2 = val,
            0xFF06 => self.timer_tma = val,
            0xFF07 => self.timer_tac = val,
            0xFF40 => self.lcd_ctrl = val,
            0xFF41 => self.lcd_stat = val,
            0xFF42 => self.lcd_scy = val,
            0xFF43 => self.lcd_scx = val,
            0xFF44 => { /* read only */ },
            0xFF46 => self.oam_dma = val,
            0xFF47 => self.bgp = val,
            0xFF48 => self.obp0 = val,
            0xFF49 => self.obp1 = val,
            0xFF4A => self.wy = val,
            0xFF4B => self.wx = val,
            0xFF4C..=0xFF7F => { /* unused in DMG */ },
            0xFFFF => self.ie = val,
            _ => todo!("HwRegisters::set for {addr:04X} not handled"),
        }
    }
}

impl Bus for CpuBus<'_> {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => match self.cartridge.as_ref() {
                None => 0,
                Some(c) => c.rom[addr as usize],
            },
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize],
            0xFF10..=0xFF3F => self.apu.read(addr),
            0xFF00..=0xFF7F => self.hw_reg.get(addr),
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.hw_reg.get(addr),
            _ => todo!("CpuBus::read for {addr:04X} not handled"),
        }
    }

    fn write(&mut self, addr: u16, byte: u8) {
        if let Some(wp) = &mut self.dbg_watchpoints {
            handle_watchpoints(wp, addr, byte);
        }

        match addr {
            0x0000..=0x7FFF => { /* ignored until MBC is emulated */ },
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = byte,
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = byte,
            0xFE00..=0xFE9F => {
                // CPU can only write to this area during HBlank and VBlank; mode 0 and 1
                let ppu_mode = self.hw_reg.lcd_stat & !0b11;
                if ppu_mode == 0 || ppu_mode == 1 {
                    self.oam[(addr - 0xFE00) as usize] = byte;
                }
            },
            0xFEA0..=0xFEFF => { /* writes to not usable section is just ignored */},
            0xFF10..=0xFF3F => self.apu.write(addr, byte),
            0xFF00..=0xFF7F => self.hw_reg.set(addr, byte),
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = byte,
            0xFFFF => self.hw_reg.set(addr, byte),
            _ => todo!("CpuBus::write for {addr:04X} not handled"),
        };
    }

    fn hw_reg(&mut self) -> Option<&mut HwRegisters> {
        Some(&mut self.hw_reg)
    }

    fn dbg_watchpoints(&mut self) -> Option<&mut Watchpoints> {
        self.dbg_watchpoints.as_deref_mut()
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
        if let Some(wp) = &mut self.dbg_watchpoints {
            handle_watchpoints(wp, addr, byte);
        }

        match addr {
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = byte,
            0xFF00..=0xFF7F => self.hw_reg.set(addr, byte),
            0xFFFF => self.hw_reg.set(addr, byte),
            _ => todo!("PpuBus::write for {addr:04X} not handled"),
        };
    }

    fn hw_reg(&mut self) -> Option<&mut HwRegisters> {
        Some(&mut self.hw_reg)
    }

    fn dbg_watchpoints(&mut self) -> Option<&mut Watchpoints> {
        self.dbg_watchpoints.as_deref_mut()
    }
}

fn handle_watchpoints(watchpoints: &mut Watchpoints, addr: u16, _byte: u8) {
    match watchpoints.ranges.iter().find(|wp| (wp.0..=wp.1).contains(&addr)) {
        Some(_) => watchpoints.hit = Some(addr),
        None => { /* do nothing */},
    }
}
