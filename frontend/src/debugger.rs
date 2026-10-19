use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use console::{GameBoy, disassembler::disassemble_next, gameboy::StopReason};

static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_sigint(_: libc::c_int) {
    STOP_REQUESTED.store(true, Ordering::Relaxed);
}

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
        self.gameboy.cpu.debug_set_stop_request_handle(&STOP_REQUESTED);
        unsafe { libc::signal(libc::SIGINT, on_sigint as *const () as libc::sighandler_t) };
        self.gameboy.power_on();
        loop {
            match self.gameboy.run() {
                StopReason::Breakpoint(addr) => self.on_pause(Some(addr)),
                StopReason::Step => self.on_pause(None),
                StopReason::StopRequested => {
                    println!();
                    self.on_pause(None);
                }
                StopReason::Halt => break,
                _ => panic!("Unhandled StopReason"),
            }
        }
    }

    fn on_pause(&mut self, breakpoint: Option<u16>) {
        let pc = self.gameboy.cpu.pc();
        if let Some(addr) = breakpoint {
            println!("Break at {:04X}", addr);
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
            match (cmd_parts.next(), cmd_parts.next(), cmd_parts.next()) {
                (Some("c" | "continue"), _, _) => return Action::Continue,
                (Some("s" | "step"), arg, _) => match parse_count(arg) {
                    Ok(n) => return Action::Step(n),
                    Err(e) => println!("Error: {e}"),
                },
                (Some("b" | "break"), arg, _) => match parse_addr(arg) {
                    Ok(addr) => self.gameboy.cpu.debug_set_breakpoint(addr),
                    Err(e) => println!("Error: {e}"),
                },
                (Some("d" | "delete"), arg, _) => match parse_addr(arg) {
                    Ok(addr) => self.gameboy.cpu.debug_delete_breakpoint(addr),
                    Err(e) => println!("Error: {e}"),
                },
                (Some("r" | "registers"), _, _) => self.print_cpu_registers(),
                (Some("m" | "memory"), addr, _) => match parse_addr(addr) {
                    Ok(addr) => println!("{}", self.read_memory(addr)),
                    Err(e) => println!("Error: {e}"),
                },
                (Some("w" | "write"), arg1, arg2) => match (arg1, arg2) {
                    (Some(arg1), Some(arg2)) => self.write_cmd(arg1, arg2),
                    _ => println!("Error: command is \"write location value\""),
                },
                (Some("h" | "help"), _, _) => println!("Available commands: (c)ontinue, (s)tep"),
                (Some(cmd), _, _) => println!("Undefined command: {cmd}. Try \"help\"."),
                (None, _, _) => {},
            }
        }
    }

    fn write_cmd(&mut self, arg1: &str, arg2: &str) {
        let val: u8;
        match parse_u8(arg2) {
            Ok(v) => val = v,
            Err(e) => { println!("Error in value: {e}"); return; }
        }

        match arg1 {
            _ => match parse_u16(arg1) {
                Ok(addr) => self.write_memory(addr, val),
                Err(e) => println!("Error in location: {e}"),
            }
        }
    }

    fn print_cpu_registers(&mut self) {
        let cpu = &self.gameboy.cpu;
        println!("AF={:04X}  BC={:04X}  DE={:04X}  HL={:04X}", cpu.af(), cpu.bc(), cpu.de(), cpu.hl());
        println!("SP={:04X}  PC={:04X}  F={}", cpu.sp(), cpu.pc(), flags_string(cpu.f()));
    }

    fn write_memory(&mut self, addr: u16, byte: u8) {
        match addr {
            0x0000..=0x3FFF => self.gameboy.cartridge.as_mut().unwrap().rom[addr as usize] = byte,
            0xFF00..=0xFF7F => self.gameboy.hw_reg.set(addr, byte),
            _ => todo!(),
        };
        println!("[${addr:04X}] = {}", self.read_memory(addr));
    }

    fn read_memory(&mut self, addr: u16) -> String {
        let byte: u8;
        match addr {
            0x0000..=0x3FFF => byte = self.gameboy.cartridge.as_ref().unwrap().rom[addr as usize],
            0xFF00..=0xFF7F => byte = self.gameboy.hw_reg.get(addr),
            _ => todo!(),
        };
        format!("${byte:02X}")
    }
}

fn parse_count(arg: Option<&str>) -> Result<i32, String> {
    match arg {
        None => Ok(1),
        Some(s) => s.parse::<i32>().map_err(|e| format!("count parsed yielded {e}")),
    }
}

fn parse_addr(arg: Option<&str>) -> Result<u16, String> {
    match arg {
        None => Err(String::from("Must specify an address")),
        Some(s) => parse_u16(s).map_err(|e| format!("parse addr yielded {e}")),
    }
}

fn parse_u8(s: &str) -> Result<u8, std::num::ParseIntError> {
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).or_else(|| s.strip_prefix('$')) {
        Some(hex) => u8::from_str_radix(hex, 16),
        None => s.parse::<u8>(),
    }
}

fn parse_u16(s: &str) -> Result<u16, std::num::ParseIntError> {
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).or_else(|| s.strip_prefix('$')) {
        Some(hex) => u16::from_str_radix(hex, 16),
        None => s.parse::<u16>(),
    }
}

fn flags_string(flags: u8) -> String {
    [(0x80, "Z"), (0x40, "N"), (0x20, "H"), (0x10, "C")].iter().map(
        |(bit, flag)| if (flags & bit) != 0 { flag } else { " " }
    ).collect()
}
