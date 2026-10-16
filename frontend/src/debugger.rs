use std::{io::{self, Write}};
use console::{GameBoy, disassembler::disassemble_next, gameboy::StopReason};

pub struct Debugger {
    gameboy: GameBoy,
}

pub enum Action {
    Continue,
    Step(i32),
    Stop,
}

impl Debugger {
    pub fn new(gameboy: GameBoy) -> Self {
        Self {
            gameboy,
        }
    }

    pub fn start(&mut self) {
        self.gameboy.set_debugged(true);
        self.gameboy.power_on();
        loop {
            match self.gameboy.run() {
                StopReason::Breakpoint(bp) => self.on_pause(Some(bp)),
                StopReason::Step => self.on_pause(None),
                StopReason::Halt => break,
                _ => panic!("Unhandled StopReason"),
            }
        }
    }

    fn on_pause(&mut self, breakpoint: Option<u32>) {
        let pc = self.gameboy.cpu.pc();
        if let Some(bp) = breakpoint {
            println!("Break at {} at {:04X}", bp, pc);
        }
        if let Some(cart) = &self.gameboy.cartridge {
            let (text, _) = disassemble_next(&cart.rom, pc);
            println!("=> ${pc:04X}: {text}");
        }

        match self.read_eval_loop() {
            Action::Continue => (),
            Action::Step(instructions) => self.gameboy.cpu.debug_stepi(instructions),
            Action::Stop => std::process::exit(0),
        }
    }

    fn read_eval_loop(&mut self) -> Action {
        loop {
            print!("(dbg) ");
            io::stdout().flush().unwrap();

            let mut line = String::new();
            if io::stdin().read_line(&mut line).unwrap() == 0 {
                return Action::Stop;
            }

            let cmd = line.trim();
            let mut cmd_parts = cmd.split_whitespace();
            match (cmd_parts.next(), cmd_parts.next()) {
                (Some("c" | "continue"), _) => return Action::Continue,
                (Some("s" | "step"), arg) => match parse_count(arg) {
                    Ok(n) => return Action::Step(n),
                    Err(e) => println!("Error: {e}"),
                },
                (Some("h" | "help"), _) => println!("Available commands: (c)ontinue, (s)tep"),
                (Some(cmd), _) => println!("Undefined command: {cmd}. Try \"help\"."),
                (None, _) => {},
            }
        }
    }
}

fn parse_count(arg: Option<&str>) -> Result<i32, String> {
    match arg {
        None => Ok(1),
        Some(s) => s.parse::<i32>().map_err(|e| format!("count parsed yielded {e}")),
    }
}
