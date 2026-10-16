// The main struct for the GameBoy, owns the components

use crate::bus::{CpuBus, IoRegisters};
use crate::cpu::CPU;
use crate::cartridge::Cartridge;

pub struct GameBoy {
    pub cpu: CPU,
    pub cartridge: Option<Cartridge>,
    pub wram: [u8; 0x2000],
    pub io_reg: IoRegisters,
    pub ie_reg: u8,
    pub hram: [u8; 127],
}

#[derive(PartialEq, Eq)]
pub enum StopReason {
    Breakpoint(u16),
    Step,
    Cycle(u8),
    Halt,
}

impl GameBoy {
    pub fn new() -> Self {
        GameBoy {
            cpu: CPU::new(),
            cartridge: None,
            wram: [0; 0x2000],
            io_reg: IoRegisters::new(),
            ie_reg: 0,
            hram: [0; 127],
        }
    }

    pub fn insert_cartridge(&mut self, cartridge: Cartridge) {
        self.cartridge = Some(cartridge);
    }

    pub fn power_on(&mut self) {
        self.cpu.prepare_boot_loader();
    }

    pub fn run(&mut self) -> StopReason {
        loop {
            let res = self.cpu.step(&mut CpuBus {
                cartridge: &mut self.cartridge,
                wram: &mut self.wram,
                io_reg: &mut self.io_reg,
                ie_reg: &mut self.ie_reg,
                hram: &mut self.hram,
            });

            match res {
                StopReason::Step => return StopReason::Step,
                StopReason::Breakpoint(addr) => return StopReason::Breakpoint(addr),
                StopReason::Cycle(_) => { /* todo */ },
                StopReason::Halt => return StopReason::Halt,
                _ => unreachable!(),
            }
        }
    }

    pub fn set_debugged(&mut self, state: bool) {
        self.cpu.set_debugged(state);
    }
}
