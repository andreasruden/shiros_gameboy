pub enum Operand {
    // Registers
    A, B, C, D, E, H, L, HL, DerefHL, SP,

    // Immediates
    Imm8, Imm16
}

impl Operand {
    fn size(self) -> usize {
        match self {
            Operand::Imm8 => 1,
            Operand::Imm16 => 2,
            _ => 0,
        }
    }
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
