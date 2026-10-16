use core::fmt;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    // Registers
    AF, A, BC, B, C, DE, D, E, HL, H, L, DerefHL, DerefHLPlus, DerefHLMinus, SP,

    // Immediates
    Imm8, Imm16
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
    DEC_B       = 0x05, "DEC", B;
    LD_B_IMM8   = 0x06, "LD", B, Imm8;
    DEC_C       = 0x0D, "DEC", C;
    LD_C_IMM8   = 0x0E, "LD", C, Imm8;
    JR_NZ_IMM8  = 0x20, "JR NZ,", Imm8;
    LD_HL_IMM16 = 0x21, "LD", HL, Imm16;
    LDI_HL_A    = 0x22, "LD", DerefHLPlus, A;
    LDI_A_HL    = 0x2A, "LD", A, DerefHLPlus;
    LDD_HL_A    = 0x32, "LD", DerefHLMinus, A;
    LDD_A_HL    = 0x3A, "LD", A, DerefHLMinus;
    INC_A       = 0x3C, "INC", A;
    LD_A_IMM8   = 0x3E, "LD", A, Imm8;
    XOR_AA      = 0xAF, "XOR", A, A;
    JP          = 0xC3, "JP", Imm16;
}

impl Operand {
    pub fn size(self) -> u16 {
        match self {
            Operand::Imm8 => 1,
            Operand::Imm16 => 2,
            _ => 0,
        }
    }

    pub fn is_mem(self) -> bool {
        matches!(self, Operand::Imm8 | Operand::Imm16 | Operand::DerefHL | Operand::DerefHLPlus |
            Operand::DerefHLMinus)
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
        };
        write!(f, "{s}")
    }
}
