mod opcodes;

use crate::bus::Bus;
use opcodes::op;

pub struct CPU {
    a: u8, f: u8,  // Accumulator & Flags
    b: u8, c: u8,
    d: u8, e: u8,
    h: u8, l: u8,
    sp: u16,  // Stack Pointer
    pc: u16,  // Program Counter
}

#[repr(u8)]
enum Flag {
    Z = 1 << 7,  // zero
    N = 1 << 6,  // subtraction
    H = 1 << 5,  // half carry
    C = 1 << 4,  // carry
}

impl CPU {
    pub fn new() -> Self {
        Self {
            a: 0, f: 0,
            b: 0, c: 0,
            d: 0, e: 0,
            h: 0, l: 0,
            sp: 0,
            pc: 0,
        }
    }

    pub fn run_boot_loader(&mut self) {
        println!("TODO: Bootloader skipped for now");
        // Just set the CPU state into a valid config:
        self.a = 0x01; self.f = 0xB0;
        self.b = 0x00; self.c = 0x13;
        self.d = 0x00; self.e = 0xD8;
        self.h = 0x01; self.l = 0x4D;
        self.sp = 0xFFFE;
        self.pc = 0x0100;
    }

    pub fn run_instruction(&mut self, bus: &mut Bus) {
        let opcode = self.fetch_pc(bus);

        match opcode {
            op::NOP => self.nop(bus),
            op::INC_A => self.inc_a(bus),
            op::XOR_AA => self.xor_aa(bus),
            op::JP => self.jp(bus),
            _ => unimplemented!("opcode={:02X} at PC={:04X}", opcode, self.pc),
        }
    }

    fn fetch_pc(&mut self, bus: &mut Bus) -> u8 {
        let byte = bus.read(self.pc);
        self.pc += 1;
        byte
    }

    fn set_flag(&mut self, flag: Flag) {
        self.f |= flag as u8;
    }

    fn clear_flag(&mut self, flag: Flag) {
        self.f &= !(flag as u8);
    }

    fn update_flag(&mut self, flag: Flag, value: bool) {
        if value {
            self.set_flag(flag);
        } else {
            self.clear_flag(flag);
        }
    }

    fn is_set(&self, flag: Flag) -> bool {
        self.f & (flag as u8) != 0
    }

    fn nop(&mut self, _bus: &mut Bus) {
        println!("NOP");
    }

    fn inc_a(&mut self, _bus: &mut Bus) {
        let value = self.a;
        let result = value.wrapping_add(1);
        self.update_flag(Flag::Z, result == 0);
        self.clear_flag(Flag::N);
        self.update_flag(Flag::H, (value & 0x0F) == 0x0F);
        println!("INC A (A = {:02X})", self.a);
    }

    fn xor_aa(&mut self, _bus: &mut Bus) {
        self.a = 0;
        self.set_flag(Flag::Z);
        self.clear_flag(Flag::N);
        self.clear_flag(Flag::H);
        self.clear_flag(Flag::Z);
        println!("XOR A, A (A = {:02X})", self.a);
    }

    fn jp(&mut self, bus: &mut Bus) {
        let lo = self.fetch_pc(bus);
        let hi = self.fetch_pc(bus);
        let addr = u16::from_le_bytes([lo, hi]);
        self.pc = addr;
        println!("JP {:04X}", addr);
    }
}
