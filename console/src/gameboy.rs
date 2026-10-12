// The main struct for the GameBoy, owns the components

use crate::bus::Bus;
use crate::cpu::CPU;
use crate::cartridge::Cartridge;

pub struct GameBoy {
    cpu: CPU,
    bus: Bus,
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
        self.cpu.run_boot_loader();
    }

    pub fn run_frame(&mut self) {
        for _i in 0..100 {
            self.cpu.run_instruction(&mut self.bus);
        }
    }
}
