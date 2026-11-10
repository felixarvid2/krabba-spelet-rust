//! Webbläsaren: spelet körs som WebAssembly och ritas i en <canvas> av web/index.html.
//! JS anropar kb_*-funktionerna nedan och hämtar bildrutan som RGBA ur minnet.
use super::*;
use std::cell::RefCell;

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn js_now() -> f64;
    fn js_save(ptr: *const u8, len: usize);
    fn js_load_len() -> usize;
    fn js_load(ptr: *mut u8);
    fn js_clipboard(ptr: *const u8, len: usize);
}

/// Millisekunder sedan epoken (Date.now())
pub fn now_ms() -> f64 {
    unsafe { js_now() }
}

/// Sparar text i localStorage
pub fn storage_save(s: &str) {
    unsafe { js_save(s.as_ptr(), s.len()) }
}

/// Läser text ur localStorage (tom sträng om inget sparats)
pub fn storage_load() -> String {
    let n = unsafe { js_load_len() };
    if n == 0 {
        return String::new();
    }
    let mut buf = vec![0u8; n];
    unsafe { js_load(buf.as_mut_ptr()) };
    String::from_utf8(buf).unwrap_or_default()
}

struct Web {
    game: Box<dyn Game>,
    canvas: Canvas,
    fonts: Fonts,
    input: Input,
    rgba: Vec<u8>,
    alive: bool,
    /// Tangenter och tecken i den ordning de kom, så att snabb inmatning + Enter inte blandas ihop
    queue: std::collections::VecDeque<Ev>,
}

enum Ev {
    Key(Key, bool),
    Char(char),
}

thread_local! {
    static WEB: RefCell<Option<Web>> = const { RefCell::new(None) };
    static PENDING: RefCell<Option<Box<dyn Game>>> = const { RefCell::new(None) };
}

/// Lämnar över spelet till webbläsarslingan (anropas av game::web_start).
pub fn install(game: Box<dyn Game>) {
    PENDING.with(|p| *p.borrow_mut() = Some(game));
}

fn with<R>(f: impl FnOnce(&mut Web) -> R) -> Option<R> {
    WEB.with(|w| {
        let mut w = w.borrow_mut();
        if w.is_none() {
            let g = PENDING.with(|p| p.borrow_mut().take())?;
            *w = Some(Web {
                game: g,
                canvas: Canvas::new(WIDTH, HEIGHT),
                fonts: Fonts::load(),
                input: Input::default(),
                rgba: vec![255; WIDTH * HEIGHT * 4],
                alive: true,
                queue: Default::default(),
            });
        }
        w.as_mut().map(f)
    })
}

fn code_key(code: u32) -> Option<Key> {
    // Samma numrering som web/index.html (KEYS-tabellen)
    const K: [Key; 58] = [
        Key::Up, Key::Down, Key::Left, Key::Right,
        Key::W, Key::A, Key::S, Key::D, Key::E, Key::F, Key::G, Key::H, Key::I, Key::J, Key::K, Key::L, Key::M, Key::N,
        Key::O, Key::P, Key::Q, Key::R, Key::T, Key::U, Key::V, Key::X, Key::Y, Key::Z, Key::B, Key::C,
        Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9,
        Key::Enter, Key::Escape, Key::Tab, Key::Backspace, Key::Delete, Key::Space, Key::Home, Key::End, Key::PageUp, Key::PageDown,
        Key::Shift, Key::Ctrl, Key::Alt,
        Key::F1, Key::F2, Key::F3, Key::F4, Key::F5,
    ];
    if code == 58 {
        return Some(Key::F11);
    }
    K.get(code as usize).copied()
}

/// Startar spelet. Returnerar 1 om det gick.
#[unsafe(no_mangle)]
pub extern "C" fn kb_init() -> u32 {
    if with(|_| ()).is_some() { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn kb_key(code: u32, down: u32) {
    with(|w| {
        if let Some(k) = code_key(code) {
            w.queue.push_back(Ev::Key(k, down != 0));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kb_char(c: u32) {
    with(|w| {
        if let Some(ch) = char::from_u32(c) {
            if c >= 32 && c != 127 {
                w.queue.push_back(Ev::Char(ch));
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kb_mouse(x: i32, y: i32) {
    with(|w| {
        w.input.mouse_x = x.clamp(0, WIDTH as i32 - 1);
        w.input.mouse_y = y.clamp(0, HEIGHT as i32 - 1);
    });
}

/// button: 0 = vänster, 2 = höger
#[unsafe(no_mangle)]
pub extern "C" fn kb_button(button: u32, down: u32) {
    with(|w| {
        if button == 0 {
            if down != 0 && !w.input.mouse_down {
                w.input.clicked = true;
            }
            w.input.mouse_down = down != 0;
        } else if button == 2 && down != 0 {
            w.input.right_clicked = true;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kb_wheel(dy: f32) {
    with(|w| w.input.wheel += dy);
}

/// Kör en bildruta och ritar. Returnerar pekaren till RGBA-bufferten (WIDTH×HEIGHT×4), 0 om spelet avslutats.
#[unsafe(no_mangle)]
pub extern "C" fn kb_frame(dt: f64) -> *const u8 {
    with(|w| {
        if !w.alive {
            return std::ptr::null();
        }
        // Ta händelser i ordning; en tangent efter inskrivna tecken väntar till nästa bildruta
        while let Some(ev) = w.queue.pop_front() {
            match ev {
                Ev::Char(c) => w.input.chars.push(c),
                Ev::Key(k, true) => {
                    if !w.input.chars.is_empty() || w.input.pressed.contains(&k) {
                        w.queue.push_front(Ev::Key(k, true));
                        break;
                    }
                    w.input.down.insert(k);
                    w.input.pressed.push(k);
                }
                Ev::Key(k, false) => {
                    if w.input.pressed.contains(&k) {
                        w.queue.push_front(Ev::Key(k, false));
                        break;
                    }
                    w.input.down.remove(&k);
                }
            }
        }
        w.input.ctrl = w.input.down.contains(&Key::Ctrl);
        w.input.shift = w.input.down.contains(&Key::Shift);
        w.alive = w.game.update(&w.input, dt.clamp(0.0, 0.1));
        w.input.pressed.clear();
        w.input.chars.clear();
        w.input.clicked = false;
        w.input.right_clicked = false;
        w.input.wheel = 0.0;
        w.game.draw(&mut w.canvas, &mut w.fonts);
        if let Some(t) = w.game.take_clipboard() {
            unsafe { js_clipboard(t.as_ptr(), t.len()) };
        }
        for (i, p) in w.canvas.px.iter().enumerate() {
            let o = i * 4;
            w.rgba[o] = (p >> 16) as u8;
            w.rgba[o + 1] = (p >> 8) as u8;
            w.rgba[o + 2] = *p as u8;
        }
        w.rgba.as_ptr()
    })
    .unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
pub extern "C" fn kb_width() -> u32 {
    WIDTH as u32
}
#[unsafe(no_mangle)]
pub extern "C" fn kb_height() -> u32 {
    HEIGHT as u32
}

/// Minne för JS att skriva text till (storage_load)
#[unsafe(no_mangle)]
pub extern "C" fn kb_alloc(n: usize) -> *mut u8 {
    let mut v = vec![0u8; n];
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

thread_local! {
    static AUDIO_BUF: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
}

/// Ljudet får starta (efter första klick eller tangent)
#[unsafe(no_mangle)]
pub extern "C" fn kb_audio_unlock() {
    super::audio::unlock();
}

/// Blandar `frames` stereoramar (44,1 kHz, L R L R …) och returnerar pekaren till dem
#[unsafe(no_mangle)]
pub extern "C" fn kb_audio(frames: usize) -> *const f32 {
    AUDIO_BUF.with(|b| {
        let mut b = b.borrow_mut();
        b.resize(frames * 2, 0.0);
        super::audio::render(&mut b);
        b.as_ptr()
    })
}
