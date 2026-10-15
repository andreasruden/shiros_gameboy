use crate::{bus::CpuBus, gameboy::StopReason, opcodes::{Operand, op}};

pub struct CPU {
    a: u8, f: u8,  // Accumulator & Flags
    b: u8, c: u8,
    d: u8, e: u8,
    h: u8, l: u8,
    sp: u16, // Stack Pointer
    pc: u16, // Program Counter
    m: u8,   // counts m cycle into current instruction (0..=4)
    ir: u8,  // instruction register
    z: u8, w: u8,  // internal registers
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

#[derive(PartialEq, Eq)]
enum ExecState {
    Done,
    Partial,
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
            m: 0,
            ir: 0,
            z: 0, w: 0,
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

    // Returns a reason for "stopping", if the reason is cycle that indluces how many
    // T-cycles the emulation advanced in this step
    pub fn step(&mut self, bus: &mut CpuBus) -> StopReason {
        if let Some(reason) = self.should_break() {
            return reason;
        }

        // M0: Instruction Fetch
        if self.m == 0 {
            self.ir = self.load_pc(bus);
        }

        // M1-4: nth Data Cycle
        if self.execute_cycle(bus) == ExecState::Done {
            self.m = 0;
        } else {
            self.m += 1;
        }

        StopReason::Cycle(4)
    }

    // implements the execute FSM
    fn execute_cycle(&mut self, bus: &mut CpuBus) -> ExecState {
        match self.ir {
            op::NOP => self.nop(bus),
            op::LD_B_IMM8 => self.ld_byte(bus, Operand::B, Operand::Imm8),
            op::LD_C_IMM8 => self.ld_byte(bus, Operand::C, Operand::Imm8),
            op::LD_HL_IMM16 => self.ld_word(bus, Operand::HL, Operand::Imm16),
            op::LDI_HL_A => self.ld_byte(bus, Operand::DerefHLPlus, Operand::A),
            op::LDI_A_HL => self.ld_byte(bus, Operand::A, Operand::DerefHLPlus),
            op::LDD_HL_A => self.ld_byte(bus, Operand::DerefHLMinus, Operand::A),
            op::LDD_A_HL => self.ld_byte(bus, Operand::A, Operand::DerefHLMinus),
            op::INC_A => self.inc_a(bus),
            op::XOR_AA => self.xor(bus, Operand::A),
            op::JP => self.jp(bus),
            _ => unimplemented!("opcode={:02X} at PC={:04X}", self.ir, self.pc),
        }
    }

    fn should_break(&mut self) -> Option<StopReason> {
        if self.debug && self.m == 0 {
            if self.break_counter == 0 {
                self.break_counter = -1;
                return Some(StopReason::Step);
            } else if self.break_counter > 0 {
                self.break_counter -= 1;
            }
        }

        None
    }

    fn load_pc(&mut self, bus: &mut CpuBus) -> u8 {
        let byte = bus.read(self.pc);
        self.pc += 1;
        byte
    }

    fn load_mem(&mut self, bus: &mut CpuBus, src: Operand) -> u8 {
        match src {
            Operand::Imm8 => self.load_pc(bus),
            Operand::DerefHL => bus.read(self.hl()),
            Operand::DerefHLPlus => {
                let addr = self.hl();
                self.set_hl(self.hl().wrapping_add(1));
                bus.read(addr)
            },
            Operand::DerefHLMinus => {
                let addr = self.hl();
                self.set_hl(self.hl().wrapping_sub(1));
                bus.read(addr)
            },
            _ => unreachable!(),
        }
    }

    fn store_mem(&mut self, bus: &mut CpuBus, dst: Operand, byte: u8) {
        match dst {
            Operand::DerefHL => bus.write(self.hl(), byte),
            Operand::DerefHLPlus => {
                bus.write(self.hl(), byte);
                self.set_hl(self.hl().wrapping_add(1));
            },
            Operand::DerefHLMinus => {
                bus.write(self.hl(), byte);
                self.set_hl(self.hl().wrapping_sub(1));
            },
            _ => unreachable!(),
        }
    }

    fn reg8(&self, reg: Operand) -> u8 {
        match reg {
            Operand::A => self.a,
            Operand::B => self.b,
            Operand::C => self.c,
            Operand::D => self.d,
            Operand::E => self.e,
            Operand::H => self.h,
            Operand::L => self.l,
            _ => unreachable!("{reg} is not an 8-bit register"),
        }
    }

    fn set_reg8(&mut self, reg: Operand, val: u8) {
        match reg {
            Operand::A => self.a = val,
            Operand::B => self.b = val,
            Operand::C => self.c = val,
            Operand::D => self.d = val,
            Operand::E => self.e = val,

            Operand::H => self.h = val,
            Operand::L => self.l = val,

            _ => unreachable!("{reg} is not an 8-bit register"),
        }
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

    fn nop(&mut self, _bus: &mut CpuBus) -> ExecState {
        println!("NOP");
        ExecState::Done
    }

    fn inc_a(&mut self, _bus: &mut CpuBus) -> ExecState {
        let value = self.a;
        let result = value.wrapping_add(1);
        self.update_flag(Flag::Z, result == 0);
        self.clear_flag(Flag::N);
        self.update_flag(Flag::H, (value & 0x0F) == 0x0F);
        self.a = value;
        println!("INC A (A = {:02X})", self.a);
        ExecState::Done
    }

    fn xor(&mut self, _bus: &mut CpuBus, operand: Operand) -> ExecState {
        match operand {
            Operand::A => { self.z = self.a; }
            _ => todo!(),
        };

        self.a = self.a ^ self.z;
        self.set_flag(Flag::Z);
        self.clear_flag(Flag::N);
        self.clear_flag(Flag::H);
        self.clear_flag(Flag::Z);

        println!("XOR A, {} (A = {:02X})", operand, self.a);
        ExecState::Done
    }

    fn jp(&mut self, bus: &mut CpuBus) -> ExecState {
        match self.m {
            0 => ExecState::Partial,
            1 => { self.z = self.load_pc(bus); ExecState::Partial },
            2 => { self.w = self.load_pc(bus); ExecState::Partial },
            3 => {
                let addr = u16::from_le_bytes([self.z, self.w]);
                self.pc = addr;
                println!("JP {:04X}", addr);
                return ExecState::Done;
            },
            _ => unreachable!(),
        }
    }

    fn ld_word(&mut self, bus: &mut CpuBus, dst: Operand, src: Operand) -> ExecState {
        match self.m {
            0 => ExecState::Partial,
            1 => {
                if src == Operand::Imm16 {
                    self.z = self.load_pc(bus);
                } else {
                    unimplemented!("src operand {src} not implemented yet");
                }
                ExecState::Partial
            },
            2 => {
                if src == Operand::Imm16 {
                    self.w = self.load_pc(bus);
                } else {
                    unimplemented!("src operand {src} not implemented yet");
                }
                ExecState::Partial
            },
            3 => {
                let val = u16::from_le_bytes([self.z, self.w]);
                match dst {
                    Operand::BC => self.set_bc(val),
                    Operand::DE => self.set_de(val),
                    Operand::HL => self.set_hl(val),
                    Operand::SP => self.set_sp(val),
                    _ => panic!("Unsupported destinations"),
                }
                println!("LD {}, {:04X}", dst, val);
                ExecState::Done
            }
            _ => unreachable!(),
        }
    }

    fn ld_byte(&mut self, bus: &mut CpuBus, dst: Operand, src: Operand) -> ExecState {
        // LD can be from reg-to-reg, mem-to-reg or reg-to-mem
        match self.m {
            0 => {
                if !src.is_mem() {
                    self.z = self.reg8(src);
                }

                if src.is_mem() || dst.is_mem() {
                    return ExecState::Partial;
                } else {
                    self.set_reg8(dst, self.z);
                }
            },
            1 => {
                if src.is_mem() {
                    self.z = self.load_mem(bus, src);
                    self.set_reg8(dst, self.z);
                } else {
                    self.store_mem(bus, dst, self.z);
                }
            },
            _ => unreachable!(),
        }

        println!("LD {}, {:02X}", dst, self.z);
        ExecState::Done
    }
}
