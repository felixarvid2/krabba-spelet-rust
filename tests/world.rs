use krabba_spelet::engine::{save_png, Canvas, Fonts, Input, Key};
use krabba_spelet::game::world::{Mark, World, WorldEvent};
use krabba_spelet::net::model::build_golden;
use krabba_spelet::net::sim::State;

fn shot(w: &mut World, st: &mut State, f: &mut Fonts, name: &str) {
    let mut c = Canvas::new(1280, 720);
    w.sync(st);
    w.draw(&mut c, f, st);
    save_png(&c, &format!("/tmp/varld_{name}.png")).unwrap();
}

fn walk(w: &mut World, st: &State, k: Key, secs: f64) {
    let mut i = Input::default();
    i.down.insert(k);
    for _ in 0..(secs * 60.0) as usize {
        w.update(&i, 1.0 / 60.0, st, false);
    }
}

#[test]
fn rita_varlden() {
    let mut st = State::new(build_golden(2));
    let mut f = Fonts::load();
    let mut w = World::new("gbg", 2);
    w.set_mark("Anna", Mark::Ticket);
    w.set_mark("Bo", Mark::Done);
    let idle = Input::default();
    for _ in 0..60 { w.update(&idle, 1.0 / 60.0, &st, false); }
    shot(&mut w, &mut st, &mut f, "gbg_start");
    walk(&mut w, &st, Key::W, 1.5);
    shot(&mut w, &mut st, &mut f, "gbg_n");
    walk(&mut w, &st, Key::A, 3.0);
    shot(&mut w, &mut st, &mut f, "gbg_w");
    w.zoom_out();
    shot(&mut w, &mut st, &mut f, "gbg_zoom2");
    w.set_site("boras");
    for _ in 0..60 { w.update(&idle, 1.0 / 60.0, &st, false); }
    shot(&mut w, &mut st, &mut f, "boras");
    w.set_site("molndal");
    for _ in 0..60 { w.update(&idle, 1.0 / 60.0, &st, false); }
    shot(&mut w, &mut st, &mut f, "molndal");
}

#[test]
fn klick_gar_och_anvander() {
    let mut st = State::new(build_golden(2));
    let mut f = Fonts::load();
    let mut w = World::new("gbg", 2);
    let idle = Input::default();
    for _ in 0..10 { w.update(&idle, 1.0 / 60.0, &st, false); }
    let mut c = Canvas::new(1280, 720);
    w.draw(&mut c, &mut f, &st);
    let p0 = w.player_pos();
    let mut i = Input::default();
    i.clicked = true; i.mouse_x = 640 - 200; i.mouse_y = 360;
    w.update(&i, 1.0 / 60.0, &st, false);
    for _ in 0..120 { w.update(&idle, 1.0 / 60.0, &st, false); }
    assert!(w.player_pos().0 < p0.0 - 1.0, "borde ha gått åt vänster: {:?} -> {:?}", p0, w.player_pos());
    let _ = WorldEvent::Laptop;
}

#[test]
fn rita_serverrummet() {
    let mut st = State::new(build_golden(2));
    let mut f = Fonts::load();
    let mut w = World::new("gbg", 2);
    let idle = Input::default();
    w.walk_to_point(-11.5, -7.8);
    for _ in 0..900 { w.update(&idle, 1.0 / 60.0, &st, false); }
    println!("pos {:?} hint {:?}", w.player_pos(), w.hint());
    shot(&mut w, &mut st, &mut f, "rack");
    w.zoom_in();
    for _ in 0..120 { w.update(&idle, 1.0 / 60.0, &st, false); }
    shot(&mut w, &mut st, &mut f, "rack4");
}
