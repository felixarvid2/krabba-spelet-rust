//! Fler IOS-kommandon från js/cli/ios.js (rad 1611–slutet): banner, VTP, debug, erase, ssh/telnet, fler show m.m.
//! Hjälptexterna (HELP/SHOW_HELP) för nyckelorden här ligger i ios_help.rs.
#![allow(clippy::too_many_lines)]
use super::ios::{Args, IosSession, Reg, NO_NEG, NO_ONLY};
use super::ios_show as SH;
use super::{CmdResult, PushReq};
use crate::net::model::*;
use crate::net::sim::{self as S, AclPkt, PingOpts, State};
use crate::net::util as U;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

// ------------------------------------------------------------------ Hjälpfunktioner

fn cfg<'a>(s: &IosSession, st: &'a mut State) -> &'a mut IosConfig {
    st.dev_mut(&s.dev).cfg_mut()
}
fn cfg_r<'a>(s: &IosSession, st: &'a State) -> &'a IosConfig {
    st.dev(&s.dev).cfg()
}
fn ok() -> CmdResult {
    CmdResult::empty()
}
fn out(s: impl Into<String>) -> CmdResult {
    CmdResult::out(s)
}
/// Vänsterjusterad (lpad i JS)
fn lp(s: impl std::fmt::Display, n: usize) -> String {
    U::pad(s, n)
}
/// Högerjusterad (rpad i JS)
fn rp(s: impl std::fmt::Display, n: usize) -> String {
    U::pad_l(s, n)
}
/// Resten av raden efter första förekomsten av ordet (skiftlägesoberoende)
fn rest(line: &str, word: &str) -> String {
    let l = line.to_ascii_lowercase();
    match l.find(&word.to_ascii_lowercase()) {
        Some(i) => line[i + word.len()..].trim().to_string(),
        None => String::new(),
    }
}
fn invalid_plain() -> String {
    "% Invalid input detected at '^' marker.".to_string()
}

/// Kör f på varje valt interface; unika utskrifter slås ihop (eachIf)
fn each_if<F>(s: &IosSession, st: &mut State, mut f: F) -> CmdResult
where
    F: FnMut(&mut Iface, &str) -> Option<String>,
{
    let names = s.ctx.ifs.clone();
    let c = cfg(s, st);
    let mut outs: Vec<String> = vec![];
    for n in &names {
        if let Some(i) = c.ifaces.get_mut(n) {
            if let Some(r) = f(i, n) {
                if !r.is_empty() && !outs.contains(&r) {
                    outs.push(r);
                }
            }
        }
    }
    out(outs.join("\n"))
}

fn each_line<F>(s: &IosSession, st: &mut State, mut f: F)
where
    F: FnMut(&mut Line),
{
    let names = s.ctx.lines.clone();
    let c = cfg(s, st);
    for n in &names {
        f(c.lines.get_mut(n));
    }
}


fn sw_only<F>(f: F) -> impl Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static
where
    F: Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static,
{
    move |s, st, a| {
        if !s.is_switch(st) {
            let w = a.kws.get(1).or(a.kws.first()).cloned().unwrap_or_default();
            return out(s.invalid_marker(st, &a.line, &w));
        }
        f(s, st, a)
    }
}
fn rt_only<F>(f: F) -> impl Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static
where
    F: Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static,
{
    move |s, st, a| {
        if !s.is_router(st) {
            let w = a.kws.get(1).or(a.kws.first()).cloned().unwrap_or_default();
            return out(s.invalid_marker(st, &a.line, &w));
        }
        f(s, st, a)
    }
}

/// Spara en rad i globala konfigurationen. on = standard är på (bara no-formen syns).
fn keep_g<T>(key: &'static str, text: T, on: bool) -> impl Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static
where
    T: Fn(&Args) -> String + 'static,
{
    move |s, st, a| {
        let x = &mut cfg(s, st).x;
        if on {
            if a.neg {
                x.insert(key.to_string(), format!("no {}", text(a)));
            } else {
                x.shift_remove(key);
            }
        } else if a.neg {
            x.shift_remove(key);
        } else {
            x.insert(key.to_string(), text(a));
        }
        ok()
    }
}
fn keep_if<T>(key: &'static str, text: T, on: bool) -> impl Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static
where
    T: Fn(&Args) -> String + 'static,
{
    move |s, st, a| {
        let t = text(a);
        let neg = a.neg;
        each_if(s, st, |i, _| {
            if on {
                if neg {
                    i.x.insert(key.to_string(), format!("no {}", t));
                } else {
                    i.x.shift_remove(key);
                }
            } else if neg {
                i.x.shift_remove(key);
            } else {
                i.x.insert(key.to_string(), t.clone());
            }
            None
        })
    }
}
fn keep_line<T>(key: &'static str, text: T) -> impl Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static
where
    T: Fn(&Args) -> String + 'static,
{
    move |s, st, a| {
        let t = text(a);
        let neg = a.neg;
        each_line(s, st, |l| {
            if neg {
                l.x.shift_remove(key);
            } else {
                l.x.insert(key.to_string(), t.clone());
            }
        });
        ok()
    }
}
fn lit(t: &'static str) -> impl Fn(&Args) -> String + 'static {
    move |_| t.to_string()
}

fn vtp(c: &mut IosConfig) -> &mut Vtp {
    c.vtp.get_or_insert_with(|| Vtp { mode: "server".into(), domain: String::new(), version: 1, password: None })
}
fn snmp(c: &mut IosConfig) -> &mut Snmp {
    c.snmp.get_or_insert_with(Snmp::default)
}

/// "hh:mm:ss" -> (h, m, s)
fn parse_hms(t: &str) -> Option<(u32, u32, u32)> {
    let p: Vec<&str> = t.split(':').collect();
    if p.len() != 3 {
        return None;
    }
    let ok_n = |x: &str, lo: usize, hi: usize| x.len() >= lo && x.len() <= hi && x.bytes().all(|b| b.is_ascii_digit());
    if !ok_n(p[0], 1, 2) || !ok_n(p[1], 2, 2) || !ok_n(p[2], 2, 2) {
        return None;
    }
    Some((p[0].parse().ok()?, p[1].parse().ok()?, p[2].parse().ok()?))
}

fn month_idx(w: &str) -> Option<u32> {
    let m = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    let w3: String = w.to_lowercase().chars().take(3).collect();
    m.iter().position(|x| *x == w3).map(|i| i as u32)
}

// Kabeltestresultat (JS: dev.rt.tdr) – finns inte i Rt, så de hålls här.
#[derive(Clone, Debug, Default)]
struct Tdr {
    fiber: bool,
    open: bool,
    len: u32,
    half: bool,
}
thread_local! {
    static TDR: RefCell<HashMap<String, Tdr>> = RefCell::new(HashMap::new());
}

// ------------------------------------------------------------------ Registrering

pub fn register(reg: &mut Reg) {
    config_cmds(reg);
    line_cmds(reg);
    stp_cmds(reg);
    misc_cfg_cmds(reg);
    if_cmds(reg);
    exec_cmds(reg);
    show_cmds(reg);
    v7_cmds(reg);
}

// ---------- banner, lösenord, inloggning
fn banner_more(s: &mut IosSession, kind: String, delim: String, lines: Vec<String>) {
    s.ask("", false, move |s, st, l| {
        let mut lines = lines;
        if let Some(e) = l.find(&delim) {
            lines.push(l[..e].to_string());
            let txt = lines.join("\n");
            let txt = txt.strip_prefix('\n').unwrap_or(&txt).to_string();
            cfg(s, st).banners.insert(kind, Banner { d: delim, text: txt });
            return ok();
        }
        lines.push(l.to_string());
        banner_more(s, kind, delim, lines);
        ok()
    });
}

fn user_algo(s: &mut IosSession, st: &mut State, a: &Args) -> CmdResult {
    let name = a.s(0).to_string();
    if a.neg {
        cfg(s, st).users.shift_remove(&name);
        return ok();
    }
    let pw = a.v.last().map(|v| v.s().to_string()).unwrap_or_default();
    let min = cfg(s, st).min_pw_len;
    if min != 0 && (pw.chars().count() as i64) < min {
        return out(format!("% Password too short - must be at least {} characters. Password configuration failed", min));
    }
    let priv_ = if a.has_kw("privilege") { a.n(1) } else { 1 };
    let secret = SH::hash_secret(&pw);
    cfg(s, st).users.insert(name, User { priv_, secret, plain: pw, by_game: false, pw_type: None });
    ok()
}

fn config_cmds(reg: &mut Reg) {
    for kind in ["motd", "login", "exec"] {
        reg.d("config", &format!("banner {} <text>", kind), move |s, st, a| {
            if a.neg {
                cfg(s, st).banners.shift_remove(kind);
                return ok();
            }
            let txt = rest(&a.line, kind);
            let mut ch = txt.chars();
            let delim = match ch.next() {
                Some(c) => c.to_string(),
                None => return ok(),
            };
            let body = &txt[delim.len()..];
            if let Some(end) = body.find(&delim) {
                cfg(s, st).banners.insert(kind.to_string(), Banner { d: delim, text: body[..end].to_string() });
                return ok();
            }
            let msg = format!("Enter TEXT message.  End with the character '{}'.", delim);
            banner_more(s, kind.to_string(), delim.clone(), vec![body.to_string()]);
            out(msg)
        });
        reg.def(
            "config",
            &format!("banner {}", kind),
            move |s, st, _| {
                cfg(s, st).banners.shift_remove(kind);
                ok()
            },
            NO_ONLY,
        );
    }

    reg.d("config", "security passwords min-length <n:0-16>", |s, st, a| {
        cfg(s, st).min_pw_len = if a.neg { 0 } else { a.n(0) };
        ok()
    });
    reg.d(
        "config",
        "login block-for <n:1-65535> attempts <n:1-65535> within <n:1-65535>",
        keep_g("login block-for", |a| format!("login block-for {} attempts {} within {}", a.n(0), a.n(1), a.n(2)), false),
    );
    reg.d("config", "login on-failure log", keep_g("login on-failure", lit("login on-failure log"), false));
    reg.d("config", "login on-success log", keep_g("login on-success", lit("login on-success log"), false));
    reg.d("config", "ip ssh time-out <n:1-120>", |s, st, a| {
        cfg(s, st).ssh.timeout = if a.neg { None } else { Some(a.n(0)) };
        ok()
    });
    reg.d("config", "ip ssh authentication-retries <n:0-5>", |s, st, a| {
        cfg(s, st).ssh.retries = if a.neg { None } else { Some(a.n(0)) };
        ok()
    });
    reg.d("config", "service password-encryption", |s, st, a| {
        cfg(s, st).pw_encrypt = !a.neg;
        ok()
    });
    reg.d("config", "username <word> algorithm-type <word> secret <word>", user_algo);
    reg.d("config", "username <word> privilege <n:0-15> algorithm-type <word> secret <word>", user_algo);

    // VTP
    reg.d(
        "config",
        "vtp mode <word>",
        sw_only(|s, st, a| {
            let w = a.s(0).to_lowercase();
            let m = ["server", "client", "transparent", "off"].into_iter().find(|x| x.starts_with(&w));
            let m = match m {
                Some(m) => m,
                None => return out(s.invalid_marker(st, &a.line, a.s(0))),
            };
            vtp(cfg(s, st)).mode = if a.neg { "server".into() } else { m.to_string() };
            if a.neg {
                return ok();
            }
            out(format!("Setting device to VTP {} mode{}", m.to_uppercase(), if m == "off" { "." } else { " for VLANS." }))
        }),
    );
    reg.d(
        "config",
        "vtp domain <word>",
        sw_only(|s, st, a| {
            let t = vtp(cfg(s, st));
            let old = std::mem::replace(&mut t.domain, a.s(0).to_string());
            if old == a.s(0) {
                out(format!("Domain name already set to {}.", a.s(0)))
            } else {
                out(format!("Changing VTP domain name from {} to {}", if old.is_empty() { "NULL" } else { &old }, a.s(0)))
            }
        }),
    );
    reg.d(
        "config",
        "vtp version <n:1-3>",
        sw_only(|s, st, a| {
            vtp(cfg(s, st)).version = if a.neg { 1 } else { a.n(0) };
            ok()
        }),
    );
    reg.d(
        "config",
        "vtp password <word>",
        sw_only(|s, st, a| {
            vtp(cfg(s, st)).password = if a.neg { None } else { Some(a.s(0).to_string()) };
            if a.neg {
                out("Clearing device VTP password.")
            } else {
                out(format!("Setting device VTP password to {}", a.s(0)))
            }
        }),
    );
}

// ---------- Linjer
fn line_cmds(reg: &mut Reg) {
    reg.d("line", "access-class <word> in", |s, st, a| {
        let (neg, v) = (a.neg, a.s(0).to_string());
        each_line(s, st, |l| l.access_class = if neg { None } else { Some(v.clone()) });
        ok()
    });
    reg.d("line", "access-class <word> out", keep_line("access-class out", |a| format!("access-class {} out", a.s(0))));
    reg.d("line", "privilege level <n:0-15>", |s, st, a| {
        let (neg, v) = (a.neg, a.n(0));
        each_line(s, st, |l| l.priv_level = if neg { None } else { Some(v) });
        ok()
    });
    reg.d("line", "transport output <text>", |s, st, a| {
        let t = rest(&a.line, "output");
        let neg = a.neg;
        each_line(s, st, |l| {
            if neg {
                l.x.shift_remove("tout");
            } else {
                l.x.insert("tout".into(), format!("transport output {}", t));
            }
        });
        ok()
    });
    reg.d("line", "history size <n:0-256>", keep_line("history", |a| format!("history size {}", a.n(0))));
    reg.d("line", "session-timeout <n:0-35791>", keep_line("session-timeout", |a| format!("session-timeout {}", a.n(0))));
    reg.d("line", "length <n:0-512>", keep_line("length", |a| format!("length {}", a.n(0))));
    reg.d("line", "width <n:0-512>", keep_line("width", |a| format!("width {}", a.n(0))));
    reg.d("line", "exec", |s, st, a| {
        let neg = a.neg;
        each_line(s, st, |l| {
            if neg {
                l.x.insert("exec".into(), "no exec".into());
            } else {
                l.x.shift_remove("exec");
            }
        });
        ok()
    });
    reg.d("line", "motd-banner", |s, st, a| {
        let neg = a.neg;
        each_line(s, st, |l| {
            if neg {
                l.x.insert("motd".into(), "no motd-banner".into());
            } else {
                l.x.shift_remove("motd");
            }
        });
        ok()
    });
}

// ---------- Spanning tree, CDP, LLDP
fn stp_root(s: &mut IosSession, st: &mut State, a: &Args, prio: u32) -> CmdResult {
    let vl = a.v.first().map(|v| v.vlans()).unwrap_or_default();
    let neg = a.neg;
    let c = cfg(s, st);
    for x in vl {
        if neg {
            c.stp_priority.remove(&x);
        } else {
            c.stp_priority.insert(x, prio);
        }
    }
    S::touch(st);
    ok()
}

fn stp_cmds(reg: &mut Reg) {
    reg.d("config", "spanning-tree vlan <vlist> root primary", sw_only(|s, st, a| stp_root(s, st, a, 24576)));
    reg.d("config", "spanning-tree vlan <vlist> root secondary", sw_only(|s, st, a| stp_root(s, st, a, 28672)));
    reg.d("config", "spanning-tree portfast default", sw_only(keep_g("stp pf default", lit("spanning-tree portfast default"), false)));
    reg.d("config", "spanning-tree portfast edge default", sw_only(keep_g("stp pf default", lit("spanning-tree portfast edge default"), false)));
    reg.d("config", "spanning-tree portfast bpduguard default", sw_only(keep_g("stp bpdu default", lit("spanning-tree portfast bpduguard default"), false)));
    reg.d(
        "config",
        "spanning-tree portfast edge bpduguard default",
        sw_only(keep_g("stp bpdu default", lit("spanning-tree portfast edge bpduguard default"), false)),
    );
    reg.d("config", "spanning-tree loopguard default", sw_only(keep_g("stp loopguard", lit("spanning-tree loopguard default"), false)));
    reg.d("config", "cdp run", |s, st, a| {
        cfg(s, st).cdp_off = a.neg;
        ok()
    });
    reg.d("config", "lldp run", |s, st, a| {
        cfg(s, st).lldp = !a.neg;
        ok()
    });
    reg.d("config", "cdp timer <n:5-254>", keep_g("cdp timer", |a| format!("cdp timer {}", a.n(0)), false));
    reg.d("config", "cdp holdtime <n:10-255>", keep_g("cdp holdtime", |a| format!("cdp holdtime {}", a.n(0)), false));
}

// ---------- Loggning, NTP, klocka, SNMP och övrigt
fn misc_cfg_cmds(reg: &mut Reg) {
    for w in ["trap", "console", "monitor"] {
        let key: &'static str = match w {
            "trap" => "logging trap",
            "console" => "logging console",
            _ => "logging monitor",
        };
        reg.d("config", &format!("logging {} <word>", w), keep_g(key, move |a| format!("logging {} {}", w, a.s(0)), false));
    }
    reg.d("config", "logging console", |s, st, a| {
        let x = &mut cfg(s, st).x;
        if a.neg {
            x.insert("logging console".into(), "no logging console".into());
        } else {
            x.shift_remove("logging console");
        }
        ok()
    });
    reg.d("config", "logging source-interface <if>", keep_g("logging source", |a| format!("logging source-interface {}", a.s(0)), false));
    reg.d("config", "ntp update-calendar", keep_g("ntp update-calendar", lit("ntp update-calendar"), false));
    reg.d("config", "ntp master", keep_g("ntp master", lit("ntp master"), false));
    reg.d("config", "ntp master <n:1-15>", keep_g("ntp master", |a| format!("ntp master {}", a.n(0)), false));
    reg.d("config", "clock summer-time <word> recurring", keep_g("clock summer-time", |a| format!("clock summer-time {} recurring", a.s(0)), false));
    reg.d("config", "clock summer-time <word> recurring <text>", |s, st, a| {
        let x = &mut cfg(s, st).x;
        if a.neg {
            x.shift_remove("clock summer-time");
        } else {
            x.insert("clock summer-time".into(), format!("clock summer-time {} recurring {}", a.s(0), rest(&a.line, "recurring")));
        }
        ok()
    });
    reg.d("config", "snmp-server community <word> <word>", |s, st, a| {
        let acc = a.s(1).to_uppercase();
        if acc != "RO" && acc != "RW" {
            return out(s.invalid_marker(st, &a.line, a.s(1)));
        }
        let sn = snmp(cfg(s, st));
        if a.neg {
            sn.communities.shift_remove(a.s(0));
        } else {
            sn.communities.insert(a.s(0).to_string(), acc);
        }
        ok()
    });
    reg.d("config", "snmp-server community <word>", |s, st, a| {
        let sn = snmp(cfg(s, st));
        if a.neg {
            sn.communities.shift_remove(a.s(0));
        } else {
            sn.communities.insert(a.s(0).to_string(), "RO".into());
        }
        ok()
    });
    reg.d("config", "snmp-server location <text>", |s, st, a| {
        let t = rest(&a.line, "location");
        snmp(cfg(s, st)).location = if a.neg { None } else { Some(t) };
        ok()
    });
    reg.d("config", "snmp-server contact <text>", |s, st, a| {
        let t = rest(&a.line, "contact");
        snmp(cfg(s, st)).contact = if a.neg { None } else { Some(t) };
        ok()
    });
    reg.d("config", "ipv6 unicast-routing", keep_g("ipv6 unicast-routing", lit("ipv6 unicast-routing"), false));
    reg.d("config", "errdisable recovery interval <n:30-86400>", keep_g("errdisable interval", |a| format!("errdisable recovery interval {}", a.n(0)), false));
    reg.d("config", "errdisable recovery cause <word>", |s, st, a| {
        let c = cfg(s, st);
        if a.s(0).starts_with("psec") {
            c.err_recovery = !a.neg;
            return ok();
        }
        let k = format!("errdisable cause {}", a.s(0));
        if a.neg {
            c.x.shift_remove(&k);
        } else {
            c.x.insert(k, format!("errdisable recovery cause {}", a.s(0)));
        }
        ok()
    });
    reg.d(
        "config",
        "mac address-table static <word> vlan <n:1-4094> interface <if>",
        sw_only(keep_g("mac static", |a| format!("mac address-table static {} vlan {} interface {}", a.s(0), a.n(1), a.s(2)), false)),
    );
    reg.d("config", "mac address-table aging-time <n:0-1000000>", sw_only(keep_g("mac aging", |a| format!("mac address-table aging-time {}", a.n(0)), false)));
    reg.d("config", "port-channel load-balance <word>", sw_only(keep_g("pc lb", |a| format!("port-channel load-balance {}", a.s(0)), false)));
    reg.d("config", "ip dhcp snooping", sw_only(keep_g("dhcp snooping", lit("ip dhcp snooping"), false)));
    reg.d(
        "config",
        "ip dhcp snooping vlan <vlist>",
        sw_only(keep_g("dhcp snooping vlan", |a| format!("ip dhcp snooping vlan {}", U::vlan_list_str(&a.v[0].vlans())), false)),
    );
    reg.d("config", "ip dhcp snooping information option", sw_only(keep_g("dhcp snooping opt82", lit("ip dhcp snooping information option"), true)));
    reg.d(
        "config",
        "ip arp inspection vlan <vlist>",
        sw_only(keep_g("dai", |a| format!("ip arp inspection vlan {}", U::vlan_list_str(&a.v[0].vlans())), false)),
    );
    reg.d("config", "ip domain lookup source-interface <if>", keep_g("dns src", |a| format!("ip domain lookup source-interface {}", a.s(0)), false));
    reg.d("config", "ip tftp source-interface <if>", keep_g("tftp src", |a| format!("ip tftp source-interface {}", a.s(0)), false));
    reg.d("config", "ip classless", |_, _, _| ok());
    reg.d("config", "ip subnet-zero", |_, _, _| ok());
    reg.d("config", "ip source-route", keep_g("ip source-route", lit("ip source-route"), true));
    reg.d("config", "ip forward-protocol nd", |_, _, _| ok());
}

// ---------- Interface
fn if_cmds(reg: &mut Reg) {
    reg.d("if", "switchport voice vlan <n:1-4094>", |s, st, a| {
        let sw = s.is_switch(st);
        let (neg, v) = (a.neg, a.n(0));
        each_if(s, st, |i, _| {
            let kind = if i.parent.is_some() {
                "sub"
            } else if i.svi.is_some() {
                "svi"
            } else if i.is_loop {
                "l3"
            } else if sw {
                "l2"
            } else {
                "l3"
            };
            if kind != "l2" {
                return Some(invalid_plain());
            }
            i.voice_vlan = if neg { None } else { Some(v) };
            None
        })
    });
    reg.d("if", "spanning-tree portfast edge", |s, st, a| {
        let neg = a.neg;
        each_if(s, st, |i, _| {
            i.portfast = !neg;
            None
        })
    });
    reg.d("if", "spanning-tree bpduguard enable", keep_if("bpduguard", lit("spanning-tree bpduguard enable"), false));
    reg.d("if", "spanning-tree bpduguard disable", keep_if("bpduguard", lit("spanning-tree bpduguard disable"), false));
    reg.d("if", "spanning-tree bpdufilter enable", keep_if("bpdufilter", lit("spanning-tree bpdufilter enable"), false));
    reg.d("if", "spanning-tree guard <word>", keep_if("guard", |a| format!("spanning-tree guard {}", a.s(0)), false));
    reg.d("if", "spanning-tree cost <n:1-200000000>", keep_if("stp cost", |a| format!("spanning-tree cost {}", a.n(0)), false));
    reg.d("if", "spanning-tree port-priority <n:0-240>", keep_if("stp prio", |a| format!("spanning-tree port-priority {}", a.n(0)), false));
    reg.d("if", "spanning-tree link-type <word>", keep_if("stp link", |a| format!("spanning-tree link-type {}", a.s(0)), false));
    reg.d("if", "channel-group <n:1-48> mode <word>", |s, st, a| {
        let w = a.s(1).to_lowercase();
        let m = ["active", "passive", "on", "desirable", "auto"].into_iter().find(|x| x.starts_with(&w));
        let m = match m {
            Some(m) => m.to_string(),
            None => return out(s.invalid_marker(st, &a.line, a.s(1))),
        };
        let n = a.n(0);
        let neg = a.neg;
        let exists = cfg(s, st).ifaces.values().any(|q| q.ch_group.as_ref().map(|g| g.n == n).unwrap_or(false));
        let made = !neg && !exists;
        let r = each_if(s, st, |i, _| {
            if neg {
                i.ch_group = None;
            } else {
                i.ch_group = Some(ChGroup { n, mode: m.clone() });
            }
            None
        });
        out(format!("{}{}", if made { format!("Creating a port-channel interface Port-channel {}\n", n) } else { String::new() }, r.out))
    });
    reg.def(
        "if",
        "channel-group",
        |s, st, _| {
            each_if(s, st, |i, _| {
                i.ch_group = None;
                None
            })
        },
        NO_ONLY,
    );
    for t in ["broadcast", "multicast", "unicast"] {
        reg.d("if", &format!("storm-control {} level <text>", t), move |s, st, a| {
            let lv = rest(&a.line, "level");
            let neg = a.neg;
            each_if(s, st, |i, _| {
                let k = format!("storm {}", t);
                if neg {
                    i.x.shift_remove(&k);
                } else {
                    i.x.insert(k, format!("storm-control {} level {}", t, lv));
                }
                None
            })
        });
    }
    reg.d("if", "storm-control action <word>", keep_if("storm action", |a| format!("storm-control action {}", a.s(0)), false));
    reg.d("if", "ip dhcp snooping trust", keep_if("snoop trust", lit("ip dhcp snooping trust"), false));
    reg.d("if", "ip dhcp snooping limit rate <n:1-2048>", keep_if("snoop rate", |a| format!("ip dhcp snooping limit rate {}", a.n(0)), false));
    reg.d("if", "ip arp inspection trust", keep_if("dai trust", lit("ip arp inspection trust"), false));
    reg.d("if", "switchport port-security aging time <n:0-1440>", keep_if("ps aging", |a| format!("switchport port-security aging time {}", a.n(0)), false));
    reg.d("if", "switchport port-security aging type <word>", keep_if("ps agetype", |a| format!("switchport port-security aging type {}", a.s(0)), false));
    reg.d("if", "switchport port-security mac-address <word>", |s, st, a| {
        if a.s(0).eq_ignore_ascii_case("sticky") {
            return ok();
        }
        let w = a.s(0).to_string();
        let neg = a.neg;
        each_if(s, st, |i, _| {
            let key = format!("ps mac {}", w);
            if neg {
                i.x.shift_remove(&key);
            } else {
                i.x.insert(key, format!("switchport port-security mac-address {}", w));
            }
            None
        })
    });
    reg.d("if", "mdix auto", keep_if("mdix", lit("mdix auto"), true));
    reg.d("if", "load-interval <n:30-600>", keep_if("load", |a| format!("load-interval {}", a.n(0)), false));
    reg.d("if", "udld port", keep_if("udld", lit("udld port"), false));
    reg.d("if", "udld port aggressive", keep_if("udld", lit("udld port aggressive"), false));
    // bandwidth och ip ospf cost ersätts av ios_dyn
    reg.d("if", "delay <n:1-16777215>", keep_if("delay", |a| format!("delay {}", a.n(0)), false));
    reg.d("if", "mtu <n:64-9216>", keep_if("mtu", |a| format!("mtu {}", a.n(0)), false));
    reg.d("if", "keepalive", keep_if("keepalive", lit("keepalive"), true));
    reg.d("if", "keepalive <n:0-32767>", keep_if("keepalive", |a| format!("keepalive {}", a.n(0)), false));
    reg.d("if", "ip redirects", keep_if("redirects", lit("ip redirects"), true));
    reg.d("if", "ip unreachables", keep_if("unreach", lit("ip unreachables"), true));
    reg.d("if", "ip proxy-arp", keep_if("proxyarp", lit("ip proxy-arp"), true));
    reg.d("if", "ip virtual-reassembly", keep_if("vreasm", lit("ip virtual-reassembly"), false));
    reg.d("if", "ip virtual-reassembly in", keep_if("vreasm", lit("ip virtual-reassembly in"), false));
    reg.d("if", "media-type <word>", keep_if("media", |a| format!("media-type {}", a.s(0)), false));
    reg.d("if", "ipv6 address <word>", keep_if("ipv6 ", |a| format!("ipv6 address {}", a.s(0)), false));
    reg.d("if", "ipv6 enable", keep_if("ipv6 en", lit("ipv6 enable"), false));
    reg.d("if", "standby <n:0-255> ip <ip>", |s, st, a| {
        let (grp, ip, neg) = (a.n(0), a.s(1).to_string(), a.neg);
        each_if(s, st, |i, _| {
            if neg {
                i.hsrp = None;
            } else {
                let h = i.hsrp.get_or_insert(Hsrp { grp, pri: 100, preempt: false, ip: None });
                h.grp = grp;
                h.ip = Some(ip.clone());
            }
            None
        })
    });
    reg.d("if", "standby <n:0-255> priority <n:0-255>", |s, st, a| {
        let (grp, pri, neg) = (a.n(0), a.n(1), a.neg);
        each_if(s, st, |i, _| {
            let h = i.hsrp.get_or_insert(Hsrp { grp, pri: 100, preempt: false, ip: None });
            h.pri = if neg { 100 } else { pri };
            None
        })
    });
    reg.d("if", "standby <n:0-255> preempt", |s, st, a| {
        let (grp, neg) = (a.n(0), a.neg);
        each_if(s, st, |i, _| {
            let h = i.hsrp.get_or_insert(Hsrp { grp, pri: 100, preempt: false, ip: None });
            h.preempt = !neg;
            None
        })
    });
}

// ---------- Exec: debug, klocka, erase, reload, tftp, ssh/telnet
const DEBUGS: [(&str, &str); 11] = [
    ("ip packet", "IP packet debugging is on"),
    ("ip icmp", "ICMP packet debugging is on"),
    ("ip nat", "IP NAT debugging is on"),
    ("ip dhcp server events", "DHCP server event debugging is on."),
    ("ip dhcp server packet", "DHCP server packet debugging is on."),
    ("spanning-tree events", "Spanning Tree event debugging is on"),
    ("crypto isakmp", "Crypto ISAKMP debugging is on"),
    ("crypto ipsec", "Crypto IPSEC debugging is on"),
    ("ip ssh", "Incoming ssh debugging is on"),
    ("cdp packets", "CDP packet info debugging is on"),
    ("ip routing", "IP routing debugging is on"),
];

fn set_clock(s: &mut IosSession, st: &mut State, a: &Args, mon_pos: usize, day_pos: usize) -> CmdResult {
    let hms = parse_hms(a.s(0));
    let mon = month_idx(a.s(mon_pos));
    let (h, m, sec) = match hms {
        Some(x) => x,
        None => return out(s.invalid_marker(st, &a.line, a.s(0))),
    };
    let mon = match mon {
        Some(x) => x,
        None => return out(s.invalid_marker(st, &a.line, a.s(mon_pos))),
    };
    let ms = U::date_utc(a.n(3), mon, a.n(day_pos) as u32, h, m, sec);
    let at = st.time;
    st.dev_mut(&s.dev).rt.clock_set = Some(ClockSet { ms, at });
    ok()
}

fn erase_startup(s: &mut IosSession, _st: &mut State, _a: &Args) -> CmdResult {
    s.ask("Erasing the nvram filesystem will remove all configuration files! Continue? [confirm]", false, |s, st, a| {
        if !a.is_empty() && !a.to_lowercase().starts_with('y') {
            return ok();
        }
        st.dev_mut(&s.dev).startup = None;
        CmdResult::delayed("[OK]\nErase of nvram: complete", 600)
    });
    ok()
}

fn tftp_copy(s: &mut IosSession, st: &mut State, _a: &Args) -> CmdResult {
    let file = format!("{}-confg", cfg_r(s, st).hostname.to_lowercase());
    s.ask("Address or name of remote host []? ", false, move |s, _st, host| {
        let host = host.to_string();
        if host.is_empty() {
            return out("%Error parsing filename (Bad IP address or host name)");
        }
        let prompt = format!("Destination filename [{}]? ", file);
        s.ask(prompt, false, move |s, st, f| {
            let ip = if U::is_ip(&host) { Some(host.clone()) } else { cfg_r(s, st).hosts.get(&host.to_lowercase()).cloned() };
            let ip = match ip {
                Some(i) => i,
                None => return out("%Error parsing filename (Bad IP address or host name)"),
            };
            let p = S::ping(st, &s.dev.clone(), &ip, PingOpts::default());
            if !p.ok {
                let name = if f.is_empty() { file.clone() } else { f.to_string() };
                return CmdResult::delayed(format!(".....\n%Error opening tftp://{}/{} (Timed out)", ip, name), 2500);
            }
            let bytes = SH::running_config(st, &s.dev.clone()).chars().count();
            CmdResult::delayed(format!("!!\n{} bytes copied in 1.084 secs ({} bytes/sec)", bytes, (bytes as f64 / 1.084).round() as i64), 900)
        });
        ok()
    });
    ok()
}

fn remote_login(s: &mut IosSession, st: &mut State, host: &str, user: Option<String>, proto: &'static str) -> CmdResult {
    let ip = if U::is_ip(host) { Some(host.to_string()) } else { cfg_r(s, st).hosts.get(&host.to_lowercase()).cloned() };
    let ip = match ip {
        Some(i) => i,
        None => return out("% Unknown command or computer name, or unable to find computer address"),
    };
    let port = if proto == "ssh" { 22 } else { 23 };
    let me = s.dev.clone();
    let p = S::ping(st, &me, &ip, PingOpts::tcp(port));
    let head = if proto == "telnet" { format!("Trying {} ... ", ip) } else { String::new() };
    if !p.ok {
        let refused = p.reason.as_deref() == Some("refused");
        return CmdResult::delayed(
            format!("{}{}", head, if refused { "\n% Connection refused by remote host" } else { "\n% Connection timed out; remote host not responding" }),
            if refused { 300 } else { 3000 },
        );
    }
    let d = S::get(st);
    let mut tgt: Option<String> = None;
    for e in d.eps.values() {
        if e.ip.as_deref() == Some(ip.as_str()) && e.up && st.devices.get(&e.dev).map(|x| x.os == Os::Ios).unwrap_or(false) {
            tgt = Some(e.dev.clone());
        }
    }
    let tgt = tgt.or(p.at.clone());
    let tgt = match tgt {
        Some(t) if st.devices.get(&t).map(|x| x.os == Os::Ios).unwrap_or(false) => t,
        _ => return out(format!("{}\n% Connection refused by remote host", head)),
    };
    if tgt == me {
        return out(format!("{}\n% Connection refused by remote host", head));
    }
    let line = st.dev(&tgt).cfg().lines.vty0_4.clone();
    if let Some(ac) = &line.access_class {
        if let Some(src) = &p.src_used {
            let pkt = AclPkt { src: src.clone(), dst: ip.clone(), proto: "tcp".into(), dport: Some(port) };
            if !S::acl_eval(st, &tgt, ac, &pkt) {
                return CmdResult::delayed(format!("{}\n% Connection refused by remote host", head), 300);
            }
        }
    }
    if !line.login_local && line.password.is_none() {
        return out(format!("{}Open\n\nPassword required, but none set\n\n[Connection to {} closed by foreign host]", head, host));
    }
    if proto == "ssh" {
        ask_pw(s, tgt, host.to_string(), proto, user, 0);
        return ok();
    }
    if !line.login_local {
        ask_pw(s, tgt, host.to_string(), proto, None, 0);
        return out(format!("{}Open\n\n\nUser Access Verification\n", head));
    }
    let host2 = host.to_string();
    s.ask("Username: ", false, move |s, _st, u| {
        ask_pw(s, tgt, host2, proto, Some(u.to_string()), 0);
        ok()
    });
    out(format!("{}Open\n\n\nUser Access Verification\n", head))
}

fn ask_pw(s: &mut IosSession, tgt: String, host: String, proto: &'static str, u: Option<String>, tries: u32) {
    s.ask("Password: ", true, move |s, st, pw| {
        let c = st.dev(&tgt).cfg();
        let line = &c.lines.vty0_4;
        let uu = u.as_ref().and_then(|n| c.users.get(n));
        let good = if line.login_local { uu.map(|x| x.plain == pw).unwrap_or(false) } else { line.password.as_deref() == Some(pw) };
        if !good {
            let tries = tries + 1;
            let tip = match (uu, &u) {
                (Some(x), Some(n)) if x.by_game => format!("\n💡 Lösenordet för {} är {} (står på lappen vid laptopen).", n, x.plain),
                (Some(x), Some(n)) => format!("\n💡 Det är lösenordet som sattes med username {} ... secret: {}", n, x.plain),
                _ => String::new(),
            };
            if tries >= 3 || proto == "telnet" {
                let tel = proto == "telnet";
                return out(format!(
                    "% {}{}{}",
                    if tel { "Login invalid" } else { "Authentication failed." },
                    tip,
                    if tel { format!("\n\n[Connection to {} closed by foreign host]", host) } else { String::new() }
                ));
            }
            let msg = format!("% Authentication failed.{}", tip);
            ask_pw(s, tgt, host, proto, u, tries);
            return out(msg);
        }
        let privileged = uu.map(|x| x.priv_ == 15).unwrap_or(false) || line.priv_level == Some(15);
        CmdResult { push: Some(PushReq::Ios { dev: tgt.clone(), via: proto.to_string(), privileged, user: u.clone() }), ..Default::default() }
    });
}

fn exec_cmds(reg: &mut Reg) {
    // debug
    for (k, msg) in DEBUGS {
        reg.d("exec", &format!("debug {}", k), move |s, st, _| {
            st.dev_mut(&s.dev).rt.debug.insert(k.to_string(), true);
            out(msg)
        });
        let off = move |s: &mut IosSession, st: &mut State, _a: &Args| {
            st.dev_mut(&s.dev).rt.debug.shift_remove(k);
            out(msg.replace(" is on", " is off"))
        };
        reg.d("exec", &format!("undebug {}", k), off);
        reg.d("exec", &format!("no debug {}", k), off);
    }
    let all_off = |s: &mut IosSession, st: &mut State, _a: &Args| {
        st.dev_mut(&s.dev).rt.debug.clear();
        out("All possible debugging has been turned off")
    };
    reg.d("exec", "undebug all", all_off);
    reg.d("exec", "no debug all", all_off);
    reg.d("exec", "debug all", |_, _, _| out("This may severely impact network performance. Continue? (yes/[no]): \n% Nej – inte på en produktionsenhet mitt på dagen."));
    reg.show(
        "debugging",
        |s, st, _| {
            let on: Vec<String> = st.dev(&s.dev).rt.debug.keys().cloned().collect();
            if on.is_empty() {
                return ok();
            }
            let mut groups: Vec<(String, Vec<String>)> = vec![];
            for k in on {
                let g = k.split(' ').next().unwrap_or("").to_uppercase();
                let msg = DEBUGS.iter().find(|d| d.0 == k).map(|d| d.1).unwrap_or("");
                let line = format!("  {}", msg);
                match groups.iter_mut().find(|x| x.0 == g) {
                    Some(x) => x.1.push(line),
                    None => groups.push((g, vec![line])),
                }
            }
            out(groups.iter().map(|(g, l)| format!("{}:\n{}", if g == "IP" { "Generic IP" } else { g }, l.join("\n"))).collect::<Vec<_>>().join("\n"))
        },
        false,
    );

    // klocka
    reg.d("exec", "clock set <word> <n:1-31> <word> <n:1993-2035>", |s, st, a| set_clock(s, st, a, 2, 1));
    // v = [tid, månad, dag, år]
    reg.d("exec", "clock set <word> <word> <n:1-31> <n:1993-2035>", |s, st, a| {
        let hms = parse_hms(a.s(0));
        let (h, m, sec) = match hms {
            Some(x) => x,
            None => return out(s.invalid_marker(st, &a.line, a.s(0))),
        };
        let mon = match month_idx(a.s(1)) {
            Some(x) => x,
            None => return out(s.invalid_marker(st, &a.line, a.s(1))),
        };
        let ms = U::date_utc(a.n(3), mon, a.n(2) as u32, h, m, sec);
        let at = st.time;
        st.dev_mut(&s.dev).rt.clock_set = Some(ClockSet { ms, at });
        ok()
    });
    reg.d("exec", "erase startup-config", erase_startup);
    reg.d("exec", "erase nvram:", erase_startup);
    reg.d("exec", "write erase", erase_startup);
    reg.d(
        "exec",
        "delete flash:vlan.dat",
        sw_only(|s, _st, _a| {
            s.ask("Delete filename [vlan.dat]? ", false, |s, _st, _| {
                s.ask("Delete flash:/vlan.dat? [confirm]", false, |s, st, a| {
                    if !a.is_empty() && !a.to_lowercase().starts_with('y') {
                        return ok();
                    }
                    st.dev_mut(&s.dev).rt.vlan_dat_deleted = true;
                    ok()
                });
                ok()
            });
            ok()
        }),
    );
    reg.d("exec", "reload in <n:1-1440>", |s, _st, a| {
        let n = a.n(0);
        s.ask("Proceed with reload? [confirm]", false, move |s, st, ans| {
            if !ans.is_empty() && !ans.to_lowercase().starts_with('y') {
                return ok();
            }
            let at = st.time + n as f64 * 60.0;
            st.dev_mut(&s.dev).rt.reload_at = Some(at);
            out(format!("Reload scheduled in {} minutes by console\nReload reason: Reload Command", n))
        });
        ok()
    });
    reg.d("exec", "reload cancel", |s, st, _| {
        let rt = &mut st.dev_mut(&s.dev).rt;
        if rt.reload_at.is_none() {
            return out("%No reload is scheduled.");
        }
        rt.reload_at = None;
        out("\n\n***\n*** --- SHUTDOWN ABORTED ---\n***")
    });
    reg.show(
        "reload",
        |s, st, _| match st.dev(&s.dev).rt.reload_at {
            None => out("No reload is scheduled."),
            Some(at) => {
                let left = ((at - st.time) / 60.0).round().max(0.0) as i64;
                out(format!("Reload scheduled in {} minutes by console\nReload reason: Reload Command", left))
            }
        },
        false,
    );
    reg.d("exec", "copy running-config tftp:", tftp_copy);
    reg.d("exec", "copy running-config tftp", tftp_copy);
    reg.d("exec", "copy startup-config tftp:", tftp_copy);

    // ssh och telnet från enheten
    reg.d("user exec", "ssh -l <word> <iphost>", |s, st, a| remote_login(s, st, a.s(1), Some(a.s(0).to_string()), "ssh"));
    reg.d("user exec", "ssh -v <n:1-2> -l <word> <iphost>", |s, st, a| remote_login(s, st, a.s(2), Some(a.s(1).to_string()), "ssh"));
    reg.d("user exec", "telnet <iphost>", |s, st, a| remote_login(s, st, a.s(0), None, "telnet"));
    reg.d("user exec", "connect <iphost>", |s, st, a| remote_login(s, st, a.s(0), None, "telnet"));
    reg.d("user exec", "terminal width <n:0-512>", |_, _, _| ok());
}

// ------------------------------------------------------------------ Show-kommandon

fn flash_list(s: &IosSession, st: &State) -> String {
    let sw = s.is_switch(st);
    let mut files: Vec<(&str, i64, &str)> = if sw {
        vec![
            ("c3560-ipservicesk9-mz.122-55.SE12.bin", 16013384, "Mar 1 1993 00:04:56 +00:00"),
            ("vlan.dat", 1916, "Sep 29 2026 07:58:12 +02:00"),
            ("config.text", 3401, "Sep 28 2026 16:22:40 +02:00"),
            ("private-config.text", 1245, "Sep 28 2026 16:22:40 +02:00"),
        ]
    } else {
        vec![("c2900-universalk9-mz.SPA.152-4.M11.bin", 97794040, "Jun 11 2019 14:38:40 +00:00"), ("pnp-tech-time", 3, "Sep 28 2026 16:22:40 +00:00")]
    };
    if sw && st.dev(&s.dev).rt.vlan_dat_deleted {
        files.retain(|f| f.0 != "vlan.dat");
    }
    let total: i64 = if sw { 32514048 } else { 255744000 };
    let used: i64 = files.iter().map(|f| f.1).sum();
    let mut o = vec!["Directory of flash:/".to_string(), String::new()];
    for (i, f) in files.iter().enumerate() {
        o.push(format!("{}  -rwx{}  {}  {}", rp(i + 2, 6), rp(f.1, 11), f.2, f.0));
    }
    o.push(String::new());
    o.push(format!("{} bytes total ({} bytes free)", total, total - used));
    o.join("\n")
}

fn counters(s: &IosSession, st: &mut State, only: Option<&str>) -> String {
    let d = S::get(st);
    let names: Vec<String> = match only {
        Some(o) => vec![o.to_string()],
        None => SH::phys_ports(st.dev(&s.dev)),
    };
    let dev = s.dev.clone();
    let time = st.time;
    let num = |n: &str, salt: &str, big: bool| -> i64 {
        match d.ports.get(&S::key(&dev, n)) {
            Some(p) if p.up => {
                let h = U::hash(&format!("{}{}{}", n, salt, dev)) as i64;
                (h % if big { 90000000 } else { 900000 }) + if big { 1000000 } else { 1000 } + (time * if big { 1500.0 } else { 12.0 }).floor() as i64
            }
            _ => 0,
        }
    };
    let mut o = vec![String::new(), "Port            InOctets    InUcastPkts    InMcastPkts    InBcastPkts".to_string()];
    for n in &names {
        o.push(format!(
            "{}{}{}{}{}",
            lp(U::short_if(n), 11),
            rp(num(n, "io", true), 13),
            rp(num(n, "iu", false), 15),
            rp(num(n, "im", false) / 40, 15),
            rp(num(n, "ib", false) / 90, 15)
        ));
    }
    o.push(String::new());
    o.push("Port           OutOctets   OutUcastPkts   OutMcastPkts   OutBcastPkts".to_string());
    for n in &names {
        o.push(format!(
            "{}{}{}{}{}",
            lp(U::short_if(n), 11),
            rp(num(n, "oo", true), 13),
            rp(num(n, "ou", false), 15),
            rp(num(n, "om", false) / 30, 15),
            rp(num(n, "ob", false) / 70, 15)
        ));
    }
    o.join("\n")
}

/// Alla interface utom interna (iface_names + filter)
fn visible_ifaces(s: &IosSession, st: &State) -> Vec<String> {
    SH::iface_names(st.dev(&s.dev)).into_iter().filter(|n| !cfg_r(s, st).ifaces.get(n).map(|i| i.internal).unwrap_or(false)).collect()
}

fn tdr_for(s: &IosSession, st: &mut State, n: &str) -> Tdr {
    let d = S::get(st);
    let p = d.ports.get(&S::key(&s.dev, n));
    let link = st.links.iter().find(|l| (l.a.dev == s.dev && l.a.port == n) || (l.b.dev == s.dev && l.b.port == n));
    let link = match link {
        Some(l) => l,
        None => return Tdr { open: true, len: 0, ..Default::default() },
    };
    let len = 3 + (U::hash(&link.id) % 45);
    if link.state == LinkState::Broken {
        let at = 1 + (U::hash(&format!("{}x", link.id)) % (len.saturating_sub(1)).max(2));
        return Tdr { open: true, len: at, ..Default::default() };
    }
    if link.kind == LinkKind::Fiber {
        return Tdr { fiber: true, ..Default::default() };
    }
    if p.and_then(|p| p.neg.as_ref()).map(|n| n.speed == 100).unwrap_or(false) {
        return Tdr { len, half: true, ..Default::default() };
    }
    Tdr { len, ..Default::default() }
}

fn show_cmds(reg: &mut Reg) {
    reg.show("privilege", |s, _st, _| out(format!("Current privilege level is {}", if s.mode == "user" { 1 } else { 15 })), true);
    reg.show("sessions", |_, _, _| out("% No connections open"), true);
    reg.show(
        "terminal",
        |s, st, _| {
            let sp = cfg_r(s, st).lines.con.speed;
            let sp = if sp == 0 { 9600 } else { sp };
            out(format!(
                "Line 0, Location: \"\", Type: \"\"\nLength: 24 lines, Width: 80 columns\nBaud rate (TX/RX) is {sp}/{sp}, no parity, 2 stopbits, 8 databits\nStatus: PSI Enabled, Ready, Active, No Exit Banner, Automore On\nCapabilities: none\nModem state: Ready\nSpecial Chars: Escape  Hold  Stop  Start  Disconnect  Activation\n                ^^x    none   -     -       none\nTimeouts:      Idle EXEC    Idle Session   Modem Answer  Session   Dispatch\n               00:10:00        never                        none     not set\nHistory is enabled, history size is 20.\nTransport protocol preferences: none"
            ))
        },
        true,
    );
    reg.show(
        "line",
        |s, st, _| {
            let router = s.is_router(st);
            let c = cfg_r(s, st);
            let mut rows = vec!["   Tty Typ     Tx/Rx    A Modem  Roty AccO AccI   Uses   Noise  Overruns   Int".to_string()];
            rows.push("*    0 CTY              -    -      -    -    -      1       0     0/0       -".into());
            if router {
                rows.push("     1 AUX   9600/9600  -    -      -    -    -      0       0     0/0       -".into());
            }
            for n in 0..=15 {
                let l = if n <= 4 { &c.lines.vty0_4 } else { &c.lines.vty5_15 };
                let tty = if router { n + 2 } else { n + 1 };
                rows.push(format!(
                    "{} VTY              -    -      -    -  {} {}       0     0/0       -",
                    rp(tty, 6),
                    lp(l.access_class.clone().unwrap_or("-".into()), 4),
                    rp(if n == 0 { 3 } else { 0 }, 6)
                ));
            }
            out(rows.join("\n"))
        },
        true,
    );
    reg.show(
        "hosts",
        |s, st, _| {
            let c = cfg_r(s, st);
            let mut o = vec![
                format!("Default domain is {}", c.domain_name.clone().unwrap_or("not set".into())),
                format!("Name/address lookup uses {}", if c.domain_lookup { "domain service" } else { "static mappings" }),
                format!("Name servers are {}", if c.name_servers.is_empty() { "255.255.255.255".to_string() } else { c.name_servers.join(", ") }),
                String::new(),
                "Codes: UN - unknown, EX - expired, OK - OK, ?? - revalidate".into(),
                "       temp - temporary, perm - permanent".into(),
                "       NA - Not Applicable None - Not defined".into(),
                String::new(),
                "Host                      Port  Flags      Age Type   Address(es)".into(),
            ];
            for (h, ip) in &c.hosts {
                o.push(format!("{}None  (perm, OK)  0   IP     {}", lp(h, 26), ip));
            }
            out(o.join("\n"))
        },
        true,
    );
    reg.show("flash", |s, st, _| out(flash_list(s, st)), true);
    reg.show("flash:", |s, st, _| out(flash_list(s, st)), true);
    reg.d("user exec", "dir", |s, st, _| out(flash_list(s, st)));
    reg.d("user exec", "dir flash:", |s, st, _| out(flash_list(s, st)));
    reg.d("user exec", "dir nvram:", |s, st, _| {
        let startup = st.dev(&s.dev).startup.clone();
        let len = match &startup {
            Some(c) => SH::running_config_of(st, &s.dev.clone(), c).chars().count() as i64,
            None => 0,
        };
        let body = if startup.is_some() { format!("    1  -rw-{}                    <no date>  startup-config", rp(len, 12)) } else { "  No files in directory".to_string() };
        out(format!("Directory of nvram:/\n\n  {}\n\n524288 bytes total ({} bytes free)", body, 524288 - len))
    });
    reg.show(
        "boot",
        sw_only(|_, _, _| out("BOOT path-list      : flash:/c3560-ipservicesk9-mz.122-55.SE12.bin\nConfig file         : flash:/config.text\nPrivate Config file : flash:/private-config.text\nEnable Break        : no\nManual Boot         : no\nHELPER path-list    :\nAuto upgrade        : yes\nNVRAM/Config file\n      buffer size:   524288")),
        true,
    );
    // JS definierar show license två gånger (router, sedan switch, den senare skriver över); här väljs text efter enhetstyp.
    reg.show(
        "license",
        |s, st, _| {
            if s.is_router(st) {
                out("Index 1 Feature: ipbasek9\n        Period left: Life time\n        License Type: Permanent\n        License State: Active, In Use\n        License Count: Non-Counted\n        License Priority: Medium\nIndex 2 Feature: securityk9\n        Period left: Life time\n        License Type: Permanent\n        License State: Active, In Use\n        License Count: Non-Counted\n        License Priority: Medium\nIndex 3 Feature: datak9\n        Period left: Not Activated\n        Period Used: 0  minute  0  second\n        License Type: EvalRightToUse\n        License State: Not in Use, EULA not accepted\n        License Count: Non-Counted\n        License Priority: None")
            } else {
                out("Index 1 Feature: lanbasek9\n        Period left: Life time\n        License Type: Permanent\n        License State: Active, In Use\n        License Priority: Medium\n        License Count: Non-Counted")
            }
        },
        false,
    );
    reg.show(
        "env all",
        sw_only(|_, _, _| out("FAN is OK\nTEMPERATURE is OK\nTemperature Value: 37 Degree Celsius\nTemperature State: GREEN\nYellow Threshold : 65 Degree Celsius\nRed Threshold    : 75 Degree Celsius\nPOWER is OK\nRPS is NOT PRESENT")),
        false,
    );
    reg.show("environment all", sw_only(|_, _, _| out("FAN is OK\nTEMPERATURE is OK\nPOWER is OK\nRPS is NOT PRESENT")), false);
    reg.show(
        "processes memory",
        |s, st, _| {
            let r = s.is_router(st);
            out(format!(
                "Processor Pool Total:  {}\n      I/O Pool Total:   {}",
                if r { "414285504 Used:   98714424 Free:  315571080" } else { "94363276 Used:   27163948 Free:   67199328" },
                if r { "75497472 Used:   21011032 Free:   54486440" } else { "8388608 Used:    3207548 Free:    5181060" }
            ))
        },
        true,
    );
    reg.show(
        "vtp status",
        sw_only(|s, st, _| {
            let mac = U::mac_dots(&st.dev(&s.dev).base_mac);
            let c = cfg(s, st);
            let vl = c.vlans.len() + 4;
            let t = vtp(c).clone();
            let mode = {
                let mut ch = t.mode.chars();
                match ch.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
                    None => String::new(),
                }
            };
            out(format!(
                "VTP Version capable             : 1 to 3\nVTP version running             : {}\nVTP Domain Name                 : {}\nVTP Pruning Mode                : Disabled\nVTP Traps Generation            : Disabled\nDevice ID                       : {}\nConfiguration last modified by 0.0.0.0 at 3-1-93 00:02:11\n{}\nFeature VLAN:\n--------------\nVTP Operating Mode                : {}\nMaximum VLANs supported locally   : 1005\nNumber of existing VLANs          : {}\nConfiguration Revision            : {}\nMD5 digest                        : 0x3F 0x37 0x45 0x9A 0x37 0x53 0xA6 0xDE\n                                    0xF9 0x1A 0x2B 0x5C 0x81 0x0E 0x4D 0x77",
                t.version,
                t.domain,
                mac,
                if t.mode == "server" { "Local updater ID is 0.0.0.0 (no valid interface found)\n" } else { "" },
                mode,
                vl,
                if t.mode == "transparent" || t.mode == "off" { 0 } else { 7 }
            ))
        }),
        true,
    );
    reg.show(
        "vtp password",
        sw_only(|s, st, _| match vtp(cfg(s, st)).password.clone() {
            Some(p) => out(format!("VTP Password: {}", p)),
            None => out("The VTP password is not configured."),
        }),
        false,
    );
    reg.show(
        "vlan summary",
        sw_only(|s, st, _| {
            let n = cfg_r(s, st).vlans.len() + 4;
            out(format!("Number of existing VLANs              : {n}\nNumber of existing VTP VLANs          : {n}\nNumber of existing extended VLANS     : 0"))
        }),
        true,
    );
    reg.show("ip default-gateway", sw_only(|s, st, _| out(cfg_r(s, st).default_gateway.clone().unwrap_or("0.0.0.0".into()))), true);
    reg.show(
        "interfaces switchport",
        sw_only(|s, st, _| {
            let names = SH::phys_ports(st.dev(&s.dev));
            let dev = s.dev.clone();
            out(names.iter().map(|n| SH::switchport(st, &dev, n)).collect::<Vec<_>>().join("\n\n"))
        }),
        true,
    );
    reg.show(
        "cdp",
        |s, st, _| {
            if cfg_r(s, st).cdp_off {
                return out("% CDP is not enabled");
            }
            out("Global CDP information:\n\tSending CDP packets every 60 seconds\n\tSending a holdtime value of 180 seconds\n\tSending CDPv2 advertisements is  enabled")
        },
        true,
    );
    reg.show(
        "cdp interface",
        |s, st, _| {
            if cfg_r(s, st).cdp_off {
                return out("% CDP is not enabled");
            }
            let d = S::get(st);
            let o: Vec<String> = cfg_r(s, st)
                .ifaces
                .keys()
                .filter_map(|n| {
                    let p = d.ports.get(&S::key(&s.dev, n))?;
                    let ud = if p.up { "up" } else { "down" };
                    Some(format!("{n} is {ud}, line protocol is {ud}\n  Encapsulation ARPA\n  Sending CDP packets every 60 seconds\n  Holdtime is 180 seconds"))
                })
                .collect();
            out(o.join("\n"))
        },
        true,
    );
    reg.show(
        "lldp",
        |s, st, _| {
            if !cfg_r(s, st).lldp {
                return out("% LLDP is not enabled");
            }
            out("\nGlobal LLDP Information:\n    Status: ACTIVE\n    LLDP advertisements are sent every 30 seconds\n    LLDP hold time advertised is 120 seconds\n    LLDP interface reinitialisation delay is 2 seconds")
        },
        true,
    );
    reg.show(
        "lldp neighbors",
        |s, st, _| {
            if !cfg_r(s, st).lldp {
                return out("% LLDP is not enabled");
            }
            let d = S::get(st);
            let mut rows = vec![];
            for n in cfg_r(s, st).ifaces.keys() {
                let p = match d.ports.get(&S::key(&s.dev, n)) {
                    Some(p) if p.up => p,
                    _ => continue,
                };
                let peer = match st.devices.get(&p.peer.dev) {
                    Some(x) if x.os == Os::Ios && x.config.as_ref().map(|c| c.lldp).unwrap_or(false) => x,
                    _ => continue,
                };
                let host: String = peer.cfg().hostname.chars().take(20).collect();
                rows.push(format!(
                    "{}{}{}{}{}",
                    lp(host, 21),
                    lp(U::short_if(n), 16),
                    lp("120", 11),
                    lp(if peer.kind == Kind::Router { "R" } else { "B" }, 16),
                    U::short_if(&p.peer.port)
                ));
            }
            out(format!("Capability codes:\n    (R) Router, (B) Bridge, (T) Telephone, (C) DOCSIS Cable Device\n    (W) WLAN Access Point, (P) Repeater, (S) Station, (O) Other\n\nDevice ID           Local Intf     Hold-time  Capability      Port ID\n{}\n\nTotal entries displayed: {}", rows.join("\n"), rows.len()))
        },
        true,
    );
    reg.show(
        "spanning-tree root",
        sw_only(|s, st, _| {
            let d = S::get(st);
            let c = cfg_r(s, st);
            let mut o = vec![
                "                                        Root    Hello Max Fwd".to_string(),
                "Vlan                   Root ID          Cost    Time  Age Dly  Root Port".into(),
                "---------------- -------------------- --------- ----- --- ---  ------------".into(),
            ];
            for v in c.vlans.keys() {
                if c.stp_off.contains(v) {
                    continue;
                }
                let info = d.stp.get(&s.dev).and_then(|m| m.get(v));
                let root_dev = info.and_then(|i| st.devices.get(&i.root)).unwrap_or_else(|| st.dev(&s.dev));
                let rb = S::bridge_id(root_dev, *v);
                let is_root = info.map(|i| i.is_root).unwrap_or(true);
                let rport = if is_root { String::new() } else { info.and_then(|i| i.root_port.clone()).map(|p| U::short_if(&p)).unwrap_or_default() };
                o.push(format!(
                    "{}{}{}{}{}{}  {}",
                    lp(format!("VLAN{:04}", v), 17),
                    lp(format!("{} {}", rb.pr, U::mac_dots(&root_dev.base_mac)), 21),
                    rp(if is_root { 0 } else { 4 }, 9),
                    rp(2, 6),
                    rp(20, 4),
                    rp(15, 4),
                    rport
                ));
            }
            out(o.join("\n"))
        }),
        true,
    );
    reg.show(
        "spanning-tree interface <if>",
        sw_only(|s, st, a| {
            let n = a.s(0).to_string();
            let short = U::short_if(&n);
            let all = SH::spanning_tree(st, &s.dev.clone(), None);
            let mut cur: Option<String> = None;
            let mut rows = vec![];
            for l in all.split('\n') {
                if l.len() == 8 && l.starts_with("VLAN") && l[4..].bytes().all(|b| b.is_ascii_digit()) {
                    cur = Some(l.to_string());
                    continue;
                }
                if let Some(c) = &cur {
                    if l.starts_with(&format!("{} ", short)) {
                        rows.push(format!("{}{}", lp(c, 17), l.chars().skip(20).collect::<String>()));
                    }
                }
            }
            if rows.is_empty() {
                return out(format!("no spanning tree info available for {}", n));
            }
            out(format!("Vlan             Role Sts Cost      Prio.Nbr Type\n---------------- ---- --- --------- -------- --------------------------------\n{}", rows.join("\n")))
        }),
        true,
    );
    reg.show("spanning-tree detail", sw_only(|s, st, _| out(SH::spanning_tree(st, &s.dev.clone(), None))), true);
    reg.show(
        "etherchannel summary",
        sw_only(|s, st, _| {
            let d = S::get(st);
            let c = cfg_r(s, st);
            let mut groups: BTreeMap<i64, Vec<(String, ChGroup)>> = BTreeMap::new();
            for (n, i) in &c.ifaces {
                if let Some(g) = &i.ch_group {
                    groups.entry(g.n).or_default().push((n.clone(), g.clone()));
                }
            }
            let mut o: Vec<String> = [
                "Flags:  D - down        P - bundled in port-channel",
                "        I - stand-alone s - suspended",
                "        H - Hot-standby (LACP only)",
                "        R - Layer3      S - Layer2",
                "        U - in use      f - failed to allocate aggregator",
                "",
                "        M - not in use, minimum links not met",
                "        u - unsuitable for bundling",
                "        w - waiting to be aggregated",
                "        d - default port",
                "",
                "",
            ]
            .iter()
            .map(|x| x.to_string())
            .collect();
            o.push(format!("Number of channel-groups in use: {}", groups.len()));
            o.push(format!("Number of aggregators:           {}", groups.len()));
            o.push(String::new());
            o.push("Group  Port-channel  Protocol    Ports".into());
            o.push("------+-------------+-----------+-----------------------------------------------".into());
            for (g, mem) in &groups {
                let mode = mem[0].1.mode.clone();
                let proto = if mode == "active" || mode == "passive" {
                    "LACP"
                } else if mode == "on" {
                    "-"
                } else {
                    "PAgP"
                };
                let mut any_up = false;
                let ports: Vec<String> = mem
                    .iter()
                    .map(|(n, _)| {
                        let p = d.ports.get(&S::key(&s.dev, n));
                        let peer = p.filter(|p| p.up).and_then(|p| st.devices.get(&p.peer.dev));
                        let pi = peer.and_then(|pd| pd.config.as_ref()).and_then(|pc| p.and_then(|p| pc.ifaces.get(&p.peer.port)));
                        let good = match (p, pi.and_then(|x| x.ch_group.as_ref())) {
                            (Some(p), Some(pg)) if p.up => {
                                if mode == "on" {
                                    pg.mode == "on"
                                } else {
                                    pg.mode != "on" && !(mode == "passive" && pg.mode == "passive") && !(mode == "auto" && pg.mode == "auto")
                                }
                            }
                            _ => false,
                        };
                        if good {
                            any_up = true;
                        }
                        let flag = if !p.map(|p| p.up).unwrap_or(false) {
                            "D"
                        } else if good {
                            "P"
                        } else if mode == "on" {
                            "D"
                        } else {
                            "I"
                        };
                        format!("{}({})", U::short_if(n), flag)
                    })
                    .collect();
                o.push(format!("{}{}{}{}", lp(g, 7), lp(format!("Po{}(S{})", g, if any_up { "U" } else { "D" }), 14), lp(proto, 12), ports.join(" ")));
            }
            out(o.join("\n"))
        }),
        true,
    );
    reg.show(
        "ip dhcp server statistics",
        rt_only(|s, st, _| {
            let n = SH::dhcp_binding(st, &s.dev.clone()).matches("Automatic").count();
            let pools = cfg_r(s, st).dhcp_pools.len();
            out(format!(
                "Memory usage         {}\nAddress pools        {}\nDatabase agents      0\nAutomatic bindings   {}\nManual bindings      0\nExpired bindings     0\nMalformed messages   0\nSecure arp entries   0\nRenew messages       0\nWorkspace timeouts   0\nStatic routes        0\nRelay bindings       0\nRelay bindings active        0\nRelay bindings terminated    0\nRelay bindings selecting     0\n\nMessage              Received\nBOOTREQUEST          0\nDHCPDISCOVER         {}\nDHCPREQUEST          {}\nDHCPDECLINE          0\nDHCPRELEASE          0\nDHCPINFORM           0\n\nMessage              Sent\nBOOTREPLY            0\nDHCPOFFER            {}\nDHCPACK              {}\nDHCPNAK              0",
                24000 + n * 312,
                pools,
                n,
                n + 2,
                n * 2 + 1,
                n + 2,
                n * 2 + 1
            ))
        }),
        false,
    );
    reg.show(
        "ip nat translations verbose",
        rt_only(|s, st, _| {
            let base = SH::nat_translations(st, &s.dev.clone());
            let o: Vec<String> = base
                .split('\n')
                .enumerate()
                .map(|(i, l)| {
                    if i == 0 {
                        return l.to_string();
                    }
                    let stat = l.starts_with("---");
                    format!(
                        "{}\n    create 00:0{}:1{}, use 00:00:0{} timeout:{}, left 23:5{}:12,\n    flags: \n{}, use_count: 0, entry-id: {}, lc_entries: 0",
                        l,
                        i % 6,
                        i,
                        i % 9,
                        if stat { "0" } else { "86400000" },
                        i % 9,
                        if stat { "static" } else { "extended" },
                        10 + i
                    )
                })
                .collect();
            out(o.join("\n"))
        }),
        false,
    );
    reg.show(
        "ip route summary",
        |s, st, _| {
            let d = S::get(st);
            let r = S::installed_routes(st, &d, &s.dev);
            let cnt = |t: &str| r.iter().filter(|x| x.kind == t || (t == "S" && x.kind == "S*")).count();
            let cl = cnt("C") + cnt("L");
            let cs = cnt("S");
            out(format!(
                "IP routing table name is default (0x0)\nIP routing table maximum-paths is 32\nRoute Source    Networks    Subnets     Replicates  Overhead    Memory (bytes)\nconnected       0           {}0           {}{}\nstatic          {}0           0           {}{}\ninternal        3                                               1380\nTotal           {}{}0           {}{}",
                lp(cl, 12),
                lp(cl * 72, 12),
                cl * 180,
                lp(cs, 12),
                lp(cs * 72, 12),
                cs * 180,
                lp(cs + 3, 12),
                lp(cl, 12),
                lp((cl + cs) * 72, 12),
                (cl + cs) * 180 + 1380
            ))
        },
        false,
    );
    reg.show(
        "crypto key mypubkey rsa",
        |s, st, _| {
            let c = cfg_r(s, st);
            let bits = match c.crypto_key {
                Some(b) => b,
                None => return ok(),
            };
            let h = format!("{:X}", U::hash(&format!("{}{}", c.hostname, st.dev(&s.dev).base_mac)));
            let hhh = format!("{h}{h}{h}");
            let a20: String = hhh.chars().take(20).collect();
            let b8: String = format!("C0FFEE{h}").chars().take(8).collect();
            out(format!(
                "% Key pair was generated at: 08:00:00 CEST Sep 28 2026\nKey name: {}.{}\nKey type: RSA KEYS\n Storage Device: private-config\n Usage: General Purpose Key\n Key is not exportable.\n Key Data:\n  30820122 300D0609 2A864886 F70D0101 01050003 82010F00 3082010A 02820101\n  00{} {} 5A2E1B03 8D7C44F2 A91B0E6D 2F04C319 83B56E21\n  ...  ({} bits)",
                c.hostname,
                c.domain_name.clone().unwrap_or_default(),
                a20,
                b8,
                bits
            ))
        },
        false,
    );
    reg.show(
        "snmp",
        |s, st, _| {
            let sn = match &cfg_r(s, st).snmp {
                Some(x) if !x.communities.is_empty() => x.clone(),
                _ => return out("%SNMP agent not enabled"),
            };
            out(format!(
                "Chassis: FOC1133Z0R4\n{}{}0 SNMP packets input\n    0 Bad SNMP version errors\n    0 Unknown community name\n0 SNMP packets output\nSNMP logging: disabled",
                sn.contact.map(|c| format!("Contact: {}\n", c)).unwrap_or_default(),
                sn.location.map(|c| format!("Location: {}\n", c)).unwrap_or_default()
            ))
        },
        false,
    );
    reg.show(
        "snmp community",
        |s, st, _| match &cfg_r(s, st).snmp {
            None => ok(),
            Some(sn) => out(sn
                .communities
                .keys()
                .map(|k| format!("\nCommunity name: {k}\nCommunity Index: {k}\nCommunity SecurityName: {k}\nstorage-type: nonvolatile\t active\n"))
                .collect::<String>()),
        },
        false,
    );
    reg.show(
        "standby brief",
        rt_only(|s, st, _| {
            let c = cfg_r(s, st);
            let mut o = vec![
                "                     P indicates configured to preempt.".to_string(),
                "                     |".into(),
                "Interface   Grp  Pri P State   Active          Standby         Virtual IP".into(),
            ];
            for (n, i) in &c.ifaces {
                let h = match &i.hsrp {
                    Some(h) if h.ip.is_some() => h,
                    _ => continue,
                };
                o.push(format!(
                    "{}{}{}{}{}{}{}{}",
                    lp(U::short_if(n), 12),
                    lp(h.grp, 5),
                    lp(h.pri, 4),
                    lp(if h.preempt { "P" } else { " " }, 2),
                    lp("Active", 8),
                    lp("local", 16),
                    lp("unknown", 16),
                    h.ip.clone().unwrap_or_default()
                ));
            }
            out(o.join("\n"))
        }),
        false,
    );
    reg.show("interfaces counters", |s, st, _| out(counters(s, st, None)), true);
    reg.show("interfaces <if> counters", |s, st, a| out(counters(s, st, Some(a.s(0)))), true);
    reg.show(
        "mac address-table count",
        sw_only(|s, st, _| {
            let t = SH::mac_table(st, &s.dev.clone(), SH::MacFilter::default());
            let dy = t.matches("DYNAMIC").count();
            let sta = t.matches("STATIC").count();
            out(format!(
                "\nMac Entries for Vlan   : All\n---------------------------\nDynamic Address Count  : {}\nStatic  Address Count  : {}\nTotal Mac Addresses    : {}\n\nTotal Mac Address Space Available: {}",
                dy,
                sta,
                dy + sta,
                12288 - dy as i64 - sta as i64
            ))
        }),
        true,
    );
    reg.show(
        "errdisable recovery",
        |s, st, _| {
            let c = cfg_r(s, st);
            let causes = ["arp-inspection", "bpduguard", "channel-misconfig", "dhcp-rate-limit", "link-flap", "loopback", "psecure-violation", "storm-control", "udld"];
            let mut o = vec!["ErrDisable Reason            Timer Status".to_string(), "-----------------            --------------".into()];
            for k in causes {
                let on = if k == "psecure-violation" { c.err_recovery } else { c.x.contains_key(&format!("errdisable cause {}", k)) };
                o.push(format!("{}{}", lp(k, 29), if on { "Enabled" } else { "Disabled" }));
            }
            let iv = c.x.get("errdisable interval").and_then(|x| x.split(' ').last()).unwrap_or("300").to_string();
            o.push(String::new());
            o.push(format!("Timer interval: {} seconds", iv));
            o.push(String::new());
            o.push("Interfaces that will be enabled at the next timeout:".into());
            out(o.join("\n"))
        },
        true,
    );
    reg.show(
        "ip dhcp snooping",
        sw_only(|s, st, _| {
            let c = cfg_r(s, st);
            if !c.x.contains_key("dhcp snooping") {
                return out("Switch DHCP snooping is disabled");
            }
            let vl = c.x.get("dhcp snooping vlan").and_then(|x| x.split(' ').last()).unwrap_or("").to_string();
            let mut rows = vec![];
            for (n, i) in &c.ifaces {
                if i.x.contains_key("snoop trust") || i.x.contains_key("snoop rate") {
                    rows.push(format!(
                        "{}{}{}",
                        lp(n, 32),
                        lp(if i.x.contains_key("snoop trust") { "yes" } else { "no" }, 16),
                        i.x.get("snoop rate").and_then(|x| x.split(' ').last()).unwrap_or("unlimited")
                    ));
                }
            }
            let v = if vl.is_empty() { "none".to_string() } else { vl };
            out(format!(
                "Switch DHCP snooping is enabled\nDHCP snooping is configured on following VLANs:\n{v}\nDHCP snooping is operational on following VLANs:\n{v}\nInsertion of option 82 is {}\nInterface                  Trusted    Allow option    Rate limit (pps)\n-----------------------    -------    ------------    ----------------\n{}",
                if c.x.contains_key("dhcp snooping opt82") { "disabled" } else { "enabled" },
                rows.join("\n")
            ))
        }),
        true,
    );
    reg.show("archive", |_, _, _| out("There are currently 0 archive configurations saved."), false);
    reg.show(
        "ip sockets",
        |s, st, _| {
            let d = S::get(st);
            let ip = S::dev_eps(&d, &s.dev).first().and_then(|e| e.ip.clone()).unwrap_or("--any--".into());
            out(format!("Proto    Remote      Port      Local       Port  In Out  Stat TTY OutputIF\n 17 --listen--             {}    123   0   0   211   0", ip))
        },
        true,
    );
    reg.show("tcp brief", |_, _, _| out("TCB       Local Address               Foreign Address             (state)"), false);
    reg.show(
        "ip http server status",
        |_, _, _| out("HTTP server status: Disabled\nHTTP server port: 80\nHTTP server active supplementary listener ports:\nHTTP server authentication method: enable\nHTTP secure server status: Disabled"),
        false,
    );
    reg.show("aaa sessions", |_, _, _| out("% AAA is not enabled (no aaa new-model)"), false);
    // show ip ospf neighbor / interface brief och show ip protocols ersätts av ios_dyn
    reg.show(
        "ipv6 interface brief",
        |s, st, _| {
            let c = cfg_r(s, st);
            out(c
                .ifaces
                .iter()
                .filter(|(_, i)| i.x.contains_key("ipv6 "))
                .map(|(n, i)| format!("{}[up/up]\n    {}", lp(n, 23), i.x["ipv6 "].split(' ').last().unwrap_or("")))
                .collect::<Vec<_>>()
                .join("\n"))
        },
        false,
    );
}

// ------------------------------------------------------------------ Version 7
fn v7_cmds(reg: &mut Reg) {
    // VLAN: state suspend/active och shutdown
    reg.d("vlan", "state suspend", |s, st, a| {
        let ids = s.ctx.vlans.clone();
        let neg = a.neg;
        let c = cfg(s, st);
        for x in ids {
            if neg {
                c.vlan_state.remove(&x);
            } else {
                c.vlan_state.insert(x, "suspend".into());
            }
        }
        S::touch(st);
        ok()
    });
    reg.d("vlan", "state active", |s, st, _| {
        let ids = s.ctx.vlans.clone();
        let c = cfg(s, st);
        for x in ids {
            c.vlan_state.remove(&x);
        }
        S::touch(st);
        ok()
    });
    reg.d("vlan", "shutdown", |s, st, a| {
        let ids = s.ctx.vlans.clone();
        let neg = a.neg;
        let c = cfg(s, st);
        for x in ids {
            if neg {
                c.vlan_state.remove(&x);
            } else {
                c.vlan_state.insert(x, "shutdown".into());
            }
        }
        ok()
    });
    reg.show(
        "vlan name <word>",
        sw_only(|s, st, a| {
            let w = a.s(0).to_lowercase();
            let id = cfg_r(s, st).vlans.iter().find(|(_, n)| n.to_lowercase() == w).map(|(k, _)| *k);
            match id {
                None => out(format!("VLAN {} not found in current VLAN database", a.s(0))),
                Some(id) => out(SH::vlan_id(st, &s.dev.clone(), id)),
            }
        }),
        true,
    );

    // alias exec
    reg.d("config", "alias exec <word> <text>", |s, st, a| {
        let c = cfg(s, st);
        if a.neg {
            c.aliases.shift_remove(a.s(0));
            return ok();
        }
        c.aliases.insert(a.s(0).to_string(), rest(&a.line, a.s(0)));
        ok()
    });
    reg.def(
        "config",
        "alias exec <word>",
        |s, st, a| {
            cfg(s, st).aliases.shift_remove(a.s(0));
            ok()
        },
        NO_ONLY,
    );
    reg.show(
        "aliases",
        |s, st, _| {
            let a = &cfg_r(s, st).aliases;
            out(format!(
                "Exec mode aliases:\n  h                     help\n  lo                    logout\n  p                     ping\n  r                     resume\n  s                     show\n  u                     undebug\n  un                    undebug\n  w                     where{}",
                a.iter().map(|(k, v)| format!("\n  {}{}", lp(k, 22), v)).collect::<String>()
            ))
        },
        true,
    );

    // ACL: numrera om
    reg.d("config", "ip access-list resequence <word> <n:1-2147483647> <n:1-2147483647>", |s, st, a| {
        let name = a.s(0).to_string();
        let (start, step) = (a.n(1), a.n(2));
        match cfg(s, st).acls.get_mut(&name) {
            None => out(format!("% Access list {} does not exist", name)),
            Some(acl) => {
                let mut n = start;
                for r in acl.rules.iter_mut() {
                    r.seq = n;
                    n += step;
                }
                ok()
            }
        }
    });

    // Kabeltest (TDR) och SFP
    reg.d(
        "exec",
        "test cable-diagnostics tdr interface <if>",
        sw_only(|s, st, a| {
            let n = a.s(0).to_string();
            if !cfg_r(s, st).ifaces.contains_key(&n) {
                return out("% Invalid interface");
            }
            let t = tdr_for(s, st, &n);
            TDR.with(|m| m.borrow_mut().insert(format!("{}|{}", s.dev, n), t));
            out(format!(
                "TDR test started on interface {}\nA TDR test can take a few seconds to run on an interface\nUse 'show cable-diagnostics tdr' to read the TDR results.",
                U::short_if(&n)
            ))
        }),
    );
    reg.show(
        "cable-diagnostics tdr interface <if>",
        sw_only(|s, st, a| {
            let n = a.s(0).to_string();
            let t = TDR.with(|m| m.borrow().get(&format!("{}|{}", s.dev, n)).cloned());
            let t = match t {
                Some(t) => t,
                None => return out(format!("% TDR test was never issued on {}", U::short_if(&n))),
            };
            if t.fiber {
                return out(format!("% TDR test is not supported on fiber interface {}", U::short_if(&n)));
            }
            let sp = if t.half {
                "100M"
            } else if t.open {
                "auto"
            } else {
                "1000M"
            };
            let clock = SH::show_clock(st, &s.dev.clone());
            let clock = clock.strip_prefix('*').unwrap_or(&clock).to_string();
            let mut o = vec![
                format!("TDR test last run on: {}", clock),
                String::new(),
                "Interface Speed Local pair Pair length        Remote pair Pair status".into(),
                "--------- ----- ---------- ------------------ ----------- --------------------".into(),
            ];
            let pairs = ["Pair A", "Pair B", "Pair C", "Pair D"];
            let remotes = ["Pair B     ", "Pair A     ", "Pair D     ", "Pair C     "];
            let pstat = if t.open { "Open" } else { "Normal" };
            for (i, pr) in pairs.iter().enumerate() {
                let remote = if t.open { "N/A        " } else { remotes[i] };
                let head = if i == 0 { format!("{}{}", lp(U::short_if(&n), 10), lp(sp, 6)) } else { "                ".to_string() };
                o.push(format!("{}{}{} {} {}", head, lp(pr, 11), lp(format!("{}   +/- {}  meters", t.len, if t.open { 1 } else { 4 }), 19), remote, pstat));
            }
            out(o.join("\n"))
        }),
        true,
    );
    reg.show(
        "interfaces <if> transceiver",
        sw_only(|s, st, a| {
            let n = a.s(0).to_string();
            let d = S::get(st);
            let p = d.ports.get(&S::key(&s.dev, &n));
            let link = st.links.iter().find(|l| (l.a.dev == s.dev && l.a.port == n) || (l.b.dev == s.dev && l.b.port == n));
            let link = match link {
                Some(l) if l.kind == LinkKind::Fiber => l,
                _ => return out(format!("% No transceiver present on {}", n)),
            };
            let up = p.map(|p| p.up).unwrap_or(false);
            let rx = if up { format!("{:.1}", -5.1 - (U::hash(&link.id) % 30) as f64 / 10.0) } else { "-40.0".to_string() };
            out(format!(
                "If device is externally calibrated, only calibrated values are printed.\n++ : high alarm, +  : high warning, -  : low warning, -- : low alarm.\nNA or N/A: not applicable, Tx: transmit, Rx: receive.\n\n                                            Optical   Optical\n           Temperature  Voltage  Current   Tx Power  Rx Power\nPort       (Celsius)    (Volts)  (mA)      (dBm)     (dBm)\n---------  -----------  -------  --------  --------  --------\n{}{}{}{}{}{}{}",
                lp(U::short_if(&n), 11),
                lp("31.2", 13),
                lp("3.29", 9),
                lp("6.8", 10),
                lp("-5.3", 10),
                rx,
                if up { "" } else { " --" }
            ))
        }),
        true,
    );

    // Fler show- och clear-kommandon
    reg.show(
        "interfaces status err-disabled",
        sw_only(|s, st, _| {
            let all = SH::interfaces_status(st, &s.dev.clone(), None);
            let c = cfg_r(s, st);
            let rows: Vec<String> = all
                .split('\n')
                .filter(|l| l.contains("err-disabled"))
                .map(|l| {
                    let first = l.split(' ').next().unwrap_or("");
                    let ps = U::norm_if(first).and_then(|n| c.ifaces.get(&n)).map(|i| i.port_sec.is_some()).unwrap_or(false);
                    format!("{}err-disabled {}", l.chars().take(29).collect::<String>(), if ps { "psecure-violation" } else { "link-flap" })
                })
                .collect();
            out(format!("\nPort      Name               Status       Reason               Err-disabled Vlans\n{}", rows.join("\n")))
        }),
        true,
    );
    reg.show(
        "mac address-table static",
        sw_only(|s, st, _| {
            let c = cfg_r(s, st);
            let mut rows = vec![];
            for v in c.x.values() {
                // mac address-table static <mac> vlan <n> interface <if>
                let t: Vec<&str> = v.split_whitespace().collect();
                if t.len() == 8 && t[0] == "mac" && t[1] == "address-table" && t[2] == "static" && t[4] == "vlan" && t[6] == "interface" && t[5].bytes().all(|b| b.is_ascii_digit()) {
                    rows.push(format!("{}{}{}{}", lp(t[5], 5), lp(t[3].to_lowercase(), 19), lp("STATIC", 12), U::short_if(t[7])));
                }
            }
            for n in SH::phys_ports(st.dev(&s.dev)) {
                if let Some(i) = c.ifaces.get(&n) {
                    if let Some(ps) = &i.port_sec {
                        if ps.sticky {
                            for m in &ps.macs {
                                rows.push(format!("{}{}{}{}", lp(i.access_vlan, 5), lp(U::mac_dots(m), 19), lp("STATIC", 12), U::short_if(&n)));
                            }
                        }
                    }
                }
            }
            out(format!(
                "          Mac Address Table\n-------------------------------------------\n\nVlan    Mac Address       Type        Ports\n----    -----------       --------    -----\n{}\nTotal Mac Addresses for this criterion: {}",
                rows.join("\n"),
                rows.len()
            ))
        }),
        true,
    );
    reg.show(
        "ip arp <ip>",
        |s, st, a| {
            let t = SH::arp_table(st, &s.dev.clone());
            let lines: Vec<&str> = t.split('\n').collect();
            let pat = format!(" {} ", a.s(0));
            let hit: Vec<&str> = lines.iter().filter(|l| l.contains(&pat)).cloned().collect();
            if hit.is_empty() {
                ok()
            } else {
                out(format!("{}\n{}", lines[0], hit.join("\n")))
            }
        },
        true,
    );
    reg.d("exec", "clear arp-cache", |s, st, _| {
        st.dev_mut(&s.dev).arp.clear();
        S::touch(st);
        ok()
    });
    reg.d("exec", "clear ip arp <ip>", |s, st, a| {
        st.dev_mut(&s.dev).arp.shift_remove(a.s(0));
        ok()
    });
    reg.show(
        "port-security address",
        sw_only(|s, st, _| {
            let c = cfg_r(s, st);
            let mut rows = vec![];
            for n in SH::phys_ports(st.dev(&s.dev)) {
                if let Some(i) = c.ifaces.get(&n) {
                    if let Some(ps) = &i.port_sec {
                        for m in &ps.macs {
                            rows.push(format!(
                                "{}{}{}{}   -",
                                lp(i.access_vlan, 5),
                                lp(U::mac_dots(m), 19),
                                lp(if ps.sticky { "SecureSticky" } else { "SecureDynamic" }, 21),
                                lp(U::short_if(&n), 10)
                            ));
                        }
                    }
                }
            }
            out(format!(
                "               Secure Mac Address Table\n-----------------------------------------------------------------------------\nVlan    Mac Address       Type                          Ports   Remaining Age\n                                                                   (mins)\n----    -----------       ----                          -----   -------------\n{}\n-----------------------------------------------------------------------------\nTotal Addresses in System (excluding one mac per port)     : {}\nMax Addresses limit in System (excluding one mac per port) : 4096",
                rows.join("\n"),
                rows.len().saturating_sub(1)
            ))
        }),
        true,
    );
    reg.show(
        "tech-support",
        |s, st, _| {
            let dev = s.dev.clone();
            let mut parts: Vec<(&str, String)> = vec![
                ("show version", SH::show_version(st, &dev)),
                ("show running-config", SH::running_config(st, &dev)),
                ("show ip interface brief", SH::ip_int_brief(st, &dev)),
                ("show logging", SH::show_logging(st, &dev)),
            ];
            if s.is_switch(st) {
                parts.push(("show vlan brief", SH::vlan_brief(st, &dev)));
                parts.push(("show interfaces status", SH::interfaces_status(st, &dev, None)));
            } else {
                parts.push(("show ip route", SH::ip_route(st, &dev, None)));
            }
            out(parts.iter().map(|(h, t)| format!("\n------------------ {} ------------------\n\n{}", h, t)).collect::<Vec<_>>().join("\n"))
        },
        false,
    );
    reg.show(
        "controllers <if>",
        |s, st, a| {
            let n = a.s(0).to_string();
            let d = S::get(st);
            let p = d.ports.get(&S::key(&s.dev, &n));
            let up = p.map(|p| p.up).unwrap_or(false);
            let (sp, dx) = match p.and_then(|p| p.neg.as_ref()) {
                Some(g) => (g.speed.to_string(), format!("{}-duplex", g.duplex)),
                None => ("auto".to_string(), "auto-duplex".to_string()),
            };
            out(format!(
                "Interface {}\nHardware is {}\nLink is {}, speed {}, {}\nautonegotiation enabled\n\n Transmit                      Receive\n        0 Bytes                       0 Bytes\n        0 Unicast frames              0 Unicast frames\n        0 Collision frames            0 FCS errors",
                n,
                if s.is_router(st) { "BCM1125 Internal MAC" } else { "Gigabit Ethernet" },
                if up { "up" } else { "down" },
                sp,
                dx
            ))
        },
        true,
    );
    // DHCP: lease i dagar, timmar och minuter
    reg.d("dhcp", "lease <n:0-365> <n:0-23>", |s, st, a| {
        if let Some(pn) = s.ctx.pool.clone() {
            if let Some(p) = cfg(s, st).dhcp_pools.get_mut(&pn) {
                p.lease = Some(a.n(0) as f64 + a.n(1) as f64 / 24.0);
            }
        }
        ok()
    });
    reg.d("dhcp", "lease <n:0-365> <n:0-23> <n:0-59>", |s, st, a| {
        if let Some(pn) = s.ctx.pool.clone() {
            if let Some(p) = cfg(s, st).dhcp_pools.get_mut(&pn) {
                p.lease = Some(a.n(0) as f64 + a.n(1) as f64 / 24.0 + a.n(2) as f64 / 1440.0);
            }
        }
        ok()
    });
    reg.d("dhcp", "option 150 ip <ip>", |s, st, a| {
        if let Some(pn) = s.ctx.pool.clone() {
            if let Some(p) = cfg(s, st).dhcp_pools.get_mut(&pn) {
                p.opt150 = if a.neg { None } else { Some(a.s(0).to_string()) };
            }
        }
        ok()
    });

    // Konfigurationsregister, boot m.m.
    reg.d("config", "config-register <word>", |s, st, a| {
        let w = a.s(0);
        let hex = w.len() >= 3 && w.len() <= 6 && w[..2].eq_ignore_ascii_case("0x") && w[2..].bytes().all(|b| b.is_ascii_hexdigit());
        if !hex {
            return out(s.invalid_marker(st, &a.line, w));
        }
        cfg(s, st).conf_reg_next = Some(w.to_lowercase());
        ok()
    });
    reg.d("config", "boot system <text>", |s, st, a| {
        let t = rest(&a.line, "system");
        let x = &mut cfg(s, st).x;
        if a.neg {
            x.shift_remove("boot system");
        } else {
            x.insert("boot system".into(), format!("boot system {}", t));
        }
        ok()
    });
    reg.d("config", "logging buffered <n:4096-2147483647> <word>", |s, st, a| {
        let l = &mut cfg(s, st).logging;
        l.buffered = !a.neg;
        if !a.neg {
            l.size = a.n(0);
            l.level = Some(a.s(1).to_string());
        }
        ok()
    });
    reg.d("config", "ntp server <ip> prefer", |s, st, a| {
        let ip = a.s(0).to_string();
        let c = cfg(s, st);
        if a.neg {
            c.ntp_servers.retain(|x| *x != ip);
            S::touch(st);
            return ok();
        }
        if !c.ntp_servers.contains(&ip) {
            c.ntp_servers.insert(0, ip.clone());
        }
        c.ntp_prefer = Some(ip);
        S::touch(st);
        ok()
    });
    reg.d("config", "clock timezone <word> <n:0-23> <n:0-59>", |_, _, _| ok());
    reg.show(
        "users",
        |s, st, _| {
            let me = if s.via == "console" { "*  0 con 0" } else { "*  2 vty 0" };
            let user = s.user.clone().unwrap_or_default();
            let _ = st;
            out(format!(
                "    Line       User       Host(s)              Idle       Location\n{}{}{}00:00:00   {}\n\n  Interface    User               Mode         Idle     Peer Address",
                lp(me, 15),
                lp(user, 11),
                lp("idle", 21),
                if s.via == "console" { "" } else { "192.168.1.200" }
            ))
        },
        true,
    );
    reg.show(
        "interfaces summary",
        |s, st, _| {
            let d = S::get(st);
            let mut o: Vec<String> = [
                " *: interface is up",
                " IHQ: pkts in input hold queue     IQD: pkts dropped from input queue",
                " OHQ: pkts in output hold queue    OQD: pkts dropped from output queue",
                " RXBS: rx rate (bits/sec)          RXPS: rx rate (pkts/sec)",
                " TXBS: tx rate (bits/sec)          TXPS: tx rate (pkts/sec)",
                " TRTL: throttle count",
                "",
                "  Interface                   IHQ       IQD       OHQ       OQD      RXBS      RXPS      TXBS      TXPS      TRTL",
                "-----------------------------------------------------------------------------------------------------------------",
            ]
            .iter()
            .map(|x| x.to_string())
            .collect();
            let c = cfg_r(s, st);
            for n in SH::iface_names(st.dev(&s.dev)) {
                let i = match c.ifaces.get(&n) {
                    Some(i) => i,
                    None => continue,
                };
                let p = d.ports.get(&S::key(&s.dev, &n));
                let ep = d.eps.get(&format!("E:{}:{}", s.dev, n));
                let up = if i.is_loop { !i.shutdown } else if let Some(p) = p { p.up } else { ep.map(|e| e.up).unwrap_or(false) };
                let rx = if up { (U::hash(&format!("{}rx", n)) % 90000 + 1000) as i64 } else { 0 };
                let tx = if up { (U::hash(&format!("{}tx", n)) % 90000 + 1000) as i64 } else { 0 };
                o.push(format!(
                    "{}{}{}{}{}{}{}{}{}{}{}",
                    if up { "* " } else { "  " },
                    lp(&n, 26),
                    rp(0, 5),
                    rp(0, 10),
                    rp(0, 10),
                    rp(0, 10),
                    rp(rx, 10),
                    rp((rx as f64 / 800.0).round() as i64, 10),
                    rp(tx, 10),
                    rp((tx as f64 / 800.0).round() as i64, 10),
                    rp(0, 10)
                ));
            }
            out(o.join("\n"))
        },
        true,
    );
    reg.show("spanning-tree vlan <n:1-4094> brief", sw_only(|s, st, a| out(SH::spanning_tree(st, &s.dev.clone(), Some(a.n(0) as u16)))), true);
    reg.show(
        "memory statistics",
        |s, st, _| {
            let r = s.is_router(st);
            out(format!(
                "                Head    Total(b)     Used(b)     Free(b)   Lowest(b)  Largest(b)\nProcessor   {}\n      I/O   {}",
                if r { "2A6B7E40   414285504    98714424   315571080   314892000   312004096" } else { "1B2C3D40    94363276    27163948    67199328    66900120    66511072" },
                if r { "E800000     75497472    21011032    54486440    54400000    54442780" } else { "7400000      8388608     3207548     5181060     5100000     5160012" }
            ))
        },
        true,
    );

    // scp-server, md5-kontroll och fler CDP-vyer
    reg.d("config", "ip scp server enable", |s, st, a| {
        cfg(s, st).scp_server = !a.neg;
        ok()
    });
    reg.d("exec", "verify /md5 <word>", |_, _, a| {
        let w = a.s(0);
        let f = w.strip_prefix("flash:/").or_else(|| w.strip_prefix("flash:")).unwrap_or(w);
        if !f.ends_with(".bin") {
            return out(format!("%Error verifying flash:{} (No such file or directory)", f));
        }
        let h: String = (0..4).map(|i| format!("{:08x}", U::hash(&format!("{}{}", f, i)))).collect();
        out(format!(".....................................................................................Done!\nverify /md5 (flash:{}) = {}", f, h))
    });
    reg.show("cdp entry *", |s, st, _| out(SH::cdp_detail(st, &s.dev.clone())), true);
    reg.show(
        "cdp neighbors <if>",
        |s, st, a| {
            let all = SH::cdp_neighbors(st, &s.dev.clone());
            let lines: Vec<&str> = all.split('\n').collect();
            let short = U::short_if(a.s(0)).replace("Gi", "Gig ");
            let head: Vec<&str> = lines.iter().take(5).cloned().collect();
            let pat = format!("{} ", short);
            let rows: Vec<&str> = lines.iter().filter(|l| l.contains(&pat)).cloned().collect();
            out(format!("{}\n{}\n\nTotal cdp entries displayed : {}", head.join("\n"), rows.join("\n"), rows.len()))
        },
        true,
    );
    reg.show(
        "line console 0",
        |s, st, _| {
            let l = &cfg_r(s, st).lines.con;
            let sp = if l.speed == 0 { 9600 } else { l.speed };
            out(format!(
                "   Tty Typ     Tx/Rx    A Modem  Roty AccO AccI   Uses   Noise  Overruns   Int\n*    0 CTY              -    -      -    -    -      1       0     0/0       -\n\nLine 0, Location: \"\", Type: \"\"\nLength: 24 lines, Width: 80 columns\nBaud rate (TX/RX) is {sp}/{sp}, no parity, 2 stopbits, 8 databits\nStatus: PSI Enabled, Ready, Active, No Exit Banner\nCapabilities: none\nModem state: Ready\nTimeouts:      Idle EXEC    Idle Session   Modem Answer  Session   Dispatch\n               00:10:00        never                        none     not set\nLogging synchronous: {}",
                if l.log_sync { "enabled" } else { "disabled" }
            ))
        },
        true,
    );

    // Fler show-kommandon som finns på riktiga Catalyst- och ISR-enheter
    reg.show(
        "ssh",
        |s, _st, _| {
            if s.via == "ssh" {
                out("Connection Version Mode Encryption  Hmac         State                 Username\n0          2.0     IN   aes256-ctr  hmac-sha2-256     Session started       drift\n0          2.0     OUT  aes256-ctr  hmac-sha2-256     Session started       drift")
            } else {
                out("%No SSHv1 server connections running.\n%No SSHv2 server connections running.")
            }
        },
        false,
    );
    reg.show(
        "environment",
        |s, st, _| {
            if s.is_router(st) {
                out("Redundant Power System is not present.\n\nSystem Temperature: Normal (34 C)\nFan: All fans normal")
            } else {
                out("FAN is OK\nTEMPERATURE is OK\nTemperature Value: 38 Degree Celsius\nTemperature State: GREEN\nPOWER is OK\nRPS is NOT PRESENT")
            }
        },
        false,
    );
    reg.show(
        "file systems",
        |s, st, _| {
            let sz = if s.is_router(st) { "256589824     201412608" } else { "122185728      98820096" };
            out(format!("File Systems:\n\n       Size(b)       Free(b)      Type  Flags  Prefixes\n*    {sz}      disk     rw   flash:\n             -             -    opaque     rw   system:\n             -             -    opaque     rw   tmpsys:\n        524288        519680     nvram     rw   nvram:\n             -             -   network     rw   tftp:\n             -             -   network     rw   scp:\n             -             -   network     rw   ftp:"))
        },
        false,
    );
    reg.show(
        "storm-control",
        sw_only(|s, st, _| {
            let c = cfg_r(s, st);
            let mut o = vec!["Interface  Filter State   Upper        Lower        Current".to_string()];
            for n in SH::iface_names(st.dev(&s.dev)) {
                let i = match c.ifaces.get(&n) {
                    Some(i) => i,
                    None => continue,
                };
                for t in ["broadcast", "multicast", "unicast"] {
                    if let Some(e) = i.x.get(&format!("storm {}", t)) {
                        let after = e.split("level").nth(1).unwrap_or("").trim();
                        let lv: Vec<&str> = after.split_whitespace().collect();
                        let up = lv.first().cloned().unwrap_or("");
                        let lo = lv.get(1).cloned().unwrap_or(up);
                        o.push(format!("{}{}{}{}0.00%", lp(U::short_if(&n), 11), lp("Forwarding", 15), lp(format!("{}%", up), 13), lp(format!("{}%", lo), 13)));
                    }
                }
            }
            if o.len() > 1 {
                out(o.join("\n"))
            } else {
                out(format!("{}\n💡 Inget storm-control är konfigurerat. Exempel: interface g0/5 → storm-control broadcast level 10", o[0]))
            }
        }),
        false,
    );
    reg.show("monitor session all", sw_only(|_, _, _| out("No SPAN configuration is present in the system.")), false);
    reg.show(
        "udld",
        sw_only(|_, _, _| out("\nInterface Gi1/0/1\n---\nPort enable administrative configuration setting: Follows device default\nPort enable operational state: Disabled\nCurrent bidirectional state: Unknown")),
        false,
    );
    reg.show("system mtu", sw_only(|_, _, _| out("Global Ethernet MTU is 1500 bytes.")), false);
    reg.show(
        "sdm prefer",
        sw_only(|_, _, _| out(" Showing SDM Template Info\n\n This is the Advanced (default) template.\n  Number of VLANs:                                  4094\n  Unicast MAC addresses:                            32768\n  IPv4 routes (direct+indirect):                    3072\n  Security Access Control Entries:                  1536")),
        false,
    );
    reg.show(
        "ip igmp snooping",
        sw_only(|_, _, _| out("Global IGMP Snooping configuration:\n-------------------------------------------\nIGMP snooping                : Enabled\nIGMPv3 snooping (minimal)    : Enabled\nReport suppression           : Enabled\nTCN solicit query            : Disabled\nTCN flood query count        : 2\nRobustness variable          : 2\nLast member query count      : 2\nLast member query interval   : 1000")),
        false,
    );
    reg.show(
        "authentication sessions",
        sw_only(|_, _, _| out("No sessions currently exist\n💡 802.1X/MAB används inte på Nordviks switchar. Port-security skyddar portmottagningen i stället.")),
        false,
    );
    reg.show(
        "ip interface",
        |s, st, _| {
            let names = visible_ifaces(s, st);
            let dev = s.dev.clone();
            out(names.iter().map(|n| SH::ip_interface(st, &dev, n)).collect::<Vec<_>>().join("\n"))
        },
        true,
    );
    reg.show(
        "platform",
        |s, st, _| {
            let m = if s.is_router(st) { "ISR4321/K9" } else { "WS-C2960X-24PS-L" };
            let t = st.time;
            out(format!(
                "Chassis type: {m}\n\nSlot      Type                State                 Insert time (ago)\n--------- ------------------- --------------------- -----------------\n0         {}ok                    {}d{}h",
                lp(m, 20),
                (3.0 + t / 3600.0).floor() as i64,
                ((t / 3600.0) % 24.0).floor() as i64
            ))
        },
        false,
    );
    reg.show(
        "diag",
        rt_only(|s, _st, _| {
            out(format!(
                "Slot 0:\n        ISR4321/K9 Mainboard\n        Hardware Revision        : 1.0\n        PCB Serial Number        : FOC21{}\n\nSlot 0 SPA 0:\n        ISR4321-2x1GE Built-In NIM controller",
                U::hash(&s.dev) % 100000
            ))
        }),
        false,
    );
    reg.show(
        "ip traffic",
        |_s, st, _| {
            let n = (st.time * 3.0).floor() as i64 + 18240;
            let f = |x: f64| (n as f64 * x).floor() as i64;
            out(format!(
                "IP statistics:\n  Rcvd:  {} total, {} local destination\n         0 format errors, 0 checksum errors, 0 bad hop count\n  Frags: 0 reassembled, 0 timeouts, 0 couldn't reassemble\n  Bcast: {} received, {} sent\n  Sent:  {} generated, {} forwarded\n\nICMP statistics:\n  Rcvd: 0 format errors, 0 checksum errors, {} redirects, 0 unreachable, {} echo\n  Sent: {} echo reply, 0 unreachable\n\nARP statistics:\n  Rcvd: {} requests, {} replies\n  Sent: {} requests, {} replies",
                n,
                f(0.4),
                f(0.08),
                f(0.01),
                f(0.42),
                f(0.5),
                n / 900,
                n / 300,
                n / 300,
                n / 60,
                n / 80,
                n / 90,
                n / 60
            ))
        },
        false,
    );
    reg.show(
        "buffers",
        |_s, st, _| {
            out(format!(
                "Buffer elements:\n     1119 in free list (1119 max allowed)\n     {} hits, 0 misses, 619 created\n\nPublic buffer pools:\nSmall buffers, 104 bytes (total 50, permanent 50):\n     48 in free list (20 min, 150 max allowed)\n     0 failures (0 no memory)\nMiddle buffers, 600 bytes (total 25, permanent 25):\n     24 in free list (10 min, 150 max allowed)\n     0 failures (0 no memory)",
                2400 + (st.time / 4.0).floor() as i64
            ))
        },
        false,
    );
    reg.show(
        "ip cef",
        |s, st, _| {
            if !s.is_router(st) && !cfg_r(s, st).ip_routing {
                return out("%IPv4 CEF not running");
            }
            let d = S::get(st);
            let rs = S::installed_routes(st, &d, &s.dev);
            let mut o = vec!["Prefix               Next Hop             Interface".to_string()];
            for r in &rs {
                let nh = r.nh.clone().unwrap_or_else(|| if r.kind == "L" { "receive".into() } else { "attached".into() });
                o.push(format!(
                    "{}{}{}",
                    lp(format!("{}/{}", r.net, U::mask_to_prefix(&r.mask).unwrap_or(0)), 21),
                    lp(nh, 21),
                    U::short_if(r.iface.as_deref().unwrap_or(""))
                ));
            }
            o.push(format!("{}drop", lp("224.0.0.0/4", 21)));
            o.push(format!("{}receive", lp("255.255.255.255/32", 21)));
            out(o.join("\n"))
        },
        true,
    );
    reg.show("policy-map", |_, _, _| ok(), false);
    reg.show("login", |_, _, _| out("     No login delay has been applied.\n     No Quiet-Mode access list has been configured.\n\n     Router NOT enabled to watch for login Attacks"), false);
    reg.show(
        "cdp traffic",
        |_s, st, _| {
            let k = (st.time / 60.0).floor() as i64 + 12;
            out(format!(
                "CDP counters :\n        Total packets output: {}, Input: {}\n        Hdr syntax: 0, Chksum error: 0, Encaps failed: 0\n        No memory: 0, Invalid packet: 0,\n        CDP version 1 advertisements output: 0, Input: 0\n        CDP version 2 advertisements output: {}, Input: {}",
                k,
                k - 1,
                k,
                k - 1
            ))
        },
        true,
    );
    reg.show(
        "interfaces stats",
        |s, st, _| {
            let t = st.time;
            let names = visible_ifaces(s, st);
            let o: Vec<String> = names
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    let pk = (t * (3 + i % 4) as f64).floor() as i64 + 100 * (i as i64 + 1);
                    format!(
                        "{}\n          Switching path    Pkts In   Chars In   Pkts Out  Chars Out\n               Processor{}{}{}{}\n             Route cache{}{}{}{}",
                        n,
                        rp(pk / 20, 11),
                        rp(pk / 20 * 64, 11),
                        rp(pk / 22, 11),
                        rp(pk / 22 * 64, 11),
                        rp(pk, 11),
                        rp(pk * 480, 11),
                        rp(pk, 11),
                        rp(pk * 512, 11)
                    )
                })
                .collect();
            out(o.join("\n"))
        },
        true,
    );
    reg.show(
        "stacks",
        |_s, st, _| {
            out(format!(
                "Minimum process stacks:\n Free/Size   Name\n 5744/6000   Router Init\n 9432/12000  Init\n 5420/6000   RADIUS INITCONFIG\n\nInterrupt level stacks:\nLevel    Called Unused/Size  Name\n  1     {}   8432/9000  Network interfaces",
                5000 + st.time.floor() as i64
            ))
        },
        false,
    );
    reg.show(
        "protocols",
        |s, st, _| {
            let routing = s.is_router(st) || cfg_r(s, st).ip_routing;
            let mut o = vec!["Global values:".to_string(), format!("  Internet Protocol routing is {}", if routing { "enabled" } else { "disabled" })];
            let names = visible_ifaces(s, st);
            let dev = s.dev.clone();
            for n in names {
                let t = SH::ip_interface(st, &dev, &n);
                let l: Vec<&str> = t.split('\n').collect();
                o.push(l[0].to_string());
                for x in &l {
                    if x.contains("Internet address is") {
                        o.push(x.to_string());
                    }
                }
            }
            out(o.join("\n"))
        },
        true,
    );
    reg.show(
        "logging history",
        |_, _, _| out("Syslog History Table:1 maximum table entries,\nsaving level warnings or higher\n 0 messages ignored, 0 dropped, 0 recursion drops\n 0 table entries flushed\n SNMP notifications not enabled"),
        false,
    );
    let _ = (NO_NEG, 0u8);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::model::build_golden;

    fn session(week: u32, dev: &str) -> (State, IosSession) {
        let st = State::new(build_golden(week));
        let s = IosSession::new(&st, dev, "console", true, None);
        (st, s)
    }
    /// Kör en rad som handle_line men utan sim::refresh (som kräver färdig simulator).
    fn step(s: &mut IosSession, st: &mut State, c: &str) -> String {
        if let Some(p) = s.pending.take() {
            return (p.f)(s, st, c.trim()).out;
        }
        s.exec(st, c).out
    }
    fn run(s: &mut IosSession, st: &mut State, cmds: &[&str]) -> String {
        let mut o = String::new();
        for c in cmds {
            o.push_str(&step(s, st, c));
            o.push('\n');
        }
        o
    }
    fn first_switch(st: &State) -> String {
        st.devices.iter().find(|(_, d)| d.kind == Kind::Switch && d.os == Os::Ios).map(|(k, _)| k.clone()).unwrap()
    }

    #[test]
    fn registrerar_utan_panik() {
        let mut reg = Reg::default();
        register(&mut reg);
        assert!(reg.tries.contains_key("config"));
    }

    #[test]
    fn config_kommandon_sparas() {
        let (mut st, _) = session(13, "x");
        let sw = first_switch(&st);
        let mut s = IosSession::new(&st, &sw, "console", true, None);
        s.mode = "config".into();
        run(&mut s, &mut st, &["vtp mode transparent", "vtp domain NORDVIK", "cdp timer 30", "alias exec sib show ip interface brief", "snmp-server community hemlig RO", "end"]);
        let c = st.dev(&sw).cfg();
        assert_eq!(c.vtp.as_ref().unwrap().mode, "transparent");
        assert_eq!(c.vtp.as_ref().unwrap().domain, "NORDVIK");
        assert_eq!(c.x.get("cdp timer").unwrap(), "cdp timer 30");
        assert_eq!(c.aliases.get("sib").unwrap(), "show ip interface brief");
        assert_eq!(c.snmp.as_ref().unwrap().communities.get("hemlig").unwrap(), "RO");
    }

    #[test]
    fn banner_en_rad_och_flera_rader() {
        let (mut st, _) = session(13, "x");
        let sw = first_switch(&st);
        let mut s = IosSession::new(&st, &sw, "console", true, None);
        s.mode = "config".into();
        run(&mut s, &mut st, &["banner motd #Hej#"]);
        assert_eq!(st.dev(&sw).cfg().banners["motd"].text, "Hej");
        run(&mut s, &mut st, &["banner login $rad ett", "rad två$"]);
        assert_eq!(st.dev(&sw).cfg().banners["login"].text, "rad ett\nrad två");
    }

    #[test]
    fn debug_och_reload() {
        let (mut st, _) = session(13, "x");
        let sw = first_switch(&st);
        let mut s = IosSession::new(&st, &sw, "console", true, None);
        let o = run(&mut s, &mut st, &["debug ip packet"]);
        assert!(o.contains("IP packet debugging is on"));
        assert!(st.dev(&sw).rt.debug.contains_key("ip packet"));
        run(&mut s, &mut st, &["undebug all"]);
        assert!(st.dev(&sw).rt.debug.is_empty());
        run(&mut s, &mut st, &["reload in 5", "y"]);
        assert!(st.dev(&sw).rt.reload_at.is_some());
        run(&mut s, &mut st, &["reload cancel"]);
        assert!(st.dev(&sw).rt.reload_at.is_none());
    }

    #[test]
    fn klocka_erase_och_interface() {
        let mut st = State::new(build_golden(13));
        let sw = first_switch(&st);
        let mut s = IosSession::new(&st, &sw, "console", true, None);
        run(&mut s, &mut st, &["clock set 10:15:00 5 March 2026"]);
        let cs = st.dev(&sw).rt.clock_set.clone().unwrap();
        assert_eq!(cs.ms, U::date_utc(2026, 2, 5, 10, 15, 0));
        run(&mut s, &mut st, &["erase startup-config", "y"]);
        assert!(st.dev(&sw).startup.is_none());
        let ifn = st.dev(&sw).cfg().ifaces.keys().find(|k| k.starts_with("GigabitEthernet")).cloned().unwrap();
        s.mode = "if".into();
        s.ctx.ifs = vec![ifn.clone()];
        let o = run(&mut s, &mut st, &["channel-group 3 mode active", "storm-control broadcast level 10", "spanning-tree cost 19"]);
        assert!(o.contains("Creating a port-channel interface Port-channel 3"));
        let i = &st.dev(&sw).cfg().ifaces[&ifn];
        assert_eq!(i.ch_group.as_ref().unwrap().n, 3);
        assert_eq!(i.x["storm broadcast"], "storm-control broadcast level 10");
        assert_eq!(i.x["stp cost"], "spanning-tree cost 19");
    }
}
