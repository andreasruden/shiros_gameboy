// The representation of a game cartridge for the gameboy

use std::{fmt, fs, io::self, path::Path};

// const ROM_HEADER_START: u16 = 0x0100;
const ROM_HEADER_END: u16 = 0x014F;

pub struct Cartridge {
    rom: Vec<u8>, // raw dump of cartridge's read-only memory

    // The first 256 bytes will only be visible after the boot loader has run, leaving
    // instruction pointer at address $0100, the entry point of the cartridge
    title: Vec<u8>,      // $0134-$0143 (or to $0142/$013E on later cartridges)
    cartridge_type: u8,  // $0147
    rom_size_type: u8,   // $0148
    ram_size_type: u8,   // $0149

    // Cartridge header ends at $014F
}

#[derive(Debug)]
pub enum CartridgeError {
    Io(io::Error),
    TooShort { len: usize },
}

impl Cartridge {
    pub fn from_bytes(rom: Vec<u8>) -> Result<Self, CartridgeError> {
        if rom.len() + 1 < ROM_HEADER_END.into() {
            return Err(CartridgeError::TooShort { len: rom.len() });
        }

        Ok(Cartridge {
            title: Vec::from(&rom[0x134..0x143]),
            cartridge_type: rom[0x147],
            rom_size_type: rom[0x148],
            ram_size_type: rom[0x149],
            rom,  // must be last since it's moved
        })
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, CartridgeError> {
        let data = fs::read(path)?;
        Self::from_bytes(data)
    }

    pub fn title(self: &Self) -> String {
        String::from_utf8(self.title.clone()).unwrap()  // panic if title cannot be converted
    }

    // This function is just temporary to get some basic info out, should be made more propper
    pub fn temp_string(self: &Self) -> String {
        let title = self.title();
        let ct = self.cartridge_type;
        let romt = self.rom_size_type;
        let ramt = self.ram_size_type;
        format!("Title: {title}, cartridge type: {ct}, rom size type: {romt}, ram size type: {ramt}")
    }
}

impl fmt::Display for CartridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "failed read: {e}"),
            Self::TooShort { len } => write!(f, "Rom size {len} is too short"),
        }
    }
}

impl std::error::Error for CartridgeError {}

impl From<io::Error> for CartridgeError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
