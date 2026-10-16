use crate::opcodes::{OPCODES, OpInfo, Operand};
use std::fmt::Write;

static UNKNOWN_OPCODE: OpInfo = OpInfo {
    opcode: 0xFD,
    mnemonic: "UNK",
    operands: &[],
};

// Returns the string representation of the next instruction as
// well as the address of the next instruction in the code
pub fn disassemble_next(code: &[u8], mut pc: u16) -> (String, u16) {
    let op = code[pc as usize];
    pc += 1;
    let op_info = lookup_opcode(op);

    let mut disassemble = String::from(op_info.mnemonic);

    let mut first = true;
    for operand in op_info.operands {
        if !first {
            disassemble += ",";
        } else {
            first = false;
        }
        match operand {
            Operand::Imm8 => write!(disassemble, " ${:02X}", code[pc as usize]).unwrap(),
            Operand::Imm16 => {
                let immediate = u16::from_le_bytes([code[pc as usize], code[(pc + 1) as usize]]);
                write!(disassemble, " ${:04X}", immediate).unwrap();
            },
            _ => write!(disassemble, " {}", operand).unwrap(),
        };

        pc += operand.size();
    }

    (disassemble, pc)
}

fn lookup_opcode(op: u8) -> &'static OpInfo {
    // Once OPCODES are fully filled in we can do:
    // OPCODES[op as usize], but for now we must search
    for op_info in OPCODES {
        if op_info.opcode == op {
            return op_info
        }
    }
    // TODO: This is just a temporary workaround to not fail fataly when trying to disassemble
    &UNKNOWN_OPCODE
}

#[cfg(test)]
mod tests {
    use crate::opcodes::op;
    use super::*;

    #[test]
    fn simple_instruction() {
        let code = [op::NOP as u8];
        let pc: u16 = 0;
        let (s, new_pc) = disassemble_next(&code, pc);
        assert_eq!(new_pc, 1);
        assert_eq!(s, "NOP");
    }

    #[test]
    fn instruction_with_hardcoded_operands() {
        let code = [op::XOR_AA as u8];
        let pc: u16 = 0;
        let (s, new_pc) = disassemble_next(&code, pc);
        assert_eq!(new_pc, 1);
        assert_eq!(s, "XOR A, A");
    }

    #[test]
    fn instruction_with_imm8_value() {
        let code = [op::JR_NZ_IMM8 as u8, 0x10 as u8];
        let pc: u16 = 0;
        let (s, new_pc) = disassemble_next(&code, pc);
        assert_eq!(new_pc, 2);
        assert_eq!(s, "JR NZ, $10");
    }

    #[test]
    fn instruction_with_imm16_value() {
        let code = [op::LD_HL_IMM16 as u8, 0xBB as u8, 0xAA as u8];
        let pc: u16 = 0;
        let (s, new_pc) = disassemble_next(&code, pc);
        assert_eq!(new_pc, 3);
        assert_eq!(s, "LD HL, $AABB");
    }
}
