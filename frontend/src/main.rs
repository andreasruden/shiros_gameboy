use std::env;
use console::{Cartridge, GameBoy};

fn main() {
    let cartridge_path = env::args().nth(1).expect("usage: shiros_gameboy <cartridge path>");
    let cartridge = Cartridge::from_file(cartridge_path).unwrap();
    println!("Loaded: {}", cartridge.temp_string());

    let mut gameboy = GameBoy::new();
    gameboy.insert_cartridge(cartridge);
    gameboy.power_on();
    gameboy.run_frame();
}
