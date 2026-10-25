// The main struct for the GameBoy, owns the components

use crate::apu::APU;
use crate::bus::{CpuBus, HwRegisters, PpuBus};
use crate::clock::Clock;
use crate::cpu::CPU;
use crate::cartridge::Cartridge;
use crate::lcd::{Consumer, LCD, make_lcd_channel};
use crate::ppu::PPU;

pub struct GameBoy {
    pub clock: Clock,
    pub cpu: CPU,
    pub ppu: PPU,
    pub lcd: LCD,
    pub apu: APU,
    pub cartridge: Option<Cartridge>,
    pub vram: [u8; 0x2000],
    pub wram: [u8; 0x2000],
    pub oam: [u8; 160],  // Object Attribute Memory
    pub hw_reg: HwRegisters,
    pub ie_reg: u8,
    pub hram: [u8; 127],

    pub lcd_consumer: Option<Consumer>,

    debugged: bool,
    watchpoints: Watchpoints,
}

pub struct Watchpoints {
    pub ranges: Vec<(u16, u16)>,
    pub hit: Option<u16>,
}

#[derive(PartialEq, Eq)]
pub enum StopReason {
    Breakpoint(u16),
    Watchpoint(u16),
    Step,
    Cycle(u8),
    Halt,
    StopRequested,
}

impl GameBoy {
    pub fn new() -> Self {
        let (lcd_producer, lcd_consumer) = make_lcd_channel();
        GameBoy {
            clock: Clock::new(),
            cpu: CPU::new(),
            ppu: PPU::new(),
            lcd: LCD::new(lcd_producer),
            apu: APU::new(),
            cartridge: None,
            vram: [0; 0x2000],
            wram: [0; 0x2000],
            oam: [0; 160],
            hw_reg: HwRegisters::new(),
            ie_reg: 0,
            hram: [0; 127],
            lcd_consumer: Some(lcd_consumer),
            debugged: false,
            watchpoints: Watchpoints {
                ranges: Vec::new(),
                hit: None,
            },
        }
    }

    pub fn insert_cartridge(&mut self, cartridge: Cartridge) {
        self.cartridge = Some(cartridge);
    }

    fn cpu_and_bus(&mut self) -> (&mut CPU, CpuBus<'_>) {
        let bus = CpuBus {
            cartridge: &mut self.cartridge,
            apu: &mut self.apu,
            vram: &mut self.vram,
            wram: &mut self.wram,
            oam: &mut self.oam,
            hw_reg: &mut self.hw_reg,
            hram: &mut self.hram,
            dbg_watchpoints: if self.debugged { Some(&mut self.watchpoints) } else { None },
        };
        (&mut self.cpu, bus)
    }

    pub fn power_on(&mut self) {
        let (cpu, mut bus) = self.cpu_and_bus();
        cpu.prepare_boot_loader(&mut bus);
    }

    pub fn run(&mut self) -> StopReason {
        loop {
            let (cpu, mut bus) = self.cpu_and_bus();
            let res = cpu.step(&mut bus);

            match res {
                StopReason::Step => return StopReason::Step,
                StopReason::Breakpoint(addr) => return StopReason::Breakpoint(addr),
                StopReason::Watchpoint(addr) => return StopReason::Watchpoint(addr),
                StopReason::Halt => return StopReason::Halt,
                StopReason::StopRequested => return StopReason::StopRequested,
                StopReason::Cycle(t_cycles) => {
                    self.ppu.step(t_cycles, &mut PpuBus {
                        hw_reg: &mut self.hw_reg,
                        lcd: &mut self.lcd,
                        vram: &mut self.vram,
                        dbg_watchpoints: if self.debugged { Some(&mut self.watchpoints) } else { None },
                    });

                    self.clock.advance(t_cycles);
                },
            }
        }
    }

    pub fn take_lcd_consumer(&mut self) -> Option<Consumer> {
        self.lcd_consumer.take()
    }

    pub fn set_debugged(&mut self, state: bool) {
        self.debugged = state;
        self.cpu.set_debugged(state);
    }

    pub fn debug_set_watchpoint(&mut self, start_addr: u16, end_addr: u16) -> Result<(), String> {
        assert!(self.debugged);
        let has_overlap = self.watchpoints.ranges.iter().any(|wp| range_overlaps(wp, &(start_addr, end_addr)));
        if has_overlap {
            return Err(String::from("the range contains already watched addresses"));
        }
        self.watchpoints.ranges.push((start_addr, end_addr));
        Ok(())
    }

    pub fn debug_delete_watchpoint(&mut self, addr: u16) -> usize {
        assert!(self.debugged);
        let size = self.watchpoints.ranges.len();
        self.watchpoints.ranges.retain(|wp| !range_overlaps(wp, &(addr, addr)));
        size - self.watchpoints.ranges.len()
    }
}

fn range_overlaps(a: &(u16, u16), b: &(u16, u16)) -> bool {
    a.0 <= b.1 && b.0 <= a.1
}
