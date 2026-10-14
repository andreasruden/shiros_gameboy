// The main struct for the GameBoy, owns the components

use crate::bus::Bus;
use crate::cpu::CPU;
use crate::cartridge::Cartridge;

pub struct GameBoy {
    pub cpu: CPU,
    pub bus: Bus,
}

#[derive(PartialEq, Eq)]
pub enum StopReason {
    Breakpoint(u32),
    Step,
    Halt,
    NA,
}

impl GameBoy {
    pub fn new() -> Self {
        GameBoy {
            cpu: CPU::new(),
            bus: Bus::new(),
        }
    }

    pub fn insert_cartridge(&mut self, cartridge: Cartridge) {
        self.bus.cartridge = Some(cartridge);
    }

    pub fn power_on(&mut self) {
        self.cpu.prepare_boot_loader();
    }

    pub fn run(&mut self) -> StopReason {
        for _i in 0..100 {
            let res = self.cpu.run_instruction(&mut self.bus);
            if res != StopReason::NA {
                return res;
            }
        }

        StopReason::Halt
    }

    pub fn set_debugged(&mut self, state: bool) {
        self.cpu.set_debugged(state);
    }
}
