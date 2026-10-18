// The main struct for the GameBoy, owns the components

use crate::bus::{CpuBus, IoRegisters};
use crate::cpu::CPU;
use crate::cartridge::Cartridge;
use crate::lcd::{Consumer, LCD, make_lcd_channel};
use crate::ppu::PPU;

pub struct GameBoy {
    pub cpu: CPU,
    pub ppu: PPU,
    pub lcd: LCD,
    pub cartridge: Option<Cartridge>,
    pub wram: [u8; 0x2000],
    pub io_reg: IoRegisters,
    pub ie_reg: u8,
    pub hram: [u8; 127],

    pub lcd_consumer: Option<Consumer>
}

#[derive(PartialEq, Eq)]
pub enum StopReason {
    Breakpoint(u16),
    Step,
    Cycle(u8),
    Halt,
    StopRequested,
}

impl GameBoy {
    pub fn new() -> Self {
        let (lcd_producer, lcd_consumer) = make_lcd_channel();
        GameBoy {
            cpu: CPU::new(),
            ppu: PPU::new(),
            lcd: LCD::new(lcd_producer),
            cartridge: None,
            wram: [0; 0x2000],
            io_reg: IoRegisters::new(),
            ie_reg: 0,
            hram: [0; 127],
            lcd_consumer: Some(lcd_consumer),
        }
    }

    pub fn insert_cartridge(&mut self, cartridge: Cartridge) {
        self.cartridge = Some(cartridge);
    }

    fn cpu_and_bus(&mut self) -> (&mut CPU, CpuBus<'_>) {
        let bus = CpuBus {
            cartridge: &mut self.cartridge,
            wram: &mut self.wram,
            io_reg: &mut self.io_reg,
            ie_reg: &mut self.ie_reg,
            hram: &mut self.hram,
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
                StopReason::Cycle(_) => { /* todo */ },
                StopReason::Halt => return StopReason::Halt,
                StopReason::StopRequested => return StopReason::StopRequested,
            }
        }
    }

    pub fn take_lcd_consumer(&mut self) -> Option<Consumer> {
        self.lcd_consumer.take()
    }

    pub fn set_debugged(&mut self, state: bool) {
        self.cpu.set_debugged(state);
    }
}
