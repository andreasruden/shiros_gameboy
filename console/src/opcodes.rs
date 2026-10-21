use core::fmt;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    // Registers
    AF, A, BC, B, C, DE, D, E, HL, H, L, DerefHL, DerefHLPlus, DerefHLMinus, SP,

    // Immediates
    Imm8, Imm16, Addr
}

macro_rules! opcodes {
    ($($name:ident = $byte:literal, $mnem:literal $(, $operand:ident)*;)*) => {
        pub mod op {
            $(pub const $name: u8 = $byte;)*
        }

        pub struct OpInfo {
            pub opcode: u8,
            pub mnemonic: &'static str,
            pub operands: &'static [Operand],
        }

        pub static OPCODES: &[OpInfo] = &[
            $(OpInfo {
                opcode: $byte,
                mnemonic: $mnem,
                operands: &[$(Operand::$operand),*],
            },)*
        ];
    };
}

opcodes! {
    NOP         = 0x00, "NOP";
    INC_B       = 0x04, "INC", B;
    DEC_B       = 0x05, "DEC", B;
    LD_B_IMM8   = 0x06, "LD", B, Imm8;
    INC_C       = 0x0C, "INC", C;
    DEC_C       = 0x0D, "DEC", C;
    LD_C_IMM8   = 0x0E, "LD", C, Imm8;
    INC_D       = 0x14, "INC", D;
    DEC_D       = 0x15, "DEC", D;
    INC_E       = 0x1C, "INC", E;
    DEC_E       = 0x1D, "DEC", E;
    JR_NZ_IMM8  = 0x20, "JR NZ,", Imm8;
    LD_HL_IMM16 = 0x21, "LD", HL, Imm16;
    LDI_HL_A    = 0x22, "LD", DerefHLPlus, A;
    INC_H       = 0x24, "INC", H;
    DEC_H       = 0x25, "DEC", H;
    LDI_A_HL    = 0x2A, "LD", A, DerefHLPlus;
    INC_L       = 0x2C, "INC", L;
    DEC_L       = 0x2D, "DEC", L;
    LD_SP_IMM16 = 0x31, "LD", SP, A;
    LDD_HL_A    = 0x32, "LD", DerefHLMinus, A;
    LD_DHL_IMM8 = 0x36, "LD", DerefHL, Imm8;
    LDD_A_HL    = 0x3A, "LD", A, DerefHLMinus;
    INC_A       = 0x3C, "INC", A;
    DEC_A       = 0x3D, "DEC", A;
    LD_A_IMM8   = 0x3E, "LD", A, Imm8;
    LD_DHL_A    = 0x77, "LD", DerefHL, A;
    XOR_AA      = 0xAF, "XOR", A, A;
    JP          = 0xC3, "JP", Imm16;
    LDH_IMM8_A  = 0xE0, "LDH", Imm8, A;
    LDH_C_A     = 0xE2, "LDH", C, A;
    LD_ADDR_A   = 0xEA, "LD", Addr, A;
    LDH_A_IMM8  = 0xF0, "LDH", A, Imm8;
    LDH_A_C     = 0xF2, "LDH", A, C;
    DI          = 0xF3, "DI";
    LD_A_ADDR   = 0xFA, "LD", A, Addr;
    EI          = 0xFB, "EI";
    CP_A_IMM8   = 0xFE, "CP", A, Imm8;
}

impl Operand {
    pub fn size(self) -> u16 {
        match self {
            Operand::Imm8 => 1,
            Operand::Imm16 => 2,
            Operand::Addr => 2,
            _ => 0,
        }
    }

    pub fn is_mem(self) -> bool {
        matches!(self, Operand::Imm8 | Operand::Imm16 | Operand::DerefHL | Operand::DerefHLPlus |
            Operand::DerefHLMinus | Operand::Addr)
    }
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Operand::AF => "AF",
            Operand::A => "A",
            Operand::BC => "BC",
            Operand::B => "B",
            Operand::C => "C",
            Operand::DE => "DE",
            Operand::D => "D",
            Operand::E => "E",
            Operand::HL => "HL",
            Operand::H => "H",
            Operand::L => "L",
            Operand::DerefHL => "[HL]",
            Operand::DerefHLPlus => "[HL+]",
            Operand::DerefHLMinus => "[HL-]",
            Operand::SP => "SP",
            Operand::Imm8 => "imm8",
            Operand::Imm16 => "imm16",
            Operand::Addr => "[a16]",
        };
        write!(f, "{s}")
    }
}
