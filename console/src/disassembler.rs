use crate::opcodes::{CB_TARGETS, OPCODES, OpInfo, Operand, op};
use std::fmt::Write;

static UNKNOWN_OPCODE: OpInfo = OpInfo {
    opcode: 0xFD,
    mnemonic: "UNK",
    operands: &[],
};

const CB_SHIFT_MNEMONICS: [&str; 8] = ["RLC", "RRC", "RL", "RR", "SLA", "SRA", "SWAP", "SRL"];

// Returns the string representation of the next instruction as
// well as the address of the next instruction in the code
pub fn disassemble_next(code: &[u8], mut pc: u16) -> (String, u16) {
    let op = code[pc as usize];
    pc += 1;

    if op == op::PREFIX {
        let cb_op = code[pc as usize];
        return (disassemble_cb(cb_op), pc + 1);
    }

    let op_info = lookup_opcode(op);
    let mut disassemble = String::from(op_info.mnemonic);

    // TODO: temporary
    if op_info.opcode == UNKNOWN_OPCODE.opcode {
        write!(disassemble, " (op=${op:02X})").unwrap();
    }

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

fn disassemble_cb(op: u8) -> String {
    let target = CB_TARGETS[(op & 0b111) as usize];
    let arg = (op >> 3) & 0b111;
    let group = op >> 6;

    match group {
        0 => format!("{} {target}", CB_SHIFT_MNEMONICS[arg as usize]),
        1 => format!("BIT {arg}, {target}"),
        2 => format!("RES {arg}, {target}"),
        3 => format!("SET {arg}, {target}"),
        _ => unreachable!(),
    }
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

    #[test]
    fn cb_shift_instruction() {
        let code = [op::PREFIX, 0x26];
        let pc: u16 = 0;
        let (s, new_pc) = disassemble_next(&code, pc);
        assert_eq!(new_pc, 2);
        assert_eq!(s, "SLA [HL]");
    }

    #[test]
    fn cb_bit_instruction() {
        let code = [op::PREFIX, 0x7C];
        let pc: u16 = 0;
        let (s, new_pc) = disassemble_next(&code, pc);
        assert_eq!(new_pc, 2);
        assert_eq!(s, "BIT 7, H");
    }
}
