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

fn idle_secs(w: &mut World, st: &State, secs: f64) {
    let idle = Input::default();
    for _ in 0..(secs * 60.0) as usize {
        w.update(&idle, 1.0 / 60.0, st, false);
    }
}

#[test]
fn kollegorna_gar_omkring() {
    let mut st = State::new(build_golden(2));
    let mut f = Fonts::load();
    let mut w = World::new("gbg", 2);
    let names = ["Anna", "Karim", "Sara", "Lisa", "Bo", "Maja", "Omar", "Linnea"];
    let start: Vec<_> = names.iter().map(|n| w.npc_pos(n).unwrap()).collect();
    // Spelaren står långt bort så att ingen fryser
    w.walk_to_point(-12.0, 9.0);
    idle_secs(&mut w, &st, 12.0);
    let mut moved = 0;
    let mut maxd: f64 = 0.0;
    for k in 0..90 {
        idle_secs(&mut w, &st, 1.0);
        for (i, n) in names.iter().enumerate() {
            let p = w.npc_pos(n).unwrap();
            let d = (p.0 - start[i].0).hypot(p.1 - start[i].1);
            maxd = maxd.max(d);
        }
        if k == 30 { shot(&mut w, &mut st, &mut f, "kollegor_a"); }
    }
    for (i, n) in names.iter().enumerate() {
        let p = w.npc_pos(n).unwrap();
        println!("{n}: {:?} -> {:?}, mål kvar {}", start[i], p, w.npc_stops(n));
        if (p.0 - start[i].0).hypot(p.1 - start[i].1) > 0.5 { moved += 1; }
        assert!(w.npc_stops(n) >= 1, "{n} har inga nåbara mål");
    }
    println!("flyttat sig {moved}, längsta sträcka {maxd:.1}");
    assert!(maxd > 2.0, "ingen har gått någonstans");
    shot(&mut w, &mut st, &mut f, "kollegor_b");
}

#[test]
fn borasochmolndal_kollegor() {
    let st = State::new(build_golden(2));
    let mut w = World::new("boras", 12);
    idle_secs(&mut w, &st, 100.0);
    println!("Nils {:?} stops {}", w.npc_pos("Nils"), w.npc_stops("Nils"));
    w.set_site("molndal");
    idle_secs(&mut w, &st, 100.0);
    println!("Elin {:?} stops {}", w.npc_pos("Elin"), w.npc_stops("Elin"));
}

#[test]
fn markerad_kollega_gar_hem_och_hittas() {
    let st = State::new(build_golden(2));
    let mut w = World::new("gbg", 2);
    w.walk_to_point(-12.0, 9.0);
    idle_secs(&mut w, &st, 70.0);
    w.set_mark("Karim", Mark::Ticket);
    idle_secs(&mut w, &st, 40.0);
    let home = (0.35, 4.62);
    let p = w.npc_pos("Karim").unwrap();
    assert!((p.0 - home.0).hypot(p.1 - home.1) < 0.7, "Karim borde sitta vid sitt skrivbord: {:?}", p);
}

#[test]
fn krabban_springer_och_fangas() {
    let mut st = State::new(build_golden(2));
    let mut f = Fonts::load();
    let mut w = World::new("gbg", 2);
    w.set_crab(Some("SW1".into()));
    let idle = Input::default();
    for _ in 0..30 { w.update(&idle, 1.0 / 60.0, &st, false); }
    let cp = w.crab_pos().expect("krabban finns");
    println!("krabba {:?}", cp);
    w.walk_to_point(cp.0 + 1.2, cp.1 - 1.0);
    for _ in 0..240 { w.update(&idle, 1.0 / 60.0, &st, false); }
    shot(&mut w, &mut st, &mut f, "krabba");
    // Jaga den
    let mut got = false;
    for i in 0..3000 {
        let Some(cp) = w.crab_pos() else { break };
        if i % 30 == 0 { w.walk_to_point(cp.0, cp.1); }
        for ev in w.update(&idle, 1.0 / 60.0, &st, false) {
            if ev == WorldEvent::Use("krabba".into()) { got = true; }
        }
        if got { break; }
    }
    assert!(got, "krabban fångades inte");
    assert!(w.crab_near.is_none());
    assert!(w.crab_pos().is_none());
}

#[test]
fn hastighet_och_sträcka() {
    let st = State::new(build_golden(2));
    let mut w1 = World::new("gbg", 2);
    let mut w2 = World::new("gbg", 2);
    w2.set_speed_mult(1.5);
    let idle = Input::default();
    for _ in 0..20 { w1.update(&idle, 1.0 / 60.0, &st, false); w2.update(&idle, 1.0 / 60.0, &st, false); }
    w1.take_walked(); w2.take_walked();
    let (a0, b0) = (w1.player_pos(), w2.player_pos());
    walk(&mut w1, &st, Key::D, 1.0);
    walk(&mut w2, &st, Key::D, 1.0);
    let (d1, d2) = (w1.player_pos().0 - a0.0, w2.player_pos().0 - b0.0);
    assert!(d2 > d1 * 1.3, "{d1} {d2}");
    let wk = w1.take_walked();
    assert!((wk - d1).abs() < 0.3, "walked {wk} vs {d1}");
    assert_eq!(w1.take_walked(), 0.0);
}

#[test]
fn vader_och_kvall() {
    let mut st = State::new(build_golden(2));
    let mut f = Fonts::load();
    let mut w = World::new("gbg", 2);
    let idle = Input::default();
    for _ in 0..30 { w.update(&idle, 1.0 / 60.0, &st, false); }
    w.set_weather("rain");
    idle_secs(&mut w, &st, 12.0);
    assert_eq!(w.weather(), "regn");
    shot(&mut w, &mut st, &mut f, "regn");
    w.set_weather("clouds");
    idle_secs(&mut w, &st, 15.0);
    shot(&mut w, &mut st, &mut f, "moln");
    w.set_weather("sun");
    idle_secs(&mut w, &st, 15.0);
    st.time = 9.0 * 3600.0;
    shot(&mut w, &mut st, &mut f, "kvall");
    st.time = 12.0 * 3600.0;
    shot(&mut w, &mut st, &mut f, "natt");
    w.walk_to_point(-12.0, -7.0);
    idle_secs(&mut w, &st, 8.0);
    shot(&mut w, &mut st, &mut f, "natt_server");
    w.set_site("boras");
    idle_secs(&mut w, &st, 2.0);
    shot(&mut w, &mut st, &mut f, "natt_boras");
    w.set_site("gbg");
    st.time = 5.0 * 3600.0;
    w.set_weather("rain");
    w.walk_to_point(9.0, -14.0);
    idle_secs(&mut w, &st, 12.0);
    shot(&mut w, &mut st, &mut f, "regn_ute");
    st.time = 10.0 * 3600.0;
    shot(&mut w, &mut st, &mut f, "regn_kvall_ute");
}

#[test]
fn djur_ger_event() {
    let st = State::new(build_golden(2));
    let mut w = World::new("gbg", 2);
    let idle = Input::default();
    idle_secs(&mut w, &st, 3.0);
    // Gå till ankdammen och använd
    w.walk_to_point(-15.0, -1.5);
    idle_secs(&mut w, &st, 12.0);
    let mut i = Input::default();
    i.pressed.push(Key::E);
    let ev = w.update(&i, 1.0 / 60.0, &st, false);
    println!("{:?} hint {:?} pos {:?}", ev, w.hint(), w.player_pos());
}
