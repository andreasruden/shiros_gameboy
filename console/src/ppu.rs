use crate::bus::{Bus, PpuBus};

const LCD_BG_AND_WINDOW_ON_BIT: u8 = 1 << 0;
const LCD_BG_TILE_MAP_BIT: u8 = 1 << 3;
const LCD_BG_TILE_DATA_BIT: u8 = 1 << 4;
const LCD_CTRL_ENABLED_BIT: u8 = 1 << 7;
const IF_VBLANK_BIT: u8 = 1 << 0;

const SCREEN_WIDTH: u8 = 160;

pub struct PPU {
    mode: u8,
    dots: u16,
    x: u8,  // draw position in current scanline
}

impl PPU {
    pub fn new() -> Self {
        PPU {
            mode: 2,
            dots: 0,
            x: 0,
        }
    }

    // The time unit of the PPU is often referred to as a "dot", that corresponds to 1 T-cycle
    // when running at 4.19 MHz mode.
    pub fn step(&mut self, mut t_cycles: u8, bus: &mut PpuBus) {
        let lcd_ctrl = bus.hw_reg.lcd_ctrl;
        if lcd_ctrl & LCD_CTRL_ENABLED_BIT == 0 {
            self.write_ppu_mode(bus);
            return;
        }

        while t_cycles > 0 {
            t_cycles = match self.mode {
                0 => self.step_mode0(t_cycles, bus),
                1 => self.step_mode1(t_cycles, bus),
                2 => self.step_mode2(t_cycles, bus),
                3 => self.step_mode3(t_cycles, bus),
                _ => unreachable!(),
            };
        }

        self.write_ppu_mode(bus);
    }

    // Object Attribute Memory (OAM) scan: up to the 10 first (of the 40 total) sprite entries in OAM will be picked
    // if they are on this scaline
    // time spent: 80 dots
    fn step_mode2(&mut self, mut t_cycles: u8, bus: &mut PpuBus) -> u8 {
        t_cycles = self.idle(t_cycles, 80);
        if self.dots == 80 {
            self.mode = 3;
            self.x = 0;
        }
        t_cycles
    }

    // Drawing Pixels: draws the scanline
    // time spent: 172-289 dots
    fn step_mode3(&mut self, mut t_cycles: u8, bus: &mut PpuBus) -> u8 {
        let bg_on = bus.hw_reg.lcd_ctrl & LCD_BG_AND_WINDOW_ON_BIT != 0;

        while t_cycles > 0 {
            if self.x == SCREEN_WIDTH {
                break;
            }

            // BG and Window drawing
            let pixel = if bg_on { self.bg_shade(bus) } else { 0 };
            bus.lcd.push_pixel(pixel);

            self.x += 1;
            t_cycles -= 1;
            self.dots += 1;
        }

        if self.x == SCREEN_WIDTH {
            self.x = 0;
            self.mode = 0;
        }

        t_cycles
    }

    // Horizontal Blank: extra times after each scanling where VRAM and OAM become accessible to the CPU
    // time spent: 456-mode2-mode3 = 87-204 dots
    fn step_mode0(&mut self, mut t_cycles: u8, bus: &mut PpuBus) -> u8 {
        t_cycles = self.idle(t_cycles, 456);
        if self.dots == 456 {
            self.dots = 0;
            bus.hw_reg.lcd_y += 1;
            if bus.hw_reg.lcd_y == 144 {
                self.mode = 1;
                self.request_vblank_interrupt(bus);
            } else {
                self.mode = 2;
            }
        }

        t_cycles
    }

    // Vertical Blank: extra time corresponding to 10 scanlines before drawing restarts, letting the CPU
    // do longer bulk operations like copying & rewriting tile maps, or running OAM DMA to update sprites
    // time spent: 10 scanlines = 4560 dots
    fn step_mode1(&mut self, mut t_cycles: u8, bus: &mut PpuBus) -> u8 {
        t_cycles = self.idle(t_cycles, 456);
        if self.dots == 456 {
            self.dots = 0;
            bus.hw_reg.lcd_y += 1;
            if bus.hw_reg.lcd_y == 154 {
                bus.hw_reg.lcd_y = 0;
                self.mode = 2;
            }
        }
        t_cycles
    }

    fn idle(&mut self, t_cycles: u8, max_dots: u16) -> u8 {
        let consumed = (t_cycles as u16).min(max_dots.saturating_sub(self.dots));
        self.dots += consumed;
        t_cycles - consumed as u8
    }

    fn write_ppu_mode(&mut self, bus: &mut PpuBus) {
        let mut mode = self.mode;
        if bus.hw_reg.lcd_ctrl & LCD_CTRL_ENABLED_BIT == 0 {
            mode = 0;
        }

        bus.hw_reg.lcd_stat &= !0b11;
        bus.hw_reg.lcd_stat |= mode;
    }

    fn request_vblank_interrupt(&self, bus: &mut PpuBus) {
        bus.hw_reg.i_f |= IF_VBLANK_BIT;
    }

    fn bg_shade(&self, bus: &mut PpuBus) -> u8 {
        // There are two BG tile maps. Each such map is 32x32 bytes. A byte is an index of the tile to be displayed.
        // The actual tiles sit in the Tile Data and are 8x8 pixels. In other words, a tile map contains 256x256 pixels.
        let tile_map_addr: u16 = if bus.hw_reg.lcd_ctrl & LCD_BG_TILE_MAP_BIT == 0 { 0x9800 } else { 0x9C00 };

        let y = bus.hw_reg.lcd_y.wrapping_add(bus.hw_reg.lcd_scy) as u16;
        let x = self.x.wrapping_add(bus.hw_reg.lcd_scx) as u16;

        let tile_index = bus.read(tile_map_addr + 32 * (y / 8) + x / 8);

        // There are 3 blocks of 2048 bytes (127 tiles) each, at $8000, $8800 and $9000.
        // If the tile data bit is set we use block $8000 and $8800 and index from $8000 unsigned.
        // If the bit is not set we use signed indexing relative to $9000.
        let tile_data_addr = if bus.hw_reg.lcd_ctrl & LCD_BG_TILE_DATA_BIT != 0 {
            0x8000 + tile_index as u16 * 16
        } else {
            0x9000u16.wrapping_add_signed(tile_index as i8 as i16 * 16)
        };

        // A tile is 8x8 and 16 bytes in size. It's laid out row by row such that the first 2 bytes are the first row
        // of pixels, and so on. Each pixel is given 2 bits for color making for 4 total colors (00, 01, 10, 11).
        // However, there is a caveat: each pixel draws one bit from each byte (starting with the most significant bit),
        // with the second byte going to the pixel's most significant bit.
        let row_addr = tile_data_addr + 2 * (y % 8);
        let lo = bus.read(row_addr);
        let hi = bus.read(row_addr + 1);
        let pixel_bit = 7 - x % 8;
        let color_msb = (hi >> pixel_bit) & 1;
        let color_lsb = (lo >> pixel_bit) & 1;
        let pixel_color = color_msb << 1 | color_lsb;

        // The BGP register contains a mapping from the tile's pixel color to the currently used shade. This lets
        // the background be recolored without touching the tile data, by editing just the BG palette register.
        // Can be used for e.g. fade to black.
        bus.hw_reg.bgp >> (pixel_color * 2) & 0b11
    }
}
