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
    debug: bool,
    break_counter: i32, // debug break in this many instructions or -1
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
    pub fn step(&mut self, bus: &mut impl Bus) -> StopReason {
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
    fn execute_cycle(&mut self, bus: &mut impl Bus) -> ExecState {
        match self.ir {
            op::NOP => self.nop(bus),
            op::DEC_B => self.dec_reg8(bus, Operand::B),
            op::LD_B_IMM8 => self.ld_byte(bus, Operand::B, Operand::Imm8),
            op::LD_C_IMM8 => self.ld_byte(bus, Operand::C, Operand::Imm8),
            op::JR_NZ_IMM8 => self.jr(bus, |s| !s.is_set(flag::Z)),
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
        println!("NOP");
        ExecState::Done
    }

    fn inc_a(&mut self, _bus: &mut impl Bus) -> ExecState {
        let value = self.a;
        let result = value.wrapping_add(1);
        self.update_flag(flag::Z, result == 0);
        self.clear_flag(flag::N);
        self.update_flag(flag::H, (value & 0x0F) == 0x0F);
        self.a = result;
        println!("INC A (A = {:02X})", self.a);
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

        println!("XOR A, {} (A = {:02X})", operand, self.a);
        ExecState::Done
    }

    fn jp(&mut self, bus: &mut impl Bus) -> ExecState {
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

    fn jr<F>(&mut self, bus: &mut impl Bus, cond: F) -> ExecState
    where
        F: Fn(&Self) -> bool,
    {
        match self.m {
            0 => ExecState::Partial,
            1 => {
                self.z = self.load_pc(bus);
                println!("{}", cond(self));
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
                println!("LD {}, {:04X}", dst, val);
                ExecState::Done
            },
            _ => unreachable!(),
        }
    }

    fn ld_byte(&mut self, bus: &mut impl Bus, dst: Operand, src: Operand) -> ExecState {
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

    fn dec_reg8(&mut self, bus: &mut impl Bus, reg: Operand) -> ExecState {
        let new_val = self.reg8(reg).wrapping_sub(1);
        self.set_reg8(reg, new_val);

        self.update_flag(flag::Z, new_val == 0);
        self.update_flag(flag::H, (new_val & 0x0F) == 0x0F);
        self.set_flag(flag::N);

        println!("DEC {reg} ({reg} = {new_val:02X})");
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
            _ => panic!("unknown register {reg}"),
        }
    }

    fn regs(cpu: &CPU) -> [(&'static str, u16); 10] {
        [
            ("a", cpu.a as u16), ("f", cpu.f as u16),
            ("b", cpu.b as u16), ("c", cpu.c as u16),
            ("d", cpu.d as u16), ("e", cpu.e as u16),
            ("h", cpu.h as u16), ("l", cpu.l as u16),
            ("sp", cpu.sp), ("pc", cpu.pc),
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
            if cpu.m == 0 {
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

        ldi_a_hl:
            given  { hl: 0xC000 }
            cycles [R(0x0100, op::LDI_A_HL), R(0xC000, 42)]
            expect { pc: 0x0101, a: 42, hl: 0xC001 }

        ldd_hl_a:
            given  { hl: 0xC000, a: 42 }
            cycles [R(0x0100, op::LDD_HL_A), W(0xC000, 42)]
            expect { pc: 0x0101, a: 42, hl: 0xBFFF }

        ldd_a_hl:
            given  { hl: 0xC000 }
            cycles [R(0x0100, op::LDD_A_HL), R(0xC000, 42)]
            expect { pc: 0x0101, a: 42, hl: 0xBFFF }

        inc_a:
            given { a: 0x3F, f: (flag::Z | flag::N).into() }
            cycles [R(0x100, op::INC_A)]
            expect { pc: 0x101, a: 0x40, f: flag::H.into() }

        xor_aa:
            given { a: 42 }
            cycles [R(0x100, op::XOR_AA)]
            expect { pc:0x101, a: 0, f: flag::Z.into() }

        jp:
            cycles [R(0x100, op::JP), R(0x101, 0xCC), R(0x102, 0xDD), Idle]
            expect { pc: 0xDDCC }
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
