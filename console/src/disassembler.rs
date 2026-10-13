use crate::opcodes::{OPCODES, OpInfo};
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
        write!(disassemble, " {}", operand).unwrap();
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
    use super::*;

    #[test]
    fn simple_instruction() {
        let code = [0 as u8];
        let pc: u16 = 0;
        let (s, new_pc) = disassemble_next(&code, pc);
        assert_eq!(new_pc, 1);
        assert_eq!(s, "NOP");
    }

    #[test]
    fn instruction_with_hardcoded_operands() {
        let code = [0xAF as u8];
        let pc: u16 = 0;
        let (s, new_pc) = disassemble_next(&code, pc);
        assert_eq!(new_pc, 1);
        assert_eq!(s, "XOR A, A");
    }
}
