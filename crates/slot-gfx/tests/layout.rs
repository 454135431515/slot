#![cfg(target_os = "macos")]

use slot_gfx::{Compositor, Draw, HeadlessSurface, ScreenEffect, SRC_H, SRC_W};
use std::sync::{Mutex, MutexGuard, PoisonError};

static GL: Mutex<()> = Mutex::new(());

fn compositor() -> Option<(MutexGuard<'static, ()>, HeadlessSurface, Compositor)> {
    let guard = GL.lock().unwrap_or_else(PoisonError::into_inner);
    let surface = HeadlessSurface::new().ok()?;
    let compositor = Compositor::new(&surface).ok()?;
    Some((guard, surface, compositor))
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

fn pattern() -> Vec<u8> {
    (0..SRC_W * SRC_H)
        .flat_map(|i| {
            let (x, y) = (i % SRC_W, i / SRC_W);
            [(x * 7) as u8, (y * 11) as u8, ((x + y) * 3) as u8, 255]
        })
        .collect()
}

fn framed_frame(c: &mut Compositor) -> Vec<u8> {
    c.fit((640, 480));
    c.set_screen_effect(ScreenEffect::Lcd3x);
    c.set_screen_power(1.0);
    let badge = c.create_texture(2, 2, &[200, 40, 40, 255].repeat(4));
    c.begin_frame();
    c.upload_game(&pattern());
    c.draw_list(&[
        Draw::Game,
        Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: 720.0,
            h: 40.0,
            colour: [0.0, 0.0, 0.0, 0.5],
        },
        Draw::Tex {
            x: 690.0,
            y: 10.0,
            w: 20.0,
            h: 20.0,
            tex: badge,
            alpha: 1.0,
        },
    ]);
    c.read_frame()
}

const FRAMED_HASH: u64 = 0xe3b9_652d_e7d2_eb8c;

#[test]
fn a_framed_frame_on_a_4_by_3_panel_is_todays_frame() {
    let Some((_g, _s, mut c)) = compositor() else {
        return;
    };
    let frame = framed_frame(&mut c);
    assert_eq!(frame.len(), 640 * 427 * 4);
    assert_eq!(fnv(&frame), FRAMED_HASH, "hash is {:#x}", fnv(&frame));
}
