use std::{collections::HashSet, sync::atomic::{AtomicBool, Ordering}};

use crate::{bus::Bus, gameboy::StopReason, opcodes::{CB_TARGETS, Operand, op}};

pub struct CPU {
    a: u8, f: u8,              // Accumulator & Flags
    b: u8, c: u8,
    d: u8, e: u8,
    h: u8, l: u8,
    sp: u16,                   // Stack Pointer
    pc: u16,                   // Program Counter
    m: u8,                     // counts m cycle into current instruction (0..=4)
    dispatch: u8,              // counts m cycles during interrupt dispatch
    ir: u8,                    // instruction register
    z: u8, w: u8,              // internal registers
    ime: u8,                   // interrupt master enable flag (0xFF = intermediate state)
    halted: bool,              // true when HALT is used and interrupt has not fired yet
    debug: bool,
    break_counter: i32,        // debug break in this many instructions or -1
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

const INTERRUPT_FLAG: u16 = 0xFF0F;
const INTERRUPT_ENABLE: u16 = 0xFFFF;

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
            dispatch: 0,
            ir: 0,
            z: 0, w: 0,
            ime: 0,
            halted: false,
            debug: false,
            break_counter: -1,
            breakpoints: HashSet::new(),
            stop_request: None,
        }
    }

    pub fn is_halted(&self) -> bool {
        self.halted
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
        self.f = f & 0xF0;  // lowest byte of F not writable to
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

    pub fn ime(&self) -> u8 {
        self.ime
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

        // set all joypad buttons to not held (TODO: should be removable when joypad is implemented)
        bus.hw_reg().unwrap().joypad = 0xCF;
    }

    // Returns a reason for "stopping", if the reason is cycle that indluces how many
    // T-cycles the emulation advanced in this step
    pub fn step(&mut self, bus: &mut impl Bus) -> StopReason {
        // M0: Instruction Fetch
        if self.m == 0 {
            if let Some(reason) = self.should_break(bus) {
                return reason;
            }

            if self.handle_interrupts(bus) {
                return StopReason::Cycle(4);
            }

            if self.halted {
                return StopReason::Cycle(4);
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

    pub fn handle_interrupts(&mut self, bus: &mut impl Bus) -> bool {
        let interrupts = bus.hw_reg().unwrap().get(INTERRUPT_FLAG) & bus.hw_reg().unwrap().get(INTERRUPT_ENABLE);
        if interrupts == 0 && self.dispatch == 0 {
            return false;
        }
        self.halted = false;

        if self.ime != 1 {
            return false;
        }

        let m = self.dispatch;
        self.dispatch += 1;
        match m {
            0 => true,
            1 => true,
            2 => {
                self.sp = self.sp.strict_sub(1);
                bus.write(self.sp, (self.pc >> 8) as u8);
                true
            },
            3 => {
                self.sp = self.sp.strict_sub(1);
                bus.write(self.sp, self.pc as u8);
                true
            },
            4 => {
                let (new_pc, bit) = match interrupts {
                    i if i & (1 << 0) != 0 => (0x40, 0),  // VBLANK
                    i if i & (1 << 1) != 0 => (0x48, 1),  // LCD
                    i if i & (1 << 2) != 0 => (0x50, 2),  // Timer
                    i if i & (1 << 3) != 0 => (0x58, 3),  // Serial
                    i if i & (1 << 4) != 0 => (0x60, 4),  // Joypad
                    _ => unreachable!(),
                };
                self.z = bit;
                self.pc = new_pc;
                true
            },
            5 => {
                let i_f = bus.hw_reg().unwrap().get(INTERRUPT_FLAG);
                bus.hw_reg().unwrap().set(INTERRUPT_FLAG, i_f & !(1 << self.z));
                self.ime = 0;
                self.dispatch = 0;
                false
            },
            _ => unreachable!(),
        }
    }

    // Implements the execute FSM
    fn execute_cycle(&mut self, bus: &mut impl Bus) -> ExecState {
        match self.ir {
            // 0X
            op::NOP => self.nop(bus),
            op::LD_BC_IMM16 => self.ld_word(bus, Operand::BC, Operand::Imm16),
            op::LD_DBC_A => self.ld_byte(bus, Operand::DerefBC, Operand::A),
            op::INC_BC => self.inc_reg16(bus, Operand::BC),
            op::INC_B => self.inc_reg8(bus, Operand::B),
            op::DEC_B => self.dec_reg8(bus, Operand::B),
            op::LD_B_IMM8 => self.ld_byte(bus, Operand::B, Operand::Imm8),
            op::RLCA => self.rotate_reg_a(bus, true, true),
            op::LD_ADDR_SP => self.ld_addr_sp(bus),
            op::ADD_HL_BC => self.add_hl(bus, Operand::BC),
            op::LD_A_DBC => self.ld_byte(bus, Operand::A, Operand::DerefBC),
            op::DEC_BC => self.dec_reg16(bus, Operand::BC),
            op::INC_C => self.inc_reg8(bus, Operand::C),
            op::DEC_C => self.dec_reg8(bus, Operand::C),
            op::LD_C_IMM8 => self.ld_byte(bus, Operand::C, Operand::Imm8),
            op::RRCA => self.rotate_reg_a(bus, false, true),

            // 1X
            op::LD_DE_IMM16 => self.ld_word(bus, Operand::DE, Operand::Imm16),
            op::LD_DDE_A => self.ld_byte(bus, Operand::DerefDE, Operand::A),
            op::INC_DE => self.inc_reg16(bus, Operand::DE),
            op::INC_D => self.inc_reg8(bus, Operand::D),
            op::DEC_D => self.dec_reg8(bus, Operand::D),
            op::LD_D_IMM8 => self.ld_byte(bus, Operand::D, Operand::Imm8),
            op::RLA => self.rotate_reg_a(bus, true, false),
            op::JR_IMM8 => self.jr(bus, |_| true),
            op::ADD_HL_DE => self.add_hl(bus, Operand::DE),
            op::LD_A_DDE => self.ld_byte(bus, Operand::A, Operand::DerefDE),
            op::DEC_DE => self.dec_reg16(bus, Operand::DE),
            op::INC_E => self.inc_reg8(bus, Operand::E),
            op::DEC_E => self.dec_reg8(bus, Operand::E),
            op::LD_E_IMM8 => self.ld_byte(bus, Operand::E, Operand::Imm8),
            op::RRA => self.rotate_reg_a(bus, false, false),

            // 2X
            op::JR_NZ_IMM8 => self.jr(bus, |s| !s.is_set(flag::Z)),
            op::LD_HL_IMM16 => self.ld_word(bus, Operand::HL, Operand::Imm16),
            op::LDI_HL_A => self.ld_byte(bus, Operand::DerefHLPlus, Operand::A),
            op::INC_HL => self.inc_reg16(bus, Operand::HL),
            op::INC_H => self.inc_reg8(bus, Operand::H),
            op::DEC_H => self.dec_reg8(bus, Operand::H),
            op::LD_H_IMM8 => self.ld_byte(bus, Operand::H, Operand::Imm8),
            op::JR_Z_IMM8 => self.jr(bus, |s| s.is_set(flag::Z)),
            op::ADD_HL_HL => self.add_hl(bus, Operand::HL),
            op::LDI_A_HL => self.ld_byte(bus, Operand::A, Operand::DerefHLPlus),
            op::DEC_HL => self.dec_reg16(bus, Operand::HL),
            op::INC_L => self.inc_reg8(bus, Operand::L),
            op::DEC_L => self.dec_reg8(bus, Operand::L),
            op::LD_L_IMM8 => self.ld_byte(bus, Operand::L, Operand::Imm8),
            op::CPL => self.cpl(bus),

            // 3X
            op::JR_NC_IMM8 => self.jr(bus, |s| !s.is_set(flag::C)),
            op::LD_SP_IMM16 => self.ld_word(bus, Operand::SP, Operand::Imm16),
            op::LDD_HL_A => self.ld_byte(bus, Operand::DerefHLMinus, Operand::A),
            op::INC_SP => self.inc_reg16(bus, Operand::SP),
            op::INC_DHL => self.unit_step_deref_hl(bus, true),
            op::DEC_DHL => self.unit_step_deref_hl(bus, false),
            op::LD_DHL_IMM8 => self.ld_deref_hl_imm8(bus),
            op::JR_C_IMM8 => self.jr(bus, |s| s.is_set(flag::C)),
            op::ADD_HL_SP => self.add_hl(bus, Operand::SP),
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
            op::LD_B_DHL => self.ld_byte(bus, Operand::B, Operand::DerefHL),
            op::LD_B_A => self.ld_byte(bus, Operand::B, Operand::A),
            op::LD_C_B => self.ld_byte(bus, Operand::C, Operand::B),
            op::LD_C_C => self.ld_byte(bus, Operand::C, Operand::C),
            op::LD_C_D => self.ld_byte(bus, Operand::C, Operand::D),
            op::LD_C_E => self.ld_byte(bus, Operand::C, Operand::E),
            op::LD_C_H => self.ld_byte(bus, Operand::C, Operand::H),
            op::LD_C_L => self.ld_byte(bus, Operand::C, Operand::L),
            op::LD_C_DHL => self.ld_byte(bus, Operand::C, Operand::DerefHL),
            op::LD_C_A => self.ld_byte(bus, Operand::C, Operand::A),

            // 5X
            op::LD_D_B => self.ld_byte(bus, Operand::D, Operand::B),
            op::LD_D_C => self.ld_byte(bus, Operand::D, Operand::C),
            op::LD_D_D => self.ld_byte(bus, Operand::D, Operand::D),
            op::LD_D_E => self.ld_byte(bus, Operand::D, Operand::E),
            op::LD_D_H => self.ld_byte(bus, Operand::D, Operand::H),
            op::LD_D_L => self.ld_byte(bus, Operand::D, Operand::L),
            op::LD_D_DHL => self.ld_byte(bus, Operand::D, Operand::DerefHL),
            op::LD_D_A => self.ld_byte(bus, Operand::D, Operand::A),
            op::LD_E_B => self.ld_byte(bus, Operand::E, Operand::B),
            op::LD_E_C => self.ld_byte(bus, Operand::E, Operand::C),
            op::LD_E_D => self.ld_byte(bus, Operand::E, Operand::D),
            op::LD_E_E => self.ld_byte(bus, Operand::E, Operand::E),
            op::LD_E_H => self.ld_byte(bus, Operand::E, Operand::H),
            op::LD_E_L => self.ld_byte(bus, Operand::E, Operand::L),
            op::LD_E_DHL => self.ld_byte(bus, Operand::E, Operand::DerefHL),
            op::LD_E_A => self.ld_byte(bus, Operand::E, Operand::A),

            // 6X
            op::LD_H_B => self.ld_byte(bus, Operand::H, Operand::B),
            op::LD_H_C => self.ld_byte(bus, Operand::H, Operand::C),
            op::LD_H_D => self.ld_byte(bus, Operand::H, Operand::D),
            op::LD_H_E => self.ld_byte(bus, Operand::H, Operand::E),
            op::LD_H_H => self.ld_byte(bus, Operand::H, Operand::H),
            op::LD_H_L => self.ld_byte(bus, Operand::H, Operand::L),
            op::LD_H_DHL => self.ld_byte(bus, Operand::H, Operand::DerefHL),
            op::LD_H_A => self.ld_byte(bus, Operand::H, Operand::A),
            op::LD_L_B => self.ld_byte(bus, Operand::L, Operand::B),
            op::LD_L_C => self.ld_byte(bus, Operand::L, Operand::C),
            op::LD_L_D => self.ld_byte(bus, Operand::L, Operand::D),
            op::LD_L_E => self.ld_byte(bus, Operand::L, Operand::E),
            op::LD_L_H => self.ld_byte(bus, Operand::L, Operand::H),
            op::LD_L_L => self.ld_byte(bus, Operand::L, Operand::L),
            op::LD_L_DHL => self.ld_byte(bus, Operand::L, Operand::DerefHL),
            op::LD_L_A => self.ld_byte(bus, Operand::L, Operand::A),

            // 7X
            op::LD_DHL_B => self.ld_byte(bus, Operand::DerefHL, Operand::B),
            op::LD_DHL_C => self.ld_byte(bus, Operand::DerefHL, Operand::C),
            op::LD_DHL_D => self.ld_byte(bus, Operand::DerefHL, Operand::D),
            op::LD_DHL_E => self.ld_byte(bus, Operand::DerefHL, Operand::E),
            op::LD_DHL_H => self.ld_byte(bus, Operand::DerefHL, Operand::H),
            op::LD_DHL_L => self.ld_byte(bus, Operand::DerefHL, Operand::L),
            op::HALT => self.halt(bus),
            op::LD_DHL_A => self.ld_byte(bus, Operand::DerefHL, Operand::A),
            op::LD_A_B => self.ld_byte(bus, Operand::A, Operand::B),
            op::LD_A_C => self.ld_byte(bus, Operand::A, Operand::C),
            op::LD_A_D => self.ld_byte(bus, Operand::A, Operand::D),
            op::LD_A_E => self.ld_byte(bus, Operand::A, Operand::E),
            op::LD_A_H => self.ld_byte(bus, Operand::A, Operand::H),
            op::LD_A_L => self.ld_byte(bus, Operand::A, Operand::L),
            op::LD_A_DHL => self.ld_byte(bus, Operand::A, Operand::DerefHL),
            op::LD_A_A => self.ld_byte(bus, Operand::A, Operand::A),

            // 8X
            op::ADD_A_B => self.add(bus, Operand::B, false),
            op::ADD_A_C => self.add(bus, Operand::C, false),
            op::ADD_A_D => self.add(bus, Operand::D, false),
            op::ADD_A_E => self.add(bus, Operand::E, false),
            op::ADD_A_H => self.add(bus, Operand::H, false),
            op::ADD_A_L => self.add(bus, Operand::L, false),
            op::ADD_A_DHL => self.add(bus, Operand::DerefHL, false),
            op::ADD_A_A => self.add(bus, Operand::A, false),

            op::ADC_A_B => self.add(bus, Operand::B, true),
            op::ADC_A_C => self.add(bus, Operand::C, true),
            op::ADC_A_D => self.add(bus, Operand::D, true),
            op::ADC_A_E => self.add(bus, Operand::E, true),
            op::ADC_A_H => self.add(bus, Operand::H, true),
            op::ADC_A_L => self.add(bus, Operand::L, true),
            op::ADC_A_DHL => self.add(bus, Operand::DerefHL, true),
            op::ADC_A_A => self.add(bus, Operand::A, true),

            // AX
            op::AND_A_B => self.and(bus, Operand::B),
            op::AND_A_C => self.and(bus, Operand::C),
            op::AND_A_D => self.and(bus, Operand::D),
            op::AND_A_E => self.and(bus, Operand::E),
            op::AND_A_H => self.and(bus, Operand::H),
            op::AND_A_L => self.and(bus, Operand::L),
            op::AND_A_DHL => self.and(bus, Operand::DerefHL),
            op::AND_A_A => self.and(bus, Operand::A),
            op::XOR_A_B => self.xor(bus, Operand::B),
            op::XOR_A_C => self.xor(bus, Operand::C),
            op::XOR_A_D => self.xor(bus, Operand::D),
            op::XOR_A_E => self.xor(bus, Operand::E),
            op::XOR_A_H => self.xor(bus, Operand::H),
            op::XOR_A_L => self.xor(bus, Operand::L),
            op::XOR_A_DHL => self.xor(bus, Operand::DerefHL),
            op::XOR_A_A => self.xor(bus, Operand::A),

            // BX
            op::OR_A_B => self.or(bus, Operand::B),
            op::OR_A_C => self.or(bus, Operand::C),
            op::OR_A_D => self.or(bus, Operand::D),
            op::OR_A_E => self.or(bus, Operand::E),
            op::OR_A_H => self.or(bus, Operand::H),
            op::OR_A_L => self.or(bus, Operand::L),
            op::OR_A_DHL => self.or(bus, Operand::DerefHL),
            op::OR_A_A => self.or(bus, Operand::A),

            // CX
            op::RET_NZ => self.ret_test(bus, flag::Z, false),
            op::POP_BC => self.pop(bus, Operand::BC),
            op::JP_NZ_IMM16 => self.jp(bus, |s| !s.is_set(flag::Z)),
            op::JP => self.jp(bus, |_| true),
            op::CALL_NZ_IMM16 => self.call_imm16(bus, |s| !s.is_set(flag::Z)),
            op::PUSH_BC => self.push(bus, Operand::BC),
            op::ADD_A_IMM8 => self.add(bus, Operand::Imm8, false),
            op::RST_00 => self.rst(bus, 0x00),
            op::RET_Z => self.ret_test(bus, flag::Z, true),
            op::RET => self.ret(bus, false),
            op::JP_Z_IMM16 => self.jp(bus, |s| s.is_set(flag::Z)),
            op::PREFIX => self.prefix(bus),
            op::CALL_Z_IMM16 => self.call_imm16(bus, |s| s.is_set(flag::Z)),
            op::CALL_IMM16 => self.call_imm16(bus, |_| true),
            op::ADC_A_IMM8 => self.add(bus, Operand::Imm8, true),
            op::RST_08 => self.rst(bus, 0x08),

            // DX
            op::RET_NC => self.ret_test(bus, flag::C, false),
            op::POP_DE => self.pop(bus, Operand::DE),
            op::JP_NC_IMM16 => self.jp(bus, |s| !s.is_set(flag::C)),
            op::CALL_NC_IMM16 => self.call_imm16(bus, |s| !s.is_set(flag::C)),
            op::PUSH_DE => self.push(bus, Operand::DE),
            op::RST_10 => self.rst(bus, 0x10),
            op::RET_C => self.ret_test(bus, flag::C, true),
            op::RETI => self.ret(bus, true),
            op::JP_C_IMM16 => self.jp(bus, |s| s.is_set(flag::C)),
            op::CALL_C_IMM16 => self.call_imm16(bus, |s| s.is_set(flag::C)),
            op::RST_18 => self.rst(bus, 0x18),

            // EX
            op::POP_HL => self.pop(bus, Operand::HL),
            op::PUSH_HL => self.push(bus, Operand::HL),
            op::LDH_IMM8_A => self.ldh_12(bus, Operand::Imm8, Operand::A),
            op::LDH_C_A => self.ldh_8(bus, Operand::C, Operand::A),
            op::AND_A_IMM8 => self.and(bus, Operand::Imm8),
            op::RST_20 => self.rst(bus, 0x20),
            op::JP_HL => self.jp_hl(bus),
            op::LD_ADDR_A => self.ld_deref_imm16(bus, true),
            op::XOR_A_IMM8 => self.xor(bus, Operand::Imm8),
            op::RST_28 => self.rst(bus, 0x28),

            // FX
            op::POP_AF => self.pop(bus, Operand::AF),
            op::PUSH_AF => self.push(bus, Operand::AF),
            op::LDH_A_IMM8 => self.ldh_12(bus, Operand::A, Operand::Imm8),
            op::LDH_A_C => self.ldh_8(bus, Operand::A, Operand::C),
            op::DI => self.di(bus),
            op::OR_A_IMM8 => self.or(bus, Operand::Imm8),
            op::RST_30 => self.rst(bus, 0x30),
            op::LD_SP_HL => self.ld_sp_hl(bus),
            op::LD_A_ADDR => self.ld_deref_imm16(bus, false),
            op::EI => self.ei(bus),
            op::RST_38 => self.rst(bus, 0x38),
            op::CP_A_IMM8 => self.cp(bus, Operand::A, Operand::Imm8),

            _ => unimplemented!("opcode={:02X} at PC={:04X}", self.ir, self.pc - 1),
        }
    }

    fn should_break(&mut self, bus: &mut impl Bus) -> Option<StopReason> {
        if self.debug {
            if let Some(flag) = self.stop_request && flag.swap(false, Ordering::Relaxed) {
                return Some(StopReason::StopRequested);
            }

            if self.breakpoints.contains(&self.pc) {
                return Some(StopReason::Breakpoint(self.pc));
            }

            if let Some(wps) = &mut bus.dbg_watchpoints() && let Some(addr) = wps.hit.take() {
                return Some(StopReason::Watchpoint(addr));
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
            Operand::DerefBC => bus.read(self.bc()),
            Operand::DerefDE => bus.read(self.de()),
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
            Operand::DerefBC => bus.write(self.bc(), byte),
            Operand::DerefDE => bus.write(self.de(), byte),
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
            Operand::AF => self.af(),
            Operand::BC => self.bc(),
            Operand::DE => self.de(),
            Operand::HL => self.hl(),
            Operand::SP => self.sp(),
            _ => unreachable!("{reg} is not a 16-bit register"),
        }
    }

    fn set_reg16(&mut self, reg: Operand, val: u16) {
        match reg {
            Operand::AF => self.set_af(val),
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

    // Implements INC [HL] and DEC [HL]
    fn unit_step_deref_hl(&mut self, bus: &mut impl Bus, add: bool) -> ExecState {
        match self.m {
            0 => ExecState::Partial,
            1 => { self.z = bus.read(self.hl()); ExecState::Partial },
            2 => {
                let val = self.z;
                let new_val = if add { val.wrapping_add(1) } else { val.wrapping_sub(1) };
                let half_carry = if add { (val & 0x0F) == 0x0F } else { (new_val & 0x0F) == 0x0F };
                bus.write(self.hl(), new_val);
                self.update_flag(flag::Z, new_val == 0);
                self.clear_flag(flag::N);
                self.update_flag(flag::H, half_carry);
                ExecState::Done
            },
            _ => unreachable!(),
        }
    }

    // First operand is always A, and result is always stored into A
    fn add(&mut self, bus: &mut impl Bus, operand: Operand, with_carry: bool) -> ExecState {
        if operand.is_mem() && self.m == 0 {
            return ExecState::Partial;
        }

        let carry = if with_carry { self.is_set(flag::C) as u8 } else { 0 };
        let rhs = if operand.is_mem() { self.load_mem(bus, operand) } else { self.reg8(operand) };
        let res = self.a.wrapping_add(carry).wrapping_add(rhs);

        self.update_flag(flag::Z, res == 0);
        self.clear_flag(flag::N);
        self.update_flag(flag::H, (self.a & 0xF) + (rhs & 0xF) + carry > 0xF);
        self.update_flag(flag::C, (self.a as u16) + (rhs as u16) + carry as u16 > 0xFF);

        self.a = res;

        ExecState::Done
    }

    // First operand is always HL, and result is always stored into HL
    fn add_hl(&mut self, _bus: &mut impl Bus, operand: Operand) -> ExecState {
        if self.m == 0 {
            return ExecState::Partial;
        }

        let hl = self.hl();
        let rhs = self.reg16(operand);
        let res = hl.wrapping_add(rhs);

        // Add HL, r8 leaves Z unchanged
        self.clear_flag(flag::N);
        self.update_flag(flag::H, (hl & 0xFFF) + (rhs & 0xFFF) > 0xFFF);
        self.update_flag(flag::C, (hl as u32) + (rhs as u32) > 0xFFFF);

        self.set_hl(res);

        ExecState::Done
    }

    // First operand is always A, and result is always stored into A
    fn and(&mut self, bus: &mut impl Bus, operand: Operand) -> ExecState {
        match self.m {
            0 => {
                if operand.is_mem() {
                    return ExecState::Partial;
                }
                self.z = self.reg8(operand);
            },
            1 => {
                self.z = self.load_mem(bus, operand);
            }
            _ => unreachable!(),
        }

        self.a = self.a & self.z;
        self.clear_flag(flag::C);
        self.set_flag(flag::H);
        self.clear_flag(flag::N);
        self.update_flag(flag::Z, self.a == 0);

        ExecState::Done
    }

    // First operand is always A, and result is always stored into A
    fn xor(&mut self, bus: &mut impl Bus, operand: Operand) -> ExecState {
        match self.m {
            0 => {
                if operand.is_mem() {
                    return ExecState::Partial;
                } else {
                    self.z = self.reg8(operand);
                }
            },
            1 => self.z = self.load_mem(bus, operand),
            _ => unreachable!(),
        };

        self.a = self.a ^ self.z;
        self.update_flag(flag::Z, self.a == 0);
        self.clear_flag(flag::N);
        self.clear_flag(flag::H);
        self.clear_flag(flag::C);

        ExecState::Done
    }

    // First operand is always A, and result is always stored into A
    fn or(&mut self, bus: &mut impl Bus, operand: Operand) -> ExecState {
        match self.m {
            0 => {
                if operand.is_mem() {
                    return ExecState::Partial;
                } else {
                    self.z = self.reg8(operand);
                }
            }
            1 => self.z = self.load_mem(bus, operand),
            _ => unreachable!(),
        }

        let result = self.a | self.z;
        self.clear_flag(flag::C);
        self.clear_flag(flag::H);
        self.clear_flag(flag::N);
        self.update_flag(flag::Z, result == 0);
        self.a = result;

        ExecState::Done
    }

    fn cpl(&mut self, _bus: &mut impl Bus) -> ExecState {
        self.a = !self.a;
        self.set_flag(flag::H);
        self.set_flag(flag::N);
        ExecState::Done
    }

    fn call_imm16<F>(&mut self, bus: &mut impl Bus, cond: F) -> ExecState
    where
        F: Fn(&Self) -> bool
    {
        match self.m {
            0 => ExecState::Partial,
            1 => { self.z = self.load_pc(bus); ExecState::Partial },
            2 => {
                self.w = self.load_pc(bus);
                if !cond(self) {
                    return ExecState::Done;
                }
                ExecState::Partial
            },
            3 => { self.sp = self.sp.wrapping_sub(1); ExecState::Partial },
            4 => {
                bus.write(self.sp, (self.pc >> 8) as u8);
                self.sp = self.sp.wrapping_sub(1);
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

    fn rst(&mut self, bus: &mut impl Bus, addr: u16) -> ExecState {
        match self.m {
            0 => ExecState::Partial,
            1 => { self.sp = self.sp.wrapping_sub(1); ExecState::Partial },
            2 => {
                bus.write(self.sp, (self.pc >> 8) as u8);
                self.sp = self.sp.wrapping_sub(1);
                ExecState::Partial
            },
            3 => {
                bus.write(self.sp, self.pc as u8);
                self.pc = addr;
                ExecState::Done
            },
            _ => unreachable!(),
        }
    }

    fn ret(&mut self, bus: &mut impl Bus, is_reti: bool) -> ExecState {
        match self.m {
            0 => ExecState::Partial,
            1 => {
                self.z = bus.read(self.sp());
                self.sp = self.sp.wrapping_add(1);
                ExecState::Partial
            },
            2 => {
                self.w = bus.read(self.sp());
                self.sp = self.sp.wrapping_add(1);
                ExecState::Partial
            },
            3 => {
                self.set_pc(u16::from_le_bytes([self.z, self.w]));
                if is_reti {
                    self.ime = 1;
                }
                ExecState::Done
            }
            _ => unreachable!(),
        }
    }

    fn ret_test(&mut self, bus: &mut impl Bus, flag: u8, set: bool) -> ExecState {
        match self.m {
            0 => ExecState::Partial,
            1 => {
                let take_branch = set == self.is_set(flag);
                if !take_branch {
                    return ExecState::Done;
                }
                ExecState::Partial
            },
            2 => {
                self.z = bus.read(self.sp());
                self.sp = self.sp.wrapping_add(1);
                ExecState::Partial
            },
            3 => {
                self.w = bus.read(self.sp());
                self.sp = self.sp.wrapping_add(1);
                ExecState::Partial
            },
            4 => {
                self.set_pc(u16::from_le_bytes([self.z, self.w]));
                ExecState::Done
            }
            _ => unreachable!(),
        }
    }

    fn jp<F>(&mut self, bus: &mut impl Bus, cond: F) -> ExecState
    where
        F: Fn(&Self) -> bool,
    {
        match self.m {
            0 => ExecState::Partial,
            1 => { self.z = self.load_pc(bus); ExecState::Partial },
            2 => {
                self.w = self.load_pc(bus);
                if cond(self) { ExecState::Partial } else { ExecState::Done }
            },
            3 => {
                let addr = u16::from_le_bytes([self.z, self.w]);
                self.pc = addr;
                ExecState::Done
            },
            _ => unreachable!(),
        }
    }

    fn jp_hl(&mut self, _bus: &mut impl Bus) -> ExecState {
        self.set_pc(self.hl());
        ExecState::Done
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

    // Implements LD [Imm16], SP
    fn ld_addr_sp(&mut self, bus: &mut impl Bus) -> ExecState {
        match self.m {
            0 => ExecState::Partial,
            1 => { self.z = self.load_pc(bus); ExecState::Partial },
            2 => { self.w = self.load_pc(bus); ExecState::Partial },
            3 => {
                bus.write(u16::from_le_bytes([self.z, self.w]), self.sp() as u8);
                ExecState::Partial
            },
            4 => {
                bus.write(u16::from_le_bytes([self.z, self.w]) + 1, (self.sp() >> 8) as u8);
                ExecState::Done
            },
            _ => unreachable!(),
        }
    }

    // Implements LD SP, HL
    fn ld_sp_hl(&mut self, _bus: &mut impl Bus) -> ExecState {
        if self.m == 0 {
            return ExecState::Partial;
        }
        self.set_sp(self.hl());
        ExecState::Done
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

    fn push(&mut self, bus: &mut impl Bus, reg: Operand) -> ExecState {
        match self.m {
            0 => { ExecState::Partial },
            1 => {
                self.sp = self.sp.wrapping_sub(1);
                ExecState::Partial
            },
            2 => {
                let hi = (self.reg16(reg) >> 8) as u8;
                bus.write(self.sp, hi);
                self.sp = self.sp.wrapping_sub(1);
                ExecState::Partial
            },
            3 => {
                let lo = self.reg16(reg) as u8;
                bus.write(self.sp, lo);
                ExecState::Done
            }
            _ => unreachable!(),
        }
    }

    fn pop(&mut self, bus: &mut impl Bus, reg: Operand) -> ExecState {
        match self.m {
            0 => ExecState::Partial,
            1 => {
                self.z = bus.read(self.sp);
                self.sp = self.sp.wrapping_add(1);
                ExecState::Partial
            },
            2 => {
                self.w = bus.read(self.sp);
                self.sp = self.sp.wrapping_add(1);
                self.set_reg16(reg, u16::from_le_bytes([self.z, self.w]));
                ExecState::Done
            },
            _ => unreachable!(),
        }
    }

    fn rotate_reg_a(&mut self, _bus: &mut impl Bus, left: bool, circular: bool) -> ExecState {
        let val = self.a;
        let bit = if left { 7 } else { 0 };
        let carry_out = (val >> bit) & 1;
        let shift_in = if circular { carry_out } else { self.is_set(flag::C) as u8 };

        let res = if left {
            val << 1 | shift_in
        } else {
            val >> 1 | (shift_in << 7)
        };

        self.a = res;
        self.clear_flag(flag::Z);
        self.clear_flag(flag::N);
        self.clear_flag(flag::H);
        self.update_flag(flag::C, carry_out != 0);

        ExecState::Done
    }

    fn halt(&mut self, _bus: &mut impl Bus) -> ExecState {
        self.halted = true;
        ExecState::Done
    }

    fn prefix(&mut self, bus: &mut impl Bus) -> ExecState {
        // Load the inner opcode into z after the prefix
        if self.m == 0 {
            return ExecState::Partial;
        } else if self.m == 1 {
            self.z = self.load_pc(bus);
        }

        // Decode the opcode into (target, arg, group)
        let opcode = self.z;
        let target = CB_TARGETS[(opcode & 0b111) as usize];
        let arg = (opcode >> 3) & 0b111;
        let group = opcode >> 6;

        // Load target
        if target.is_mem() {
            if self.m == 1 {
                return ExecState::Partial;
            } else if self.m == 2 {
                self.w = self.load_mem(bus, target);
            }
        } else if self.m == 1 {
            self.w = self.reg8(target);
        }

        // Execute the opcode derived from group
        let operand_ready = if target.is_mem() { self.m == 2 } else { self.m == 1 };
        if operand_ready {
            self.execute_cb_opcode(group, arg);
        }

        // For opcodes that write back to memory we need to wait an extra cycle since we read on this M-cycle
        if group != 1 && self.m == 2 && target.is_mem() {
            return ExecState::Partial;
        }

        // Store the results
        if group != 1 {
            if target.is_mem() {
                self.store_mem(bus, target, self.w);
            } else {
                self.set_reg8(target, self.w);
            }
        }

        ExecState::Done
    }

    fn execute_cb_opcode(&mut self, group: u8, arg: u8) {
        match group {
            // Rotate/Shift depending on arg: RLC, RRC, RL, RR, SLA, SRA, SWAP, SRL
            0 => {
                let w = self.w;
                let c = self.is_set(flag::C) as u8;
                let (result, new_carry) = match arg {
                    0 => (w.rotate_left(1), w >> 7),
                    1 => (w.rotate_right(1), w & 1),
                    2 => ((w << 1) | c, w >> 7),
                    3 => ((w >> 1) | (c << 7), w & 1),
                    4 => (w << 1, w >> 7),
                    5 => ((w >> 1) | (w & 0x80), w & 1),
                    6 => (w.rotate_left(4), 0),
                    7 => (w >> 1, w & 1),
                    _ => unreachable!(),
                };
                self.w = result;
                self.update_flag(flag::Z, result == 0);
                self.clear_flag(flag::N);
                self.clear_flag(flag::H);
                self.update_flag(flag::C, new_carry != 0);
            },
            // Test Bit #arg
            1 => {
                self.update_flag(flag::Z, self.w & (1 << arg) == 0);
                self.clear_flag(flag::N);
                self.set_flag(flag::H);
            },
            // Clear Bit #arg
            2 => self.w = self.w & !(1 << arg),
            // Set bit #arg
            3 => self.w = self.w | (1 << arg),
            _ => unreachable!(),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::HwRegisters;

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
        hw: HwRegisters,
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

        fn hw_reg(&mut self) -> Option<&mut HwRegisters> {
            Some(&mut self.hw)
        }
    }

    fn set_reg(cpu: &mut CPU, hw: &mut HwRegisters, reg: &str, val: u16) {
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
            "i_f" => hw.set(INTERRUPT_FLAG, val as u8),
            "ie" => hw.set(INTERRUPT_ENABLE, val as u8),
            _ => panic!("unknown register {reg}"),
        }
    }

    fn regs(cpu: &CPU, hw: &mut HwRegisters) -> [(&'static str, u16); 13] {
        [
            ("a", cpu.a as u16), ("f", cpu.f as u16),
            ("b", cpu.b as u16), ("c", cpu.c as u16),
            ("d", cpu.d as u16), ("e", cpu.e as u16),
            ("h", cpu.h as u16), ("l", cpu.l as u16),
            ("sp", cpu.sp), ("pc", cpu.pc),
            ("ime", cpu.ime as u16),
            ("i_f", hw.get(INTERRUPT_FLAG) as u16),
            ("ie", hw.get(INTERRUPT_ENABLE) as u16),
        ]
    }

    fn run(given: &[(&str, u16)], cycles: Vec<Cycle>, expect: &[(&str, u16)]) {
        let mut cpu = CPU::new();
        cpu.pc = 0x0100;
        let mut bus = MockBus { cycles, pos: 0, hw: HwRegisters::new() };
        for &(reg, val) in given {
            set_reg(&mut cpu, &mut bus.hw, reg, val);
        }

        let mut expected = CPU::new();
        let mut expected_hw = HwRegisters::new();
        for (reg, val) in regs(&cpu, &mut bus.hw) {
            set_reg(&mut expected, &mut expected_hw, reg, val);
        }
        for &(reg, val) in expect {
            set_reg(&mut expected, &mut expected_hw, reg, val);
        }

        for cycle in 1.. {
            let pos = bus.pos;
            cpu.step(&mut bus);
            if bus.pos == pos && bus.cycles.get(pos) == Some(&Idle) {
                bus.pos += 1;
            }
            assert_eq!(bus.pos, cycle, "cycle {cycle} did not match expectation");
            if cpu.m == 0 && cpu.dispatch == 0 && bus.pos == bus.cycles.len() {
                break;
            }
        }

        assert_eq!(bus.pos, bus.cycles.len(), "unconsumed cycles: {:04X?}", &bus.cycles[bus.pos..]);
        let (actual, expected) = (regs(&cpu, &mut bus.hw), regs(&expected, &mut expected_hw));
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

        rlca:
            given { a: 0xA2 }
            cycles [R(0x100, op::RLCA)]
            expect { pc: 0x101, a: 0x45, f: flag::C.into() }

        ld_addr_sp:
            given { sp: 0xFFF0 }
            cycles [R(0x100, op::LD_ADDR_SP), R(0x101, 0x80), R(0x102, 0x80), W(0x8080, 0xF0), W(0x8081, 0xFF)]
            expect { pc: 0x103 }

        add_hl_bc:
            given { hl: 0xFFAB, bc: 0xFF3C, f: (flag::Z | flag::N).into() }
            cycles [R(0x100, op::ADD_HL_BC), Idle]
            expect { pc: 0x101, hl: 0xFEE7, f: (flag::Z | flag::H | flag::C).into() }

        ld_a_dbc:
            given { bc: 0x2030 }
            cycles [R(0x100, op::LD_A_DBC), R(0x2030, 42)]
            expect { pc: 0x101, a: 42 }

        ld_dde_a:
            given { a: 42, de: 0x11AA }
            cycles [R(0x100, op::LD_DDE_A), W(0x11AA, 42)]
            expect { pc: 0x101 }

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

        cpl:
            given { a: 0b1010 }
            cycles [R(0x100, op::CPL)]
            expect { pc: 0x101, a: 0b11110101, f: (flag::H | flag::N).into() }

        dec_dhl:
            given {hl: 0x8080 }
            cycles [R(0x100, op::DEC_DHL), R(0x8080, 0x20), W(0x8080, 0x1F)]
            expect { pc: 0x101, f: flag::H.into() }

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

        add_a_b:
            given { a: 8, b: 9, f: (flag::Z | flag::N | flag::C).into() }
            cycles [R(0x100, op::ADD_A_B)]
            expect { pc: 0x101, a: 17, f: flag::H.into() }

        adc_a_dhl:
            given { a: 130, hl: 0x8080, f: (flag::H | flag::C).into() }
            cycles [R(0x100, op::ADC_A_DHL), R(0x8080, 128)]
            expect { pc: 0x101, a: 3, f: flag::C.into() }

        and_a_b:
            given { a: 0b1010, b: 0b1_0000, f: (flag::C | flag::H).into() }
            cycles [R(0x100, op::AND_A_B)]
            expect { pc: 0x101, a: 0, f: (flag::Z | flag::H).into() }

        xor_a_dhl:
            given { a: 0b1010, hl: 0x8080 }
            cycles [R(0x100, op::XOR_A_DHL), R(0x8080, 0b1101)]
            expect { pc:0x101, a: 0b0111 }

        xor_a_a:
            given { a: 42 }
            cycles [R(0x100, op::XOR_A_A)]
            expect { pc:0x101, a: 0, f: flag::Z.into() }

        or_a_l:
            given { f: flag::N.into() }
            cycles [R(0x100, op::OR_A_L)]
            expect { pc: 0x101, a: 0, f: flag::Z.into() }

        or_a_dhl:
            given { hl: 0x02CC, f: flag::H.into() }
            cycles [R(0x100, op::OR_A_DHL), R(0x02CC, 42)]
            expect { pc: 0x101, a: 42, f: 0 }

        jp:
            cycles [R(0x100, op::JP), R(0x101, 0xCC), R(0x102, 0xDD), Idle]
            expect { pc: 0xDDCC }

        ret:
            given { sp: 0xFFE0 }
            cycles [R(0x100, op::RET), R(0xFFE0, 0x11), R(0xFFE1, 0xAA), Idle]
            expect { pc: 0xAA11, sp: 0xFFE2 }

        call_z_imm16_not_taken:
            given { sp: 0xFFF4 }
            cycles [R(0x100, op::CALL_Z_IMM16), R(0x101, 0x3C), R(0x102, 0x02)]
            expect { pc: 0x103, sp: 0xFFF4 }

        call_z_imm16_taken:
            given { sp: 0xFFF4, f: flag::Z.into() }
            cycles [R(0x100, op::CALL_Z_IMM16), R(0x101, 0x3C), R(0x102, 0x02), Idle, W(0xFFF3, 0x01), W(0xFFF2, 0x03)]
            expect { pc: 0x23C, sp: 0xFFF2 }

        call_imm16:
            given { sp: 0xFFF4 }
            cycles [R(0x100, op::CALL_IMM16), R(0x101, 0x3C), R(0x102, 0x02), Idle, W(0xFFF3, 0x01), W(0xFFF2, 0x03)]
            expect { pc: 0x023C, sp: 0xFFF2 }

        jp_c_imm16_not_taken:
            cycles [R(0x100, op::JP_C_IMM16), R(0x101, 0xC8), R(0x102, 0x02)]
            expect { pc: 0x103 }

        jp_c_imm16_taken:
            given { f: flag::C.into() }
            cycles [R(0x100, op::JP_C_IMM16), R(0x101, 0xC8), R(0x102, 0x02), Idle]
            expect { pc: 0x02C8 }

        ret_c_not_taken:
            cycles [R(0x100, op::RET_C), Idle]
            expect { pc: 0x101 }

        ret_c_taken:
            given { f: flag::C.into(), sp: 0xFFE0 }
            cycles [R(0x100, op::RET_C), Idle, R(0xFFE0, 0x11), R(0xFFE1, 0xAA), Idle]
            expect { pc: 0xAA11, sp: 0xFFE2 }

        reti:
            given { sp: 0xFFF0, ime: 0 }
            cycles [R(0x100, op::RETI), R(0xFFF0, 0x20), R(0xFFF1, 0x10), Idle]
            expect { pc: 0x1020, sp: 0xFFF2, ime: 1 }

        dispatch_vblank_interrupt:
            given { pc: 0x1234, sp: 0xFFF0, ime: 1, i_f: 0b101, ie: 0b101 }
            cycles [Idle, Idle, W(0xFFEF, 0x12), W(0xFFEE, 0x34), Idle, R(0x40, op::NOP)]
            expect { pc: 0x41, sp: 0xFFEE, ime: 0, i_f: 0b100 }

        ldh_imm8_a:
            given { a: 42 }
            cycles[R(0x100, op::LDH_IMM8_A), R(0x101, 0xF1), W(0xFFF1, 42)]
            expect { pc: 0x102 }

        pop_hl:
            given { sp: 0xFFA0 }
            cycles [R(0x100, op::POP_HL), R(0xFFA0, 0x88), R(0xFFA1, 0x3C)]
            expect { pc: 0x101, sp: 0xFFA2, hl: 0x3C88 }

        ldh_deref_c_a:
            given { a: 42, c: 0xAA }
            cycles [R(0x100, op::LDH_C_A), W(0xFFAA, 42)]
            expect { pc: 0x101 }

        and_a_imm8:
            given { a: 0b1010 }
            cycles [R(0x100, op::AND_A_IMM8), R(0x101, 0b111)]
            expect { pc: 0x102, a: 0b10, f: flag::H.into() }

        ld_addr_a:
            given { a: 42 }
            cycles[R(0x100, op::LD_ADDR_A), R(0x101, 0xAF), R(0x102, 0xC0), W(0xC0AF, 42)]
            expect{ pc: 0x103 }

        rst_28:
            given { sp: 0xFFFA }
            cycles [R(0x100, op::RST_28), Idle, W(0xFFF9, 0x01), W(0xFFF8, 0x01)]
            expect { pc: 0x28, sp: 0xFFF8 }

        ldh_a_imm8:
            cycles[R(0x100, op::LDH_A_IMM8), R(0x101, 0xCC), R(0xFFCC, 42)]
            expect { pc: 0x102, a: 42 }

        pop_af:
            given { sp: 0xFFA0, f: flag::N.into() }
            cycles [R(0x100, op::POP_AF), R(0xFFA0, 0x88), R(0xFFA1, 0x3C)]
            expect { pc: 0x101, sp: 0xFFA2, af: 0x3C80, f: flag::Z.into() }

        di:
            given { ime: true as u16 }
            cycles [R(0x100, op::DI)]
            expect { pc: 0x101, ime: 0 }

        push_af:
            given { af: 0x3C40, sp: 0xFFA2 }
            cycles [R(0x100, op::PUSH_AF), Idle, W(0xFFA1, 0x3C), W(0xFFA0, 0x40)]
            expect { pc: 0x101, sp: 0xFFA0, f: flag::N.into() }

        ld_a_addr:
            cycles[R(0x100, op::LD_A_ADDR), R(0x101, 0xAF), R(0x102, 0xC0), R(0xC0AF, 42)]
            expect{ pc: 0x103, a: 42 }

        ld_sp_hl:
            given { hl: 42, sp: 0xFFFA }
            cycles [R(0x100, op::LD_SP_HL), Idle]
            expect { pc: 0x101, sp: 42 }

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

        prefix_rl_b:
            given { b: 0x80, f: flag::C.into() }
            cycles [R(0x100, op::PREFIX), R(0x101, 0x10)]
            expect { pc: 0x102, b: 0x01, f: flag::C.into() }

        prefix_srl_b:
            given { b: 0x01 }
            cycles [R(0x100, op::PREFIX), R(0x101, 0x38)]
            expect { pc: 0x102, b: 0, f: (flag::Z | flag::C).into() }

        prefix_bit_7_h:
            given { h: 0x80 }
            cycles [R(0x100, op::PREFIX), R(0x101, 0x7C)]
            expect { pc: 0x102, f: flag::H.into() }

        prefix_bit_7_h_zero:
            given { h: 0 }
            cycles [R(0x100, op::PREFIX), R(0x101, 0x7C)]
            expect { pc: 0x102, f: (flag::Z | flag::H).into() }

        prefix_set_0_dhl:
            given { hl: 0x8080 }
            cycles [R(0x100, op::PREFIX), R(0x101, 0xC6), R(0x8080, 0), W(0x8080, 1)]
            expect { pc: 0x102 }

        prefix_bit_0_dhl:
            given { hl: 0x8080 }
            cycles [R(0x100, op::PREFIX), R(0x101, 0x46), R(0x8080, 1)]
            expect { pc: 0x102, f: flag::H.into() }

        prefix_sla_dhl:
            given { hl: 0x8080 }
            cycles [R(0x100, op::PREFIX), R(0x101, 0x26), R(0x8080, 0x81), W(0x8080, 0x02)]
            expect { pc: 0x102, f: flag::C.into() }
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
