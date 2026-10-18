use std::{sync::{Arc, atomic::{AtomicBool, Ordering::Relaxed}}, time::{Duration, Instant}};

use console::lcd::Consumer;
use sdl2::{EventPump, event::Event, pixels::PixelFormatEnum, render::{Canvas, Texture}, video::Window};

const WIDTH: u32 = 160;
const HEIGHT: u32 = 144;
const DEFAULT_SCALE: u32 = 4;

const PALETTE: [[u8; 3]; 4] = [
    [0xE0, 0xF8, 0xD0],
    [0x88, 0xC0, 0x70],
    [0x34, 0x68, 0x56],
    [0x08, 0x18, 0x20],
  ];

pub struct Render {
    target_fps: u16,
    consumer: Consumer,
    quit_flag: Arc<AtomicBool>,
    canvas: Canvas<Window>,
    events: EventPump,
    frame_buffer: [u8; (WIDTH as usize) * (HEIGHT as usize) * 3],
}

impl Render {
    pub fn new(target_fps: u16, consumer: Consumer, quit_flag: Arc<AtomicBool>) -> Self {
        let sdl = sdl2::init().unwrap();
        let window = sdl.video().unwrap()
            .window("Shiro's Gameboy", WIDTH * DEFAULT_SCALE, HEIGHT * DEFAULT_SCALE)
            .position_centered()
            .build()
            .unwrap();

        let mut canvas = window.into_canvas().build().unwrap();
        canvas.set_logical_size(WIDTH, HEIGHT).unwrap();
        sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "0");  // nearest neighbor

        Render {
            target_fps,
            consumer,
            quit_flag,
            canvas,
            events: sdl.event_pump().unwrap(),
            frame_buffer: [0u8; (WIDTH as usize) * (HEIGHT as usize) * 3],
        }
    }

    pub fn run(mut self) {
        let tc = self.canvas.texture_creator();
        let mut texture = tc.create_texture_streaming(PixelFormatEnum::RGB24, WIDTH, HEIGHT).unwrap();
        let target_frame_time = Duration::from_secs_f64(1.0 / self.target_fps as f64);
        let mut next_frame = Instant::now() + target_frame_time;

        while !self.quit_flag.load(Relaxed) {
            if !self.handle_events() {
                break;
            }

            self.update_frame_buffer();

            self.present_texture(&mut texture);

            let now = Instant::now();
            if now < next_frame {
                std::thread::sleep(next_frame - now);
                next_frame += target_frame_time;
            } else {
                next_frame = now + target_frame_time;
            }
        }
    }

    fn handle_events(&mut self) -> bool {
        for e in self.events.poll_iter() {
            match e {
                Event::Quit { .. } => return false,
                _ => { },
            }
        }
        true
    }

    fn update_frame_buffer(&mut self) {
        let (data, mut start) = self.consumer.read_new();
        start %= (WIDTH * HEIGHT) as usize;
        for (i, pixel) in data.iter().enumerate() {
            let pixel_start = 3 * start + 3 * i;
            let rgb = PALETTE[(pixel & 3) as usize];
            self.frame_buffer[pixel_start + 0] = rgb[0];
            self.frame_buffer[pixel_start + 1] = rgb[1];
            self.frame_buffer[pixel_start + 2] = rgb[2];
        }
    }

    fn present_texture(&mut self, texture: &mut Texture) {
        texture.update(None, &self.frame_buffer, (WIDTH as usize) * 3).unwrap();
        self.canvas.copy(&texture, None, None).unwrap();
        self.canvas.present();
    }
}
