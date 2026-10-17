mod common;

use std::path::Path;

use common::{app_playing_in, clocked, tmp_root_with_carts, write_gb_cart};
use slot::app::{App, Phase};
use slot::session::Session;
use slot_input::{Action, Btn, Millis, RawEvent};
use slot_store::{
    read_carousel, read_controls, read_shortcuts, write_carousel, Binding, CarouselAction,
    CarouselKeys, Controls, Device, GameButton, Modifier, Platform, Shortcut,
};
use slot_ui::QuickRow;
use tempfile::TempDir;

fn press(a: &mut App, b: Btn) {
    a.apply(Action::GbaDown(b));
    a.apply(Action::GbaUp(b));
}

fn in_game_at(a: &mut App, row: QuickRow) {
    for _ in 0..4 {
        if a.in_game_menu().is_none() {
            break;
        }
        press(a, Btn::B);
    }
    a.apply(Action::QuickMenu);
    let mut path = vec![row];
    while let Some(parent) = path.last().and_then(|r| r.parent_in_game()) {
        path.push(parent);
    }
    for (depth, step) in path.iter().rev().enumerate() {
        if depth > 0 {
            press(a, Btn::A);
        }
        let hidden = a.in_game_hidden();
        for _ in 0..step.position(&hidden) {
            press(a, Btn::Down);
        }
    }
    assert_eq!(a.in_game_menu(), Some(row));
}

fn bind(a: &mut App, presses: &[RawEvent]) {
    press(a, Btn::A);
    assert!(a.capturing(), "A did not start waiting for a press");
    for ev in presses {
        a.capture_input(*ev);
    }
    assert!(!a.capturing(), "still waiting after {presses:?}");
}

fn playing(stem: &str) -> (TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let a = app_playing_in(d.path(), stem);
    (d, a)
}

#[test]
fn a_console_button_binds_to_the_next_press_and_moves_on() {
    let (_d, mut a) = playing("Emerald");
    in_game_at(&mut a, QuickRow::GameA);
    bind(&mut a, &[RawEvent::Down(Btn::X)]);
    assert_eq!(a.controls().game(GameButton::A), Binding::plain(Device::X));
    assert_eq!(a.in_game_menu(), Some(QuickRow::GameB));
}

#[test]
fn holding_menu_binds_menu_and_the_button() {
    let (_d, mut a) = playing("Emerald");
    in_game_at(&mut a, QuickRow::GameB);
    bind(&mut a, &[RawEvent::Down(Btn::Menu), RawEvent::Down(Btn::Y)]);
    assert_eq!(
        a.controls().game(GameButton::B),
        Binding::with(Modifier::Menu, Device::Y)
    );
}

#[test]
fn select_is_a_plain_button_on_controls() {
    let (_d, mut a) = playing("Emerald");
    in_game_at(&mut a, QuickRow::GameStart);
    bind(&mut a, &[RawEvent::Down(Btn::Select)]);
    assert_eq!(
        a.controls().game(GameButton::Start),
        Binding::plain(Device::Select)
    );
}

#[test]
fn x_clears_a_row_and_left_and_right_do_nothing() {
    let (_d, mut a) = playing("Emerald");
    in_game_at(&mut a, QuickRow::GameA);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::Left);
    assert_eq!(a.controls(), Controls::default());
    press(&mut a, Btn::X);
    assert_eq!(a.controls().game(GameButton::A), None);
}

#[test]
fn a_change_lasts_until_the_game_closes_unless_it_is_saved() {
    let (d, mut a) = playing("Emerald");
    in_game_at(&mut a, QuickRow::GameA);
    bind(&mut a, &[RawEvent::Down(Btn::X)]);
    assert_eq!(
        read_controls(d.path(), Platform::Gba, "Emerald"),
        Controls::default()
    );
    let b = app_playing_in(d.path(), "Emerald");
    assert_eq!(b.controls(), Controls::default());
}

#[test]
fn save_for_console_reaches_every_game_on_the_console() {
    let (d, mut a) = playing("Emerald");
    in_game_at(&mut a, QuickRow::GameA);
    bind(&mut a, &[RawEvent::Down(Btn::X)]);
    in_game_at(&mut a, QuickRow::SaveConsole);
    press(&mut a, Btn::A);
    let other = app_playing_in(d.path(), "Fusion");
    assert_eq!(
        other.controls().game(GameButton::A),
        Binding::plain(Device::X)
    );
}

#[test]
fn save_for_game_stays_with_that_game_and_restore_takes_it_back() {
    let (d, mut a) = playing("Emerald");
    in_game_at(&mut a, QuickRow::GameA);
    bind(&mut a, &[RawEvent::Down(Btn::X)]);
    in_game_at(&mut a, QuickRow::SaveGame);
    press(&mut a, Btn::A);
    assert_eq!(
        read_controls(d.path(), Platform::Gba, "Emerald").game(GameButton::A),
        Binding::plain(Device::X)
    );
    assert_eq!(
        read_controls(d.path(), Platform::Gba, "Fusion"),
        Controls::default()
    );
    in_game_at(&mut a, QuickRow::SaveRestore);
    press(&mut a, Btn::A);
    assert_eq!(a.controls(), Controls::default());
    assert_eq!(
        read_controls(d.path(), Platform::Gba, "Emerald"),
        Controls::default()
    );
}

#[test]
fn a_game_boy_has_no_l_or_r_rows() {
    let d = tmp_root_with_carts(&["Emerald"]);
    write_gb_cart(&d, "Tetris", "TETRIS");
    let a = app_playing_in(d.path(), "Tetris");
    let hidden = a.in_game_hidden();
    assert!(hidden.contains(&QuickRow::GameL));
    assert!(hidden.contains(&QuickRow::GameR));
}

const FRAME: Millis = 16;

fn step(s: &mut Session, now: &mut Millis, events: &[RawEvent]) {
    *now += FRAME;
    s.feed(events.iter().copied(), *now);
    s.update(FRAME as f32 / 1000.0);
}

fn tap(s: &mut Session, now: &mut Millis, btn: Btn) {
    step(s, now, &[RawEvent::Down(btn)]);
    step(s, now, &[RawEvent::Up(btn)]);
    for _ in 0..25 {
        step(s, now, &[]);
    }
}

fn shelf_card() -> TempDir {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_gb_cart(&d, "Tetris", "TETRIS");
    clocked(d.path());
    d
}

fn on_shelf(root: &Path) -> (Session, Millis) {
    let mut s = Session::boot(root.to_path_buf());
    let mut now = 0;
    for _ in 0..30 {
        if matches!(s.app().phase(), Phase::Shelf) {
            return (s, now);
        }
        step(&mut s, &mut now, &[]);
    }
    panic!("never reached the shelf: {:?}", s.app().phase());
}

#[test]
fn the_carousel_page_rebinds_shelf_switching_and_saves_at_once() {
    let d = shelf_card();
    let (mut s, mut now) = on_shelf(d.path());
    tap(&mut s, &mut now, Btn::Menu);
    for _ in 0..QuickRow::Carousel.position(&[]) {
        tap(&mut s, &mut now, Btn::Down);
    }
    tap(&mut s, &mut now, Btn::A);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::Recents));
    tap(&mut s, &mut now, Btn::Down);
    tap(&mut s, &mut now, Btn::Down);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ShelfPrev));
    tap(&mut s, &mut now, Btn::A);
    assert!(s.app().capturing());
    tap(&mut s, &mut now, Btn::X);
    assert!(!s.app().capturing());
    assert_eq!(
        read_carousel(d.path()).get(CarouselAction::PreviousShelf),
        Binding::plain(Device::X)
    );
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ShelfNext));
    tap(&mut s, &mut now, Btn::B);
    tap(&mut s, &mut now, Btn::B);
    assert!(matches!(s.app().phase(), Phase::Shelf));
    let before = s.app().shelf_platform_name();
    tap(&mut s, &mut now, Btn::L1);
    assert_eq!(
        s.app().shelf_platform_name(),
        before,
        "L1 still switched shelves"
    );
    tap(&mut s, &mut now, Btn::X);
    assert_ne!(
        s.app().shelf_platform_name(),
        before,
        "X did not switch shelves"
    );
}

#[test]
fn a_menu_chord_on_the_carousel_switches_shelves_without_opening_the_menu() {
    let d = shelf_card();
    let mut keys = CarouselKeys::default();
    keys.set(
        CarouselAction::NextShelf,
        Binding::with(Modifier::Menu, Device::R1),
    );
    write_carousel(d.path(), &keys).expect("write carousel");
    let (mut s, mut now) = on_shelf(d.path());
    let before = s.app().shelf_platform_name();
    tap(&mut s, &mut now, Btn::R1);
    assert_eq!(
        s.app().shelf_platform_name(),
        before,
        "plain R1 switched shelves"
    );
    step(&mut s, &mut now, &[RawEvent::Down(Btn::Menu)]);
    tap(&mut s, &mut now, Btn::R1);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::Menu)]);
    for _ in 0..25 {
        step(&mut s, &mut now, &[]);
    }
    assert_ne!(
        s.app().shelf_platform_name(),
        before,
        "MENU+R1 did not switch"
    );
    assert_eq!(s.app().quick_menu(), None, "the chord opened the menu");
}

#[test]
fn shortcuts_are_set_from_the_main_menu_and_saved_for_every_game_at_once() {
    let d = shelf_card();
    let (mut s, mut now) = on_shelf(d.path());
    tap(&mut s, &mut now, Btn::Menu);
    for _ in 0..QuickRow::Shortcuts.position(&[]) {
        tap(&mut s, &mut now, Btn::Down);
    }
    tap(&mut s, &mut now, Btn::A);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ScSave));
    tap(&mut s, &mut now, Btn::A);
    assert!(s.app().capturing());
    step(&mut s, &mut now, &[RawEvent::Down(Btn::Select)]);
    step(&mut s, &mut now, &[RawEvent::Down(Btn::X)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::X)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::Select)]);
    assert!(!s.app().capturing());
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ScLoad));
    tap(&mut s, &mut now, Btn::A);
    step(&mut s, &mut now, &[RawEvent::Down(Btn::Select)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::Select)]);
    assert!(!s.app().capturing());
    let saved = read_shortcuts(d.path());
    assert_eq!(
        saved.get(Shortcut::SaveState),
        Binding::with(Modifier::Select, Device::X)
    );
    assert_eq!(
        saved.get(Shortcut::LoadState),
        Binding::plain(Device::Select)
    );
    for stem in ["Emerald", "Fusion"] {
        assert_eq!(app_playing_in(d.path(), stem).shortcuts(), saved, "{stem}");
    }
}

#[test]
fn the_in_game_settings_have_no_shortcuts_page() {
    assert!(!QuickRow::SETTINGS.contains(&QuickRow::Shortcuts));
    assert!(QuickRow::MAIN.contains(&QuickRow::Shortcuts));
}
