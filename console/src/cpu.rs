use crate::{bus::Bus, gameboy::StopReason, opcodes::{Operand, op}};

pub struct CPU {
    a: u8, f: u8,  // Accumulator & Flags
    b: u8, c: u8,
    d: u8, e: u8,
    h: u8, l: u8,
    sp: u16,  // Stack Pointer
    pc: u16,  // Program Counter
    debug: bool,
    break_counter: i32, // debug break in this many instructions or -1
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
            debug: false,
            break_counter: -1,
        }
    }

    pub fn af(&self) -> u16 {
        u16::from_be_bytes([self.a, self.f])
    }

    pub fn set_af(&mut self, af: u16) {
        let [a, f] = af.to_be_bytes();
        self.a = a;
        self.f = f;
    }

    pub fn bc(&self) -> u16 {
        u16::from_be_bytes([self.b, self.c])
    }

    pub fn set_bc(&mut self, bc: u16) {
        let [b, c] = bc.to_be_bytes();
        self.b = b;
        self.c = c;
    }

    pub fn de(&self) -> u16 {
        u16::from_be_bytes([self.d, self.e])
    }

    pub fn set_de(&mut self, de: u16) {
        let [d, e] = de.to_be_bytes();
        self.d = d;
        self.e = e;
    }

    pub fn hl(&self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }

    pub fn set_hl(&mut self, hl: u16) {
        let [h, l] = hl.to_be_bytes();
        self.h = h;
        self.l = l;
    }

    pub fn sp(&self) -> u16 {
        self.sp
    }

    pub fn set_sp(&mut self, sp: u16) {
        self.sp = sp
    }

    pub fn pc(&self) -> u16 {
        self.pc
    }

    pub fn set_pc(&mut self, pc: u16) {
        self.pc = pc
    }

    pub fn set_debugged(&mut self, state: bool) {
        self.debug = state;
    }

    pub fn debug_stepi(&mut self, instructions: i32) {
        assert!(self.debug, "must be debugged");

        self.break_counter = instructions;
    }

    pub fn prepare_boot_loader(&mut self) {
        println!("TODO: Bootloader skipped for now");

        if self.debug {
            self.break_counter = 0;
        }

        // Just set the CPU state into a valid config:
        self.a = 0x01; self.f = 0xB0;
        self.b = 0x00; self.c = 0x13;
        self.d = 0x00; self.e = 0xD8;
        self.h = 0x01; self.l = 0x4D;
        self.sp = 0xFFFE;
        self.pc = 0x0100;
    }

    pub fn run_instruction(&mut self, bus: &mut Bus) -> StopReason {
        if self.debug {
            if self.break_counter == 0 {
                self.break_counter = -1;
                return StopReason::Step;
            } else if self.break_counter > 0 {
                self.break_counter -= 1;
            }
        }

        let opcode = self.fetch_pc(bus);
        match opcode {
            op::NOP => self.nop(bus),
            op::LD_HL_IMM16 => self.ld_word(bus, Operand::HL, Operand::Imm16),
            op::INC_A => self.inc_a(bus),
            op::XOR_AA => self.xor(bus, Operand::A),
            op::JP => self.jp(bus),
            _ => unimplemented!("opcode={:02X} at PC={:04X}", opcode, self.pc),
        }

        StopReason::NA
    }

    fn fetch_operand(&self, operand: Operand) -> u8 {
        match operand {
            Operand::A => self.a,
            Operand::B => self.b,
            Operand::C => self.c,
            Operand::D => self.d,
            Operand::E => self.e,
            Operand::H => self.h,
            Operand::L => self.l,
            _ => todo!(),
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

    fn xor(&mut self, _bus: &mut Bus, operand: Operand) {
        self.a = self.a ^ self.fetch_operand(operand);
        self.set_flag(Flag::Z);
        self.clear_flag(Flag::N);
        self.clear_flag(Flag::H);
        self.clear_flag(Flag::Z);
        println!("XOR A, {} (A = {:02X})", operand, self.a);
    }

    fn jp(&mut self, bus: &mut Bus) {
        let lo = self.fetch_pc(bus);
        let hi = self.fetch_pc(bus);
        let addr = u16::from_le_bytes([lo, hi]);
        self.pc = addr;
        println!("JP {:04X}", addr);
    }

    fn ld_word(&mut self, bus: &mut Bus, dst: Operand, src: Operand) {
        let val: u16;
        if src == Operand::Imm16 {
            val = u16::from_le_bytes([self.fetch_pc(bus), self.fetch_pc(bus)]);
        } else {
            unimplemented!("src operand {src} not implemented yet");
        }

        match dst {
            Operand::BC => self.set_bc(val),
            Operand::DE => self.set_de(val),
            Operand::HL => self.set_hl(val),
            Operand::SP => self.set_sp(val),
            _ => panic!("Unsupported destinations"),
        }

        println!("LD {}, {:04X}", dst, val);
    }
}
