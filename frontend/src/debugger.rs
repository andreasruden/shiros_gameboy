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
                StopReason::Breakpoint(addr) => self.on_pause(Some(addr), None),
                StopReason::Watchpoint(addr) => self.on_pause(None, Some(addr)),
                StopReason::Step => self.on_pause(None, None),
                StopReason::StopRequested => {
                    println!();
                    self.on_pause(None, None);
                }
                StopReason::Halt => break,
                _ => panic!("Unhandled StopReason"),
            }
        }
    }

    fn on_pause(&mut self, breakpoint: Option<u16>, watchpoint: Option<u16>) {
        let pc = self.gameboy.cpu.pc();
        if let Some(addr) = breakpoint {
            println!("Break at {:04X}", addr);
        }
        if let Some(addr) = watchpoint {
            println!("Watchpoint for address {:04X} triggered", addr);
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
                (Some("w" | "watch"), arg1, arg2) => match parse_addr_range(arg1, arg2) {
                    Ok((start_addr, end_addr)) => {
                        match self.gameboy.debug_set_watchpoint(start_addr, end_addr) {
                            Ok(_) => { println!("Watching ${start_addr:04X}..=${end_addr:04X}") },
                            Err(e) => println!("Error: {e}"),
                        };
                    }
                    Err(e) => println!("Error: {e}"),
                },
                (Some("uw" | "unwatch"), arg, _) => match parse_addr(arg) {
                    Ok(addr) => {
                        let removed = self.gameboy.debug_delete_watchpoint(addr);
                        println!("removed {removed} watchpoint range(s)");
                    }
                    Err(e) => println!("Error: {e}"),
                },
                (Some("r" | "registers"), _, _) => self.print_cpu_registers(),
                (Some("m" | "memory"), start, end) => match parse_addr_range(start, end) {
                    Ok((start_addr, end_addr)) => self.hexdump_memory(start_addr, end_addr),
                    Err(e) => println!("Error: {e}"),
                },
                (Some("wr" | "write"), arg1, arg2) => match (arg1, arg2) {
                    (Some(arg1), Some(arg2)) => self.write_cmd(arg1, arg2),
                    _ => println!("Error: command is \"write location value\""),
                },
                (Some("h" | "help"), _, _) => print_help(),
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

    fn hexdump_memory(&mut self, start: u16, end: u16) {
        assert!(start <= end);

        if start == end {
            let byte = self.read_memory(start);
            if (0x20..=0x7E).contains(&byte) {
                println!("{:02X} '{}'", byte, byte as char);
            } else {
                println!("{:02X}", byte);
            }
            return;
        }

        let start = start as u32;
        let end = end as u32;
        let mut line_addr = start & !0xF;  // truncate to nearest 16-byte boundary

        while line_addr <= end {
            let mut hex = String::with_capacity(50);
            let mut ascii = String::with_capacity(16);

            for i in 0..16 {
                let addr = line_addr + i;
                if addr < start || addr > end {
                    hex.push_str("   ");
                    ascii.push(' ');
                } else {
                    let byte = self.read_memory(addr as u16);
                    hex.push_str(&format!("{:02X} ", byte));
                    let ascii_char = if (0x20..=0x7E).contains(&byte) { byte as char } else { '.' };
                    ascii.push(ascii_char);
                }

                if i == 7 {
                    hex.push(' ');  // extra space at 8 byte boundary
                }
            }

            println!("{:04X}:  {} |{}|", line_addr, hex, ascii);
            line_addr += 16;
        }
    }

    fn write_memory(&mut self, addr: u16, byte: u8) {
        match addr {
            0x0000..=0x3FFF => self.gameboy.cartridge.as_mut().unwrap().rom[addr as usize] = byte,
            0xFF00..=0xFF7F => self.gameboy.hw_reg.set(addr, byte),
            _ => todo!(),
        };
        println!("[${addr:04X}] = {}", self.read_memory(addr));
    }

    fn read_memory(&mut self, addr: u16) -> u8 {
        let byte: u8;
        match addr {
            0x0000..=0x3FFF => byte = self.gameboy.cartridge.as_ref().unwrap().rom[addr as usize],
            0x8000..=0x9FFF => byte = self.gameboy.vram[(addr - 0x8000) as usize],
            0xFF00..=0xFF7F => byte = self.gameboy.hw_reg.get(addr),
            _ => todo!(),
        };
        byte
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

fn parse_addr_range(start: Option<&str>, end: Option<&str>) -> Result<(u16, u16), String> {
    match end {
        None => {
            match parse_addr(start) {
                Err(e) => Err(e),
                Ok(addr) => Ok((addr, addr)),
            }
        }
        Some(_) => {
            let start_addr = parse_addr(start);
            let end_addr = parse_addr(end);
            if start_addr.is_err() || end_addr.is_err() {
                return Err(String::from("could not parse addresses in range"));
            }

            let start_addr_val = start_addr.unwrap();
            let end_addr_val = end_addr.unwrap();
            if start_addr_val > end_addr_val {
                return Err(String::from("start is larger than end"));
            }

            Ok((start_addr_val, end_addr_val))
        }
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

fn print_help() {
    println!("Available commands:");
    println!("  c  | continue     run until next breakpoint or ctrl-c");
    println!("  s  | step         run one instruction");
    println!("  b  | break        break on address (arg1)");
    println!("  d  | delete       delete breakpoint on address (arg1)");
    println!("  w  | watch        break on writes between start (arg1) and end inclusively (arg2, optional)");
    println!("  uw | unwatch      remove watch breakpoint range overlapping address (arg1)");
    println!("  r  | registers    print the value of CPU registers");
    println!("  m  | memory       dump the memory between start (arg1) and end inclusively (arg2, optional)");
    println!("  wr | write        dump into address (arg1) the value specified (arg2)");
}
