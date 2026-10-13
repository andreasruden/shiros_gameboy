use core::fmt;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    // Registers
    A, B, C, D, E, H, L, HL, DerefHL, SP,

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
    NOP        = 0x00, "NOP";
    INC_A      = 0x3C, "INC", A;
    XOR_AA     = 0xAF, "XOR", A, A;
    JP         = 0xC3, "JP", Imm16;
}

impl Operand {
    pub fn size(self) -> u16 {
        match self {
            Operand::Imm8 => 1,
            Operand::Imm16 => 2,
            _ => 0,
        }
    }
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Operand::A => "A",
            Operand::B => "B",
            Operand::C => "C",
            Operand::D => "D",
            Operand::E => "E",
            Operand::H => "H",
            Operand::L => "L",
            Operand::HL => "HL",
            Operand::DerefHL => "[HL]",
            Operand::SP => "SP",
            Operand::Imm8 => "Imm8",
            Operand::Imm16 => "Imm16",
        };
        write!(f, "{s}")
    }
}
