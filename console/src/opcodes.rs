use core::fmt;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    // Registers
    AF, A, BC, B, C, DE, D, E, HL, H, L, SP,

    // Registers used as addresses
    DerefBC, DerefDE, DerefHL, DerefHLPlus, DerefHLMinus,

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

// BC, DE, HL, SP

opcodes! {
    NOP         = 0x00, "NOP";
    LD_BC_IMM16 = 0x01, "LD", BC, Imm16;
    LD_DBC_A    = 0x02, "LD", DerefBC, A;
    INC_BC      = 0x03, "INC", BC;
    INC_B       = 0x04, "INC", B;
    DEC_B       = 0x05, "DEC", B;
    LD_B_IMM8   = 0x06, "LD", B, Imm8;
    LD_A_DBC    = 0x0A, "LD", A, DerefBC;
    DEC_BC      = 0x0B, "DEC", BC;
    INC_C       = 0x0C, "INC", C;
    DEC_C       = 0x0D, "DEC", C;
    LD_C_IMM8   = 0x0E, "LD", C, Imm8;

    LD_DE_IMM16 = 0x11, "LD", DE, Imm16;
    LD_DDE_A    = 0x12, "LD", DerefDE, A;
    INC_DE      = 0x13, "INC", DE;
    INC_D       = 0x14, "INC", D;
    DEC_D       = 0x15, "DEC", D;
    LD_D_IMM8   = 0x16, "LD", D, Imm8;
    JR_IMM8     = 0x18, "JR", Imm8;
    LD_A_DDE    = 0x1A, "LD", A, DerefDE;
    DEC_DE      = 0x1B, "DEC", DE;
    INC_E       = 0x1C, "INC", E;
    DEC_E       = 0x1D, "DEC", E;
    LD_E_IMM8   = 0x1E, "LD", E, Imm8;

    JR_NZ_IMM8  = 0x20, "JR NZ,", Imm8;
    LD_HL_IMM16 = 0x21, "LD", HL, Imm16;
    LDI_HL_A    = 0x22, "LD", DerefHLPlus, A;
    INC_HL      = 0x23, "INC", HL;
    INC_H       = 0x24, "INC", H;
    DEC_H       = 0x25, "DEC", H;
    LD_H_IMM8   = 0x26, "LD", H, Imm8;
    JR_Z_IMM8   = 0x28, "JR Z,", Imm8;
    LDI_A_HL    = 0x2A, "LD", A, DerefHLPlus;
    DEC_HL      = 0x2B, "DEC", HL;
    INC_L       = 0x2C, "INC", L;
    DEC_L       = 0x2D, "DEC", L;
    LD_L_IMM8   = 0x2E, "LD", L, Imm8;

    JR_NC_IMM8  = 0x30, "JR NC,", Imm8;
    LD_SP_IMM16 = 0x31, "LD", SP, Imm16;
    LDD_HL_A    = 0x32, "LD", DerefHLMinus, A;
    INC_SP      = 0x33, "INC", SP;
    LD_DHL_IMM8 = 0x36, "LD", DerefHL, Imm8;
    JR_C_IMM8   = 0x38, "JR C,", Imm8;
    LDD_A_HL    = 0x3A, "LD", A, DerefHLMinus;
    DEC_SP      = 0x3B, "DEC", SP;
    INC_A       = 0x3C, "INC", A;
    DEC_A       = 0x3D, "DEC", A;
    LD_A_IMM8   = 0x3E, "LD", A, Imm8;

    LD_B_B      = 0x40, "LD", B, B;
    LD_B_C      = 0x41, "LD", B, C;
    LD_B_D      = 0x42, "LD", B, D;
    LD_B_E      = 0x43, "LD", B, E;
    LD_B_H      = 0x44, "LD", B, H;
    LD_B_L      = 0x45, "LD", B, L;
    LD_B_A      = 0x47, "LD", B, A;
    LD_C_B      = 0x48, "LD", C, B;
    LD_C_C      = 0x49, "LD", C, C;
    LD_C_D      = 0x4A, "LD", C, D;
    LD_C_E      = 0x4B, "LD", C, E;
    LD_C_H      = 0x4C, "LD", C, H;
    LD_C_L      = 0x4D, "LD", C, L;
    LD_C_A      = 0x4F, "LD", C, A;

    LD_D_B      = 0x50, "LD", D, B;
    LD_D_C      = 0x51, "LD", D, C;
    LD_D_D      = 0x52, "LD", D, D;
    LD_D_E      = 0x53, "LD", D, E;
    LD_D_H      = 0x54, "LD", D, H;
    LD_D_L      = 0x55, "LD", D, L;
    LD_D_A      = 0x57, "LD", D, A;
    LD_E_B      = 0x58, "LD", E, B;
    LD_E_C      = 0x59, "LD", E, C;
    LD_E_D      = 0x5A, "LD", E, D;
    LD_E_E      = 0x5B, "LD", E, E;
    LD_E_H      = 0x5C, "LD", E, H;
    LD_E_L      = 0x5D, "LD", E, L;
    LD_E_A      = 0x5F, "LD", E, A;

    LD_H_B      = 0x60, "LD", H, B;
    LD_H_C      = 0x61, "LD", H, C;
    LD_H_D      = 0x62, "LD", H, D;
    LD_H_E      = 0x63, "LD", H, E;
    LD_H_H      = 0x64, "LD", H, H;
    LD_H_L      = 0x65, "LD", H, L;
    LD_H_A      = 0x67, "LD", H, A;
    LD_L_B      = 0x68, "LD", L, B;
    LD_L_C      = 0x69, "LD", L, C;
    LD_L_D      = 0x6A, "LD", L, D;
    LD_L_E      = 0x6B, "LD", L, E;
    LD_L_H      = 0x6C, "LD", L, H;
    LD_L_L      = 0x6D, "LD", L, L;
    LD_L_A      = 0x6F, "LD", L, A;

    LD_DHL_A    = 0x77, "LD", DerefHL, A;
    LD_A_B      = 0x78, "LD", A, B;
    LD_A_C      = 0x79, "LD", A, C;
    LD_A_D      = 0x7A, "LD", A, D;
    LD_A_E      = 0x7B, "LD", A, E;
    LD_A_H      = 0x7C, "LD", A, H;
    LD_A_L      = 0x7D, "LD", A, L;
    LD_A_A      = 0x7F, "LD", A, A;

    AND_A_B     = 0xA0, "AND", A, B;
    AND_A_C     = 0xA1, "AND", A, C;
    AND_A_D     = 0xA2, "AND", A, D;
    AND_A_E     = 0xA3, "AND", A, E;
    AND_A_H     = 0xA4, "AND", A, H;
    AND_A_L     = 0xA5, "AND", A, L;
    AND_A_DHL   = 0xA6, "AND", A, DerefHL;
    AND_A_A     = 0xA7, "AND", A, A;
    XOR_AA      = 0xAF, "XOR", A, A;

    OR_A_B      = 0xB0, "OR", A, B;
    OR_A_C      = 0xB1, "OR", A, C;
    OR_A_D      = 0xB2, "OR", A, D;
    OR_A_E      = 0xB3, "OR", A, E;
    OR_A_H      = 0xB4, "OR", A, H;
    OR_A_L      = 0xB5, "OR", A, L;
    OR_A_DHL    = 0xB6, "OR", A, DerefHL;
    OR_A_A      = 0xB7, "OR", A, A;

    RET_NZ      = 0xC0, "RET NZ";
    JP          = 0xC3, "JP", Imm16;
    RET_Z       = 0xC8, "RET Z";
    RET         = 0xC9, "RET";
    CALL_IMM16  = 0xCD, "CALL", Imm16;

    RET_NC      = 0xD0, "RET NC";
    RET_C       = 0xD8, "RET C";

    LDH_IMM8_A  = 0xE0, "LDH", Imm8, A;
    LDH_C_A     = 0xE2, "LDH", C, A;
    AND_A_IMM8  = 0xE6, "AND", A, Imm8;
    LD_ADDR_A   = 0xEA, "LD", Addr, A;

    LDH_A_IMM8  = 0xF0, "LDH", A, Imm8;
    LDH_A_C     = 0xF2, "LDH", A, C;
    DI          = 0xF3, "DI";
    OR_A_IMM8   = 0xF6, "OR", A, Imm8;
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
        matches!(self, Operand::Imm8 | Operand::Imm16 | Operand::DerefHL | Operand::DerefBC | Operand::DerefDE |
            Operand::DerefHLPlus | Operand::DerefHLMinus | Operand::Addr)
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
            Operand::DerefBC => "[BC]",
            Operand::DE => "DE",
            Operand::D => "D",
            Operand::E => "E",
            Operand::DerefDE => "[DE]",
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
