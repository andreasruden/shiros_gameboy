// The main struct for the GameBoy, owns the components

use crate::bus::CpuBus;
use crate::cpu::CPU;
use crate::cartridge::Cartridge;

pub struct GameBoy {
    pub cpu: CPU,
    pub cartridge: Option<Cartridge>,
    pub wram: [u8; 0x2000],
}

#[derive(PartialEq, Eq)]
pub enum StopReason {
    Breakpoint(u32),
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
        }
    }

    pub fn insert_cartridge(&mut self, cartridge: Cartridge) {
        self.cartridge = Some(cartridge);
    }

    pub fn power_on(&mut self) {
        self.cpu.prepare_boot_loader();
    }

    pub fn run(&mut self) -> StopReason {
        for _i in 0..100 {
            let res = self.cpu.step(&mut CpuBus {
                cartridge: &mut self.cartridge,
                wram: &mut self.wram
            });

            match res {
                StopReason::Step => return StopReason::Step,
                StopReason::Cycle(_) => { /* todo */ },
                _ => unreachable!(),
            }
        }

        StopReason::Halt
    }

    pub fn set_debugged(&mut self, state: bool) {
        self.cpu.set_debugged(state);
    }
}
