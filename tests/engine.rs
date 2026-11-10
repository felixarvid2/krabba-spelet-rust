use krabba_spelet::engine::*;

#[test]
fn rita_former_och_text() {
    let mut c = Canvas::new(WIDTH, HEIGHT);
    let mut f = Fonts::load();
    c.clear(rgb(24, 28, 48));
    c.fill_rect(20, 20, 200, 100, rgb(200, 60, 60));
    c.fill_rect_a(120, 70, 200, 100, rgb(60, 120, 220), 128);
    c.rect(20, 20, 300, 150, 3, rgb(255, 255, 255));
    c.round_rect(340, 20, 200, 100, 16, rgb(80, 200, 120), 255);
    c.circle(620, 70, 40, rgb(250, 220, 60), 255);
    c.ellipse(740, 70, 60, 30, rgb(200, 100, 250), 200);
    c.line(20, 200, 300, 260, rgb(255, 255, 255));
    c.thick_line(20, 220, 300, 280, 5, rgb(255, 160, 0));
    let spr = Sprite::from_map(
        &["..##..", ".#oo#.", "#oooo#", ".#oo#.", "..##.."],
        &[('#', 0xFF202020), ('o', 0xFFE04040)],
    );
    c.blit(&spr, 400, 200, 6, false);
    c.blit(&spr, 480, 200, 6, true);
    c.blit_a(&spr, 560, 200, 6, false, 100);

    c.set_clip(900, 20, 100, 60);
    c.fill_rect(880, 0, 300, 300, rgb(100, 100, 100));
    c.reset_clip();
    c.dim(0, 0);

    let w = rgb(240, 240, 240);
    let mut y = 320;
    let mut n = f.text(&mut c, 20, y, "VT323: Blåbärssaft på ÅÄÖ åäö é – kräftor!", 32.0, w, FontKind::Mono);
    assert!(n > 100);
    y += 40;
    f.text(&mut c, 20, y, "Press Start: ÅÄÖ åäö é Router#", 16.0, w, FontKind::Pixel);
    y += 40;
    f.text(&mut c, 20, y, "Symboler: ✓ • → ← ↑ ↓ ⏎ – 🦀 ✗ ▶ ▲ \u{4e2d}", 32.0, w, FontKind::Mono);
    y += 40;
    f.text(&mut c, 20, y, "Pixel: ✓ • → ← ↑ ↓ ⏎ 🦀 ÅÄÖ", 16.0, w, FontKind::Pixel);
    y += 40;
    f.text(&mut c, 20, y, "Stor rubrik KRABBA", 32.0, rgb(255, 200, 0), FontKind::Pixel);
    y += 50;
    n = f.text(&mut c, 20, y, "Två\nrader", 24.0, w, FontKind::Mono);
    assert!(n > 0);
    let lines = f.wrap("Det här är en ganska lång text som ska brytas vid ord.\nNy rad", 24.0, FontKind::Mono, 300);
    assert!(lines.len() >= 3);
    for (i, l) in lines.iter().enumerate() {
        f.text(&mut c, 700, 320 + i as i32 * 24, l, 24.0, w, FontKind::Mono);
        assert!(f.measure(l, 24.0, FontKind::Mono).0 <= 300);
    }
    let (mw, mh) = f.measure("abc", 32.0, FontKind::Mono);
    assert!(mw > 0 && mh >= 20);
    assert!((f.mono_advance(32.0) * 3.0 - mw as f32).abs() < 1.5);
    // utanför skärmen får inte krascha
    c.fill_rect(-50, -50, 100, 100, 0xFF0000);
    c.fill_rect(1250, 700, 100, 100, 0xFF0000);
    f.text(&mut c, -20, 710, "utanför", 32.0, w, FontKind::Mono);
    c.blit(&spr, -10, 715, 4, false);
    save_png(&c, "/tmp/engine_test.png").unwrap();
}

struct G(u32);
impl Game for G {
    fn update(&mut self, i: &Input, _dt: f64) -> bool {
        self.0 += i.chars.len() as u32;
        true
    }
    fn draw(&mut self, c: &mut Canvas, f: &mut Fonts) {
        c.clear(rgb(10, 10, 40));
        f.text(c, 10, 10, &format!("tecken: {}", self.0), 32.0, rgb(255, 255, 255), FontKind::Mono);
    }
}

#[test]
fn huvudlost() {
    let mut inp = Input::default();
    inp.chars = vec!['a', 'å'];
    let mut g = G(0);
    run_headless(&mut g, &[Input::default(), inp], &[(1, "/tmp/engine_headless.png".to_string())]);
    assert_eq!(g.0, 2);
    assert!(std::path::Path::new("/tmp/engine_headless.png").exists());
}
