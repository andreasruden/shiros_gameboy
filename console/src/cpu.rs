use std::{collections::HashSet, sync::atomic::{AtomicBool, Ordering}};

use crate::{bus::Bus, gameboy::StopReason, opcodes::{Operand, op}};

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
    ime: u8,  // interrupt master enable flag (0xFF = intermediate state)
    debug: bool,
    break_counter: i32, // debug break in this many instructions or -1
    breakpoints: HashSet<u16>,
    stop_request: Option<&'static AtomicBool>,
}

mod flag {
    pub const Z: u8 = 1 << 7;  // zero
    pub const N: u8 = 1 << 6;  // subtraction
    pub const H: u8 = 1 << 5;  // half carry
    pub const C: u8 = 1 << 4;  // carry
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
            ime: 0,
            debug: false,
            break_counter: -1,
            breakpoints: HashSet::new(),
            stop_request: None,
        }
    }

    pub fn af(&self) -> u16 {
        u16::from_be_bytes([self.a, self.f])
    }

    pub fn f(&self) -> u8 {
        self.f
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

    pub fn debug_set_stop_request_handle(&mut self, flag: &'static AtomicBool) {
        self.stop_request = Some(flag);
    }

    pub fn debug_stepi(&mut self, instructions: i32) {
        assert!(self.debug, "must be debugged");

        self.break_counter = instructions;
    }

    pub fn debug_set_breakpoint(&mut self, addr: u16) {
        assert!(self.debug, "must be in debug mode");
        self.breakpoints.insert(addr);
    }

    pub fn debug_delete_breakpoint(&mut self, addr: u16) {
        self.breakpoints.remove(&addr);
    }

    pub fn prepare_boot_loader(&mut self, bus: &mut impl Bus) {
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

        // Also init the LCD registers to match the result of the DMG boot loader:
        bus.write(0xFF40, 0x91);  // LCD control
        bus.write(0xFF41, 0x85);  // LCD status
        bus.write(0xFF46, 0xFF);  // OAM DMA (Object Attribute Memory: Direct Memory Access) start
        bus.write(0xFF47, 0xFc);  // background palette
        bus.write(0xFF48, 0xFF);  // sprite palette 0
        bus.write(0xFF49, 0xFF);  // sprite palette 1
    }

    // Returns a reason for "stopping", if the reason is cycle that indluces how many
    // T-cycles the emulation advanced in this step
    pub fn step(&mut self, bus: &mut impl Bus) -> StopReason {
        // M0: Instruction Fetch
        if self.m == 0 {
            if let Some(reason) = self.should_break() {
                return reason;
            }

            // Implementation of delay in EI instruction
            if self.ime == 0xFF {
                self.ime = 1;
            }

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

    // Implements the execute FSM
    fn execute_cycle(&mut self, bus: &mut impl Bus) -> ExecState {
        match self.ir {
            // 0X
            op::NOP => self.nop(bus),
            op::LD_BC_IMM16 => self.ld_word(bus, Operand::BC, Operand::Imm16),
            op::INC_BC => self.inc_reg16(bus, Operand::BC),
            op::INC_B => self.inc_reg8(bus, Operand::B),
            op::DEC_B => self.dec_reg8(bus, Operand::B),
            op::LD_B_IMM8 => self.ld_byte(bus, Operand::B, Operand::Imm8),
            op::DEC_BC => self.dec_reg16(bus, Operand::BC),
            op::INC_C => self.inc_reg8(bus, Operand::C),
            op::DEC_C => self.dec_reg8(bus, Operand::C),
            op::LD_C_IMM8 => self.ld_byte(bus, Operand::C, Operand::Imm8),

            // 1X
            op::LD_DE_IMM16 => self.ld_word(bus, Operand::DE, Operand::Imm16),
            op::INC_DE => self.inc_reg16(bus, Operand::DE),
            op::INC_D => self.inc_reg8(bus, Operand::D),
            op::DEC_D => self.dec_reg8(bus, Operand::D),
            op::DEC_DE => self.dec_reg16(bus, Operand::DE),
            op::INC_E => self.inc_reg8(bus, Operand::E),
            op::DEC_E => self.dec_reg8(bus, Operand::E),

            // 2X
            op::JR_NZ_IMM8 => self.jr(bus, |s| !s.is_set(flag::Z)),
            op::LD_HL_IMM16 => self.ld_word(bus, Operand::HL, Operand::Imm16),
            op::LDI_HL_A => self.ld_byte(bus, Operand::DerefHLPlus, Operand::A),
            op::INC_HL => self.inc_reg16(bus, Operand::HL),
            op::INC_H => self.inc_reg8(bus, Operand::H),
            op::DEC_H => self.dec_reg8(bus, Operand::H),
            op::LDI_A_HL => self.ld_byte(bus, Operand::A, Operand::DerefHLPlus),
            op::DEC_HL => self.dec_reg16(bus, Operand::HL),
            op::LD_SP_IMM16 => self.ld_word(bus, Operand::SP, Operand::Imm16),
            op::INC_L => self.inc_reg8(bus, Operand::L),
            op::DEC_L => self.dec_reg8(bus, Operand::L),

            // 3X
            op::LDD_HL_A => self.ld_byte(bus, Operand::DerefHLMinus, Operand::A),
            op::INC_SP => self.inc_reg16(bus, Operand::SP),
            op::LD_DHL_IMM8 => self.ld_deref_hl_imm8(bus),
            op::LDD_A_HL => self.ld_byte(bus, Operand::A, Operand::DerefHLMinus),
            op::DEC_SP => self.dec_reg16(bus, Operand::SP),
            op::INC_A => self.inc_reg8(bus, Operand::A),
            op::DEC_A => self.dec_reg8(bus, Operand::A),
            op::LD_A_IMM8 => self.ld_byte(bus, Operand::A, Operand::Imm8),

            // 4X
            op::LD_B_B => self.ld_byte(bus, Operand::B, Operand::B),
            op::LD_B_C => self.ld_byte(bus, Operand::B, Operand::C),
            op::LD_B_D => self.ld_byte(bus, Operand::B, Operand::D),
            op::LD_B_E => self.ld_byte(bus, Operand::B, Operand::E),
            op::LD_B_H => self.ld_byte(bus, Operand::B, Operand::H),
            op::LD_B_L => self.ld_byte(bus, Operand::B, Operand::L),
            op::LD_B_A => self.ld_byte(bus, Operand::B, Operand::A),
            op::LD_C_B => self.ld_byte(bus, Operand::C, Operand::B),
            op::LD_C_C => self.ld_byte(bus, Operand::C, Operand::C),
            op::LD_C_D => self.ld_byte(bus, Operand::C, Operand::D),
            op::LD_C_E => self.ld_byte(bus, Operand::C, Operand::E),
            op::LD_C_H => self.ld_byte(bus, Operand::C, Operand::H),
            op::LD_C_L => self.ld_byte(bus, Operand::C, Operand::L),
            op::LD_C_A => self.ld_byte(bus, Operand::C, Operand::A),

            // 5X
            op::LD_D_B => self.ld_byte(bus, Operand::D, Operand::B),
            op::LD_D_C => self.ld_byte(bus, Operand::D, Operand::C),
            op::LD_D_D => self.ld_byte(bus, Operand::D, Operand::D),
            op::LD_D_E => self.ld_byte(bus, Operand::D, Operand::E),
            op::LD_D_H => self.ld_byte(bus, Operand::D, Operand::H),
            op::LD_D_L => self.ld_byte(bus, Operand::D, Operand::L),
            op::LD_D_A => self.ld_byte(bus, Operand::D, Operand::A),
            op::LD_E_B => self.ld_byte(bus, Operand::E, Operand::B),
            op::LD_E_C => self.ld_byte(bus, Operand::E, Operand::C),
            op::LD_E_D => self.ld_byte(bus, Operand::E, Operand::D),
            op::LD_E_E => self.ld_byte(bus, Operand::E, Operand::E),
            op::LD_E_H => self.ld_byte(bus, Operand::E, Operand::H),
            op::LD_E_L => self.ld_byte(bus, Operand::E, Operand::L),
            op::LD_E_A => self.ld_byte(bus, Operand::E, Operand::A),

            // 6X
            op::LD_H_B => self.ld_byte(bus, Operand::H, Operand::B),
            op::LD_H_C => self.ld_byte(bus, Operand::H, Operand::C),
            op::LD_H_D => self.ld_byte(bus, Operand::H, Operand::D),
            op::LD_H_E => self.ld_byte(bus, Operand::H, Operand::E),
            op::LD_H_H => self.ld_byte(bus, Operand::H, Operand::H),
            op::LD_H_L => self.ld_byte(bus, Operand::H, Operand::L),
            op::LD_H_A => self.ld_byte(bus, Operand::H, Operand::A),
            op::LD_L_B => self.ld_byte(bus, Operand::L, Operand::B),
            op::LD_L_C => self.ld_byte(bus, Operand::L, Operand::C),
            op::LD_L_D => self.ld_byte(bus, Operand::L, Operand::D),
            op::LD_L_E => self.ld_byte(bus, Operand::L, Operand::E),
            op::LD_L_H => self.ld_byte(bus, Operand::L, Operand::H),
            op::LD_L_L => self.ld_byte(bus, Operand::L, Operand::L),
            op::LD_L_A => self.ld_byte(bus, Operand::L, Operand::A),

            // 7X
            op::LD_DHL_A => self.ld_byte(bus, Operand::DerefHL, Operand::A),
            op::LD_A_B => self.ld_byte(bus, Operand::A, Operand::B),
            op::LD_A_C => self.ld_byte(bus, Operand::A, Operand::C),
            op::LD_A_D => self.ld_byte(bus, Operand::A, Operand::D),
            op::LD_A_E => self.ld_byte(bus, Operand::A, Operand::E),
            op::LD_A_H => self.ld_byte(bus, Operand::A, Operand::H),
            op::LD_A_L => self.ld_byte(bus, Operand::A, Operand::L),
            op::LD_A_A => self.ld_byte(bus, Operand::A, Operand::A),

            // AX
            op::XOR_AA => self.xor(bus, Operand::A),

            // CX
            op::JP => self.jp(bus),
            op::CALL_IMM16 => self.call_imm16(bus),

            // EX
            op::LDH_IMM8_A => self.ldh_12(bus, Operand::Imm8, Operand::A),
            op::LDH_C_A => self.ldh_8(bus, Operand::C, Operand::A),
            op::LD_ADDR_A => self.ld_deref_imm16(bus, true),

            // FX
            op::LDH_A_IMM8 => self.ldh_12(bus, Operand::A, Operand::Imm8),
            op::LDH_A_C => self.ldh_8(bus, Operand::A, Operand::C),
            op::DI => self.di(bus),
            op::LD_A_ADDR => self.ld_deref_imm16(bus, false),
            op::EI => self.ei(bus),
            op::CP_A_IMM8 => self.cp(bus, Operand::A, Operand::Imm8),

            _ => unimplemented!("opcode={:02X} at PC={:04X}", self.ir, self.pc - 1),
        }
    }

    fn should_break(&mut self) -> Option<StopReason> {
        if self.debug {
            if let Some(flag) = self.stop_request && flag.swap(false, Ordering::Relaxed) {
                return Some(StopReason::StopRequested);
            }

            if self.breakpoints.contains(&self.pc) {
                return Some(StopReason::Breakpoint(self.pc));
            }

            if self.break_counter == 0 {
                self.break_counter = -1;
                return Some(StopReason::Step);
            } else if self.break_counter > 0 {
                self.break_counter -= 1;
            }
        }

        None
    }

    fn load_pc(&mut self, bus: &mut impl Bus) -> u8 {
        let byte = bus.read(self.pc);
        self.pc += 1;
        byte
    }

    fn load_mem(&mut self, bus: &mut impl Bus, src: Operand) -> u8 {
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

    fn store_mem(&mut self, bus: &mut impl Bus, dst: Operand, byte: u8) {
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

    fn reg16(&self, reg: Operand) -> u16 {
        match reg {
            Operand::BC => self.bc(),
            Operand::DE => self.de(),
            Operand::HL => self.hl(),
            Operand::SP => self.sp(),
            _ => unreachable!("{reg} is not a 16-bit register"),
        }
    }

    fn set_reg16(&mut self, reg: Operand, val: u16) {
        match reg {
            Operand::BC => self.set_bc(val),
            Operand::DE => self.set_de(val),
            Operand::HL => self.set_hl(val),
            Operand::SP => self.set_sp(val),
            _ => unreachable!("{reg} is not a 16-bit register"),
        }
    }

    fn set_flag(&mut self, flag: u8) {
        self.f |= flag;
    }

    fn clear_flag(&mut self, flag: u8) {
        self.f &= !(flag as u8);
    }

    fn update_flag(&mut self, flag: u8, value: bool) {
        if value {
            self.set_flag(flag);
        } else {
            self.clear_flag(flag);
        }
    }

    fn is_set(&self, flag: u8) -> bool {
        self.f & (flag as u8) != 0
    }

    fn nop(&mut self, _bus: &mut impl Bus) -> ExecState {
        ExecState::Done
    }

    fn inc_reg8(&mut self, _bus: &mut impl Bus, reg: Operand) -> ExecState {
        let val = self.reg8(reg);
        let new_val = val.wrapping_add(1);
        self.set_reg8(reg, new_val);

        self.update_flag(flag::Z, new_val == 0);
        self.clear_flag(flag::N);
        self.update_flag(flag::H, (val & 0x0F) == 0x0F);

        ExecState::Done
    }

    fn dec_reg8(&mut self, _bus: &mut impl Bus, reg: Operand) -> ExecState {
        let new_val = self.reg8(reg).wrapping_sub(1);
        self.set_reg8(reg, new_val);

        self.update_flag(flag::Z, new_val == 0);
        self.update_flag(flag::H, (new_val & 0x0F) == 0x0F);
        self.set_flag(flag::N);

        ExecState::Done
    }

    fn inc_reg16(&mut self, _bus: &mut impl Bus, reg: Operand) -> ExecState {
        if self.m == 0 {
            return ExecState::Partial;
        }

        let val = self.reg16(reg);
        let new_val = val.wrapping_add(1);
        self.set_reg16(reg, new_val);

        // No flags are updated for these increases

        ExecState::Done
    }

    fn dec_reg16(&mut self, _bus: &mut impl Bus, reg: Operand) -> ExecState {
        if self.m == 0 {
            return ExecState::Partial;
        }

        let new_val = self.reg16(reg).wrapping_sub(1);
        self.set_reg16(reg, new_val);

        // No flags are updated for these decreases

        ExecState::Done
    }

    fn xor(&mut self, _bus: &mut impl Bus, operand: Operand) -> ExecState {
        match operand {
            Operand::A => { self.z = self.a; }
            _ => todo!(),
        };

        self.a = self.a ^ self.z;
        self.set_flag(flag::Z);
        self.clear_flag(flag::N);
        self.clear_flag(flag::H);
        self.clear_flag(flag::C);

        ExecState::Done
    }

    fn call_imm16(&mut self, bus: &mut impl Bus) -> ExecState {
        match self.m {
            0 => ExecState::Partial,
            1 => { self.z = self.load_pc(bus); ExecState::Partial },
            2 => { self.w = self.load_pc(bus); ExecState::Partial },
            3 => { self.sp -= 1; ExecState::Partial },
            4 => {
                bus.write(self.sp, (self.pc >> 8) as u8);
                self.sp -= 1;
                ExecState::Partial
            },
            5 => {
                bus.write(self.sp, self.pc as u8);
                self.pc = u16::from_le_bytes([self.z, self.w]);
                ExecState::Done
            },
            _ => unreachable!(),
        }
    }

    fn jp(&mut self, bus: &mut impl Bus) -> ExecState {
        match self.m {
            0 => ExecState::Partial,
            1 => { self.z = self.load_pc(bus); ExecState::Partial },
            2 => { self.w = self.load_pc(bus); ExecState::Partial },
            3 => {
                let addr = u16::from_le_bytes([self.z, self.w]);
                self.pc = addr;
                return ExecState::Done;
            },
            _ => unreachable!(),
        }
    }

    fn jr<F>(&mut self, bus: &mut impl Bus, cond: F) -> ExecState
    where
        F: Fn(&Self) -> bool,
    {
        match self.m {
            0 => ExecState::Partial,
            1 => {
                self.z = self.load_pc(bus);
                if cond(self) { ExecState::Partial } else { ExecState::Done }
            },
            2 => {
                self.pc = self.pc.wrapping_add_signed((self.z as i8) as i16);
                ExecState::Done
            }
            _ => unreachable!(),
        }
    }

    fn ld_word(&mut self, bus: &mut impl Bus, dst: Operand, src: Operand) -> ExecState {
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

                let val = u16::from_le_bytes([self.z, self.w]);
                match dst {
                    Operand::BC => self.set_bc(val),
                    Operand::DE => self.set_de(val),
                    Operand::HL => self.set_hl(val),
                    Operand::SP => self.set_sp(val),
                    _ => panic!("Unsupported destinations"),
                }
                ExecState::Done
            },
            _ => unreachable!(),
        }
    }

    // LD handles by this function can be from reg-to-reg, mem-to-reg or reg-to-mem
    fn ld_byte(&mut self, bus: &mut impl Bus, dst: Operand, src: Operand) -> ExecState {
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

        ExecState::Done
    }

    // This function only implements:
    // $EA: LD [a16], A
    // $FA: LD A, [a16]
    fn ld_deref_imm16(&mut self, bus: &mut impl Bus, is_ea: bool) -> ExecState {
        match self.m {
            0 => ExecState::Partial,
            1 => {
                self.z = self.load_mem(bus, Operand::Imm8);
                ExecState::Partial
            },
            2 => {
                self.w = self.load_mem(bus, Operand::Imm8);
                ExecState::Partial
            },
            3 => {
                let addr = u16::from_le_bytes([self.z, self.w]);
                if is_ea {
                    bus.write(addr, self.a);
                } else {
                    self.a = bus.read(addr);
                }
                ExecState::Done
            },
            _ => unreachable!(),
        }
    }

    // This function only implements LD [HL], imm8
    fn ld_deref_hl_imm8(&mut self, bus: &mut impl Bus) -> ExecState {
        match self.m {
            0 => ExecState::Partial,
            1 => { self.z = self.load_mem(bus, Operand::Imm8); ExecState::Partial },
            2 => { self.store_mem(bus, Operand::DerefHL, self.z); ExecState::Done },
            _ => unreachable!(),
        }
    }

    // Implements LDH [C], A and LDH A, [C]
    fn ldh_8(&mut self, bus: &mut impl Bus, dst: Operand, src: Operand) -> ExecState {
        match self.m {
            0 => ExecState::Partial,
            1 => {
                if src == Operand::C {
                    assert!(dst == Operand::A);
                    self.a = bus.read(0xFF00 + self.c as u16);
                } else {
                    assert!(dst == Operand::C && src == Operand::A);
                    bus.write(0xFF00 + self.c as u16, self.a);
                }
                ExecState::Done
            },
            _ => unreachable!(),
        }
    }

    // Implements LDH [imm8], A and LDH a, [imm8]
    fn ldh_12(&mut self, bus: &mut impl Bus, dst: Operand, src: Operand) -> ExecState {
        match self.m {
            0 => ExecState::Partial,
            1 => {
                if src.is_mem() {
                    self.z = self.load_mem(bus, src)
                } else {
                    self.z = self.load_mem(bus, dst);
                }
                ExecState::Partial
            },
            2 => {
                if src.is_mem() {
                    self.set_reg8(dst, bus.read(0xFF00 + self.z as u16));
                } else {
                    bus.write(0xFF00 + self.z as u16, self.reg8(src));
                }
                ExecState::Done
            }
            _ => unreachable!(),
        }
    }

    fn di(&mut self, _bus: &mut impl Bus) -> ExecState {
        self.ime = 0;
        ExecState::Done
    }

    fn ei(&mut self, _bus: &mut impl Bus) -> ExecState {
        self.ime = 0xFF;
        ExecState::Done
    }

    fn cp(&mut self, bus: &mut impl Bus, lhs: Operand, rhs: Operand) -> ExecState {
        // Only implements CP a, imm8 currently
        match self.m {
            0 => {
                if !lhs.is_mem() {
                    self.z = self.reg8(lhs);
                }
                return ExecState::Partial;
            }
            1 => {
                if rhs.is_mem() {
                    self.w = self.load_mem(bus, rhs);
                }
            },
            _ => unreachable!(),
        }

        self.update_flag(flag::Z, self.z == self.w);
        self.set_flag(flag::N);
        self.update_flag(flag::H, (self.z & 0xF) < (self.w & 0xF));
        self.update_flag(flag::C, self.z < self.w);

        ExecState::Done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    enum Cycle {
        R(u16, u8),
        W(u16, u8),
        Idle,
    }
    use Cycle::*;

    struct MockBus {
        cycles: Vec<Cycle>,
        pos: usize,
    }

    impl MockBus {
        fn next(&mut self, actual: Cycle) -> &Cycle {
            let expected = self.cycles.get(self.pos)
                .unwrap_or_else(|| panic!("unexpected bus access {actual:04X?} after all cycles"));
            self.pos += 1;
            expected
        }
    }

    impl Bus for MockBus {
        fn read(&mut self, addr: u16) -> u8 {
            match self.next(R(addr, 0)) {
                &R(a, v) if a == addr => v,
                other => panic!("expected {other:04X?}, got read at {addr:04X}"),
            }
        }

        fn write(&mut self, addr: u16, byte: u8) {
            match self.next(W(addr, byte)) {
                &W(a, v) if a == addr && v == byte => {},
                other => panic!("expected {other:04X?}, got {:04X?}", W(addr, byte)),
            }
        }
    }

    fn set_reg(cpu: &mut CPU, reg: &str, val: u16) {
        match reg {
            "a" => cpu.a = val as u8,
            "f" => cpu.f = val as u8,
            "b" => cpu.b = val as u8,
            "c" => cpu.c = val as u8,
            "d" => cpu.d = val as u8,
            "e" => cpu.e = val as u8,
            "h" => cpu.h = val as u8,
            "l" => cpu.l = val as u8,
            "af" => cpu.set_af(val),
            "bc" => cpu.set_bc(val),
            "de" => cpu.set_de(val),
            "hl" => cpu.set_hl(val),
            "sp" => cpu.sp = val,
            "pc" => cpu.pc = val,
            "ime" => cpu.ime = val as u8,
            _ => panic!("unknown register {reg}"),
        }
    }

    fn regs(cpu: &CPU) -> [(&'static str, u16); 11] {
        [
            ("a", cpu.a as u16), ("f", cpu.f as u16),
            ("b", cpu.b as u16), ("c", cpu.c as u16),
            ("d", cpu.d as u16), ("e", cpu.e as u16),
            ("h", cpu.h as u16), ("l", cpu.l as u16),
            ("sp", cpu.sp), ("pc", cpu.pc),
            ("ime", cpu.ime as u16),
        ]
    }

    fn run(given: &[(&str, u16)], cycles: Vec<Cycle>, expect: &[(&str, u16)]) {
        let mut cpu = CPU::new();
        cpu.pc = 0x0100;
        for &(reg, val) in given {
            set_reg(&mut cpu, reg, val);
        }

        let mut expected = CPU::new();
        for (reg, val) in regs(&cpu) {
            set_reg(&mut expected, reg, val);
        }
        for &(reg, val) in expect {
            set_reg(&mut expected, reg, val);
        }

        let mut bus = MockBus { cycles, pos: 0 };
        for cycle in 1.. {
            let pos = bus.pos;
            cpu.step(&mut bus);
            if bus.pos == pos && bus.cycles.get(pos) == Some(&Idle) {
                bus.pos += 1;
            }
            assert_eq!(bus.pos, cycle, "cycle {cycle} did not match expectation");
            if cpu.m == 0 && bus.pos == bus.cycles.len() {
                break;
            }
        }

        assert_eq!(bus.pos, bus.cycles.len(), "unconsumed cycles: {:04X?}", &bus.cycles[bus.pos..]);
        let (actual, expected) = (regs(&cpu), regs(&expected));
        assert!(actual == expected, "registers differ\n  actual: {actual:04X?}\nexpected: {expected:04X?}");
    }

    macro_rules! cpu_tests {
        ($(
            $name:ident:
                $(given { $($greg:ident: $gval:expr),* $(,)? })?
                cycles [$($cycle:expr),* $(,)?]
                expect { $($ereg:ident: $eval:expr),* $(,)? }
        )*) => {
            $(
                #[test]
                fn $name() {
                    run(
                        &[$($((stringify!($greg), $gval)),*)?],
                        vec![$($cycle),*],
                        &[$((stringify!($ereg), $eval)),*],
                    );
                }
            )*
        };
    }

    cpu_tests! {
        nop:
            cycles [R(0x0100, op::NOP)]
            expect { pc: 0x101 }

        dec_b:
            given { b: 0, f: flag::Z.into() }
            cycles [R(0x0100, op::DEC_B)]
            expect { pc: 0x101, b: 0xFF, f: (flag::N | flag::H).into() }

        jr_nz_imm8_skip:
            given { f: flag::Z.into() }
            cycles [R(0x0100, op::JR_NZ_IMM8), R(0x0101, 0xD6)]
            expect { pc: 0x0102 }

        jr_nz_imm8_jump:
            cycles [R(0x0100, op::JR_NZ_IMM8), R(0x0101, 0xD6), Idle]
            expect { pc: 0x00D8 }

        ld_hl_imm16:
            cycles [R(0x0100, op::LD_HL_IMM16), R(0x0101, 0x42), R(0x0102, 0x42)]
            expect { pc: 0x0103, hl: 0x4242 }

        ldi_hl_a:
            given  { hl: 0xC000, a: 42 }
            cycles [R(0x0100, op::LDI_HL_A), W(0xC000, 42)]
            expect { pc: 0x0101, a: 42, hl: 0xC001 }

        ld_dhl_imm8:
            given { hl: 0xC000 }
            cycles [R(0x100, op::LD_DHL_IMM8), R(0x101, 42), W(0xC000, 42)]
            expect { pc: 0x102 }

        ldi_a_hl:
            given  { hl: 0xC000 }
            cycles [R(0x0100, op::LDI_A_HL), R(0xC000, 42)]
            expect { pc: 0x0101, a: 42, hl: 0xC001 }

        ldd_hl_a:
            given  { hl: 0xC000, a: 42 }
            cycles [R(0x0100, op::LDD_HL_A), W(0xC000, 42)]
            expect { pc: 0x0101, a: 42, hl: 0xBFFF }

        inc_sp:
            given { sp: 0x1122 }
            cycles [R(0x100, op::INC_SP), Idle]
            expect { pc: 0x101, sp: 0x1123 }

        ldd_a_hl:
            given  { hl: 0xC000 }
            cycles [R(0x0100, op::LDD_A_HL), R(0xC000, 42)]
            expect { pc: 0x0101, a: 42, hl: 0xBFFF }

        dec_sp:
            given { sp: 0x1122 }
            cycles [R(0x100, op::DEC_SP), Idle]
            expect { pc: 0x101, sp: 0x1121 }

        inc_a:
            given { a: 0x3F, f: (flag::Z | flag::N).into() }
            cycles [R(0x100, op::INC_A)]
            expect { pc: 0x101, a: 0x40, f: flag::H.into() }

        ld_a_imm8:
            cycles [R(0x100, op::LD_A_IMM8), R(0x101, 42)]
            expect { pc: 0x102, a: 42 }

        ld_b_c:
            given { c: 42 }
            cycles [R(0x100, op::LD_B_C)]
            expect { pc: 0x101, b: 42 }

        ld_dhl_a:
            given { hl: 0xC040, a: 42 }
            cycles [R(0x100, op::LD_DHL_A), W(0xC040, 42)]
            expect { pc: 0x101 }

        xor_aa:
            given { a: 42 }
            cycles [R(0x100, op::XOR_AA)]
            expect { pc:0x101, a: 0, f: flag::Z.into() }

        jp:
            cycles [R(0x100, op::JP), R(0x101, 0xCC), R(0x102, 0xDD), Idle]
            expect { pc: 0xDDCC }

        call_imm16:
            given { sp: 0xFFF4 }
            cycles [R(0x100, op::CALL_IMM16), R(0x101, 0x3C), R(0x102, 0x02), Idle, W(0xFFF3, 0x01), W(0xFFF2, 0x03)]
            expect { pc: 0x023C, sp: 0xFFF2 }

        ldh_imm8_a:
            given { a: 42 }
            cycles[R(0x100, op::LDH_IMM8_A), R(0x101, 0xF1), W(0xFFF1, 42)]
            expect { pc: 0x102 }

        ldh_deref_c_a:
            given { a: 42, c: 0xAA }
            cycles [R(0x100, op::LDH_C_A), W(0xFFAA, 42)]
            expect { pc: 0x101 }

        ld_addr_a:
            given { a: 42 }
            cycles[R(0x100, op::LD_ADDR_A), R(0x101, 0xAF), R(0x102, 0xC0), W(0xC0AF, 42)]
            expect{ pc: 0x103 }

        ldh_a_imm8:
            cycles[R(0x100, op::LDH_A_IMM8), R(0x101, 0xCC), R(0xFFCC, 42)]
            expect { pc: 0x102, a: 42 }

        di:
            given { ime: true as u16 }
            cycles [R(0x100, op::DI)]
            expect { pc: 0x101, ime: 0 }

        ld_a_addr:
            cycles[R(0x100, op::LD_A_ADDR), R(0x101, 0xAF), R(0x102, 0xC0), R(0xC0AF, 42)]
            expect{ pc: 0x103, a: 42 }

        ei_1:
            given { ime: false as u16 }
            cycles[R(0x100, op::EI)]
            expect { pc: 0x101, ime: 0xFF }  // intermediate value, still means false

        ei_2:
            given { ime: false as u16 }
            cycles[R(0x100, op::EI), R(0x101, op::NOP)]  // dummy instruction to observe change
            expect { pc: 0x102, ime: 0x1 }

        cp_a_imm8:
            given { a: 42, f: flag::Z.into() }
            cycles[R(0x100, op::CP_A_IMM8), R(0x101, 43)]
            expect { pc: 0x102, f: (flag::N | flag::H | flag::C).into() }
    }

    macro_rules! ld_r_imm8 {
        ($($name:ident: $opcode:ident, $reg:ident;)*) => {
            cpu_tests! {$(
                $name:
                    cycles [R(0x0100, op::$opcode), R(0x0101, 0x42)]
                    expect { pc: 0x0102, $reg: 0x42 }
            )*}
        };
    }

    ld_r_imm8! {
        ld_b_imm8: LD_B_IMM8, b;
        ld_c_imm8: LD_C_IMM8, c;
    }
}
