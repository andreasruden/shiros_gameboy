use console::{Cartridge, GameBoy};

struct Args {
    cartridge_path: String,
    debugging_enabled: bool,
}

fn main() -> Result<(), lexopt::Error> {
    let args = parse_args()?;

    let cartridge = Cartridge::from_file(args.cartridge_path).unwrap();
    println!("Loaded: {}", cartridge.temp_string());

    let mut gameboy = GameBoy::new();
    gameboy.insert_cartridge(cartridge);
    gameboy.power_on();
    gameboy.run_frame();

    Ok(())
}

fn print_usage() {
    println!("usage: sgb [options] <cartridge path>");
    println!("\noptions:");
    println!("-h, --help     show this help message and exit");
    println!("-d, --debug    utilize the built in debugger to debug the running game");
}

fn parse_args() -> Result<Args, lexopt::Error> {
    use lexopt::prelude::*;

    let mut cartridge_path = None;
    let mut debugging_enabled = false;
    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Short('d') | Long("debug") => {
                debugging_enabled = true;
            },
            Value(val) if cartridge_path.is_none() => {
                cartridge_path = Some(val.string()?);
            },
            Short('h') | Long("help") => {
                print_usage();
                std::process::exit(0);
            }
            _ => return Err(arg.unexpected()),
        }
    }

    Ok(Args {
        cartridge_path: cartridge_path.ok_or("missing cartridge path")?,
        debugging_enabled
    })
}
