//! IOS-kommandon från js/cli/ios.js (rad 403–1610): exec, config, interface, VLAN, DHCP, ACL, NAT, SSH, linjer, IPsec.
use super::ios::{result, Args, Ctx, DefOpts, IosSession, Reg, Tok, NO_NEG, NO_ONLY};
use super::ios_show as SH;
use super::{Chunk, CmdResult};
use crate::net::model::*;
use crate::net::sim::{self as S, PingOpts, State};
use crate::net::util::{self as U, VlanList};

// ------------------------------------------------------------------ Hjälpfunktioner

pub fn cfg<'a>(s: &IosSession, st: &'a State) -> &'a IosConfig {
    st.dev(&s.dev).cfg()
}
pub fn cfg_mut<'a>(s: &IosSession, st: &'a mut State) -> &'a mut IosConfig {
    st.dev_mut(&s.dev).cfg_mut()
}
fn is_y(a: &str) -> bool {
    a.to_lowercase().starts_with('y')
}
fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}
pub fn next_seq(acl: &Acl) -> i64 {
    acl.rules.iter().map(|r| r.seq).max().unwrap_or(0).max(0) + 10
}
const INVALID: &str = "% Invalid input detected at '^' marker.";

/// JS: String(x).slice(-4) av '000'+x
fn default_vlan_name(x: u16) -> String {
    format!("VLAN{:04}", x)
}

/// Kör f för varje interface i sammanhanget; unika svar slås ihop (eachIf i ios.js).
pub fn each_if<F>(s: &IosSession, st: &mut State, mut f: F) -> String
where
    F: FnMut(&mut Iface, &str) -> Option<String>,
{
    let names = s.ctx.ifs.clone();
    let mut outs: Vec<String> = vec![];
    for n in names {
        let r = match cfg_mut(s, st).ifaces.get_mut(&n) {
            Some(i) => f(i, &n),
            None => None,
        };
        if let Some(r) = r {
            if !r.is_empty() && !outs.contains(&r) {
                outs.push(r);
            }
        }
    }
    outs.join("\n")
}

/// Som each_if men closuren får hela enheten (för rt-data m.m.).
pub fn each_if_dev<F>(s: &IosSession, st: &mut State, mut f: F) -> String
where
    F: FnMut(&mut Device, &str) -> Option<String>,
{
    let names = s.ctx.ifs.clone();
    let mut outs: Vec<String> = vec![];
    for n in names {
        let r = f(st.dev_mut(&s.dev), &n);
        if let Some(r) = r {
            if !r.is_empty() && !outs.contains(&r) {
                outs.push(r);
            }
        }
    }
    outs.join("\n")
}

/// 'l2', 'sub', 'svi' eller 'l3' (kindOf)
pub fn kind_of(is_switch: bool, i: &Iface) -> &'static str {
    if i.parent.is_some() {
        "sub"
    } else if i.svi.is_some() {
        "svi"
    } else if i.is_loop {
        "l3"
    } else if is_switch {
        "l2"
    } else {
        "l3"
    }
}

/// Kommando som bara finns på switchar
pub fn sw_only<F>(f: F) -> impl Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static
where
    F: Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static,
{
    move |s, st, a| {
        if !s.is_switch(st) {
            let w = a.kws.get(1).or(a.kws.first()).cloned().unwrap_or_default();
            return result(s.invalid_marker(st, &a.line, &w));
        }
        f(s, st, a)
    }
}
/// Kommando som bara finns på routrar
pub fn rt_only<F>(f: F) -> impl Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static
where
    F: Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static,
{
    move |s, st, a| {
        if !s.is_router(st) {
            let w = a.kws.get(1).or(a.kws.first()).cloned().unwrap_or_default();
            return result(s.invalid_marker(st, &a.line, &w));
        }
        f(s, st, a)
    }
}

fn log_cfg_i(s: &IosSession, st: &mut State) {
    let via = if s.via == "console" { "console by console" } else { "vty0 (192.168.1.200)" };
    S::push_log(st, &s.dev, &format!("%SYS-5-CONFIG_I: Configured from {}", via));
}
fn end_cmd(s: &mut IosSession, st: &mut State, _a: &Args) -> CmdResult {
    s.mode = "exec".into();
    s.ctx = Ctx::default();
    log_cfg_i(s, st);
    result("")
}

fn save_config(s: &IosSession, st: &mut State) -> String {
    let d = st.dev_mut(&s.dev);
    d.startup = d.config.clone();
    "Building configuration...\n[OK]".into()
}

// ------------------------------------------------------------------ Registrering

pub fn register(reg: &mut Reg) {
    reg_exec(reg);
    reg_show(reg);
    reg_clear_ping(reg);
    reg_config(reg);
    reg_dhcp_acl_nat(reg);
    reg_ssh_line_vlan(reg);
    reg_interface(reg);
    reg_more(reg);
    reg_ipsec(reg);
}

// ------------------------------------------------------------------ EXEC / USER

fn enable_ask(s: &mut IosSession, tries: u32) {
    s.ask("Password: ", true, move |s, st, pw| {
        let plain = cfg(s, st).enable_plain.clone();
        if plain.as_deref() == Some(pw) {
            s.mode = "exec".into();
            return result("");
        }
        let t = tries + 1;
        if t >= 3 {
            return result("% Bad secrets\n");
        }
        enable_ask(s, t);
        result("")
    });
}

fn do_reload(s: &mut IosSession, st: &mut State) -> CmdResult {
    let time = st.time;
    let dev = s.dev.clone();
    let d = st.dev_mut(&dev);
    // Konfigurationsregistret gäller vid nästa start även om det inte sparats (det ligger i NVRAM)
    let reg: Option<String> = d.cfg().conf_reg_next.clone().or_else(|| d.cfg().conf_reg.clone());
    // Utan sparad konfiguration startar enheten med fabriksinställningar.
    // Med registret 0x2142 hoppar enheten över startup-config (så går lösenordsåterställning till)
    let newc = match &d.startup {
        Some(sc) if reg.as_deref() != Some("0x2142") => sc.clone(),
        _ => crate::net::model::factory_config(d),
    };
    d.config = Some(newc);
    if let Some(r) = &reg {
        let c = d.cfg_mut();
        c.conf_reg = Some(r.clone());
        c.conf_reg_next = None;
    }
    d.rt.logs.clear();
    d.rt.dhcp_bindings.clear();
    d.rt.nat_trans.clear();
    d.rt.errdisabled.clear();
    d.rt.acl_counters.clear();
    d.rt.counters.clear();
    d.rt.boot_time = time;
    d.rt.reloads += 1;
    let router = d.kind == Kind::Router;
    s.mode = "user".into();
    s.ctx = Ctx::default();
    s.booting = true;
    S::touch(st);
    let ch = |t: &str, d: u32| Chunk { text: t.to_string(), delay: d };
    let stream = vec![
        ch("\n\n%SYS-5-RELOAD: Reload requested by console. Reload Reason: Reload Command.", 300),
        ch(&format!("\n\nSystem Bootstrap, Version {}, RELEASE SOFTWARE (fc1)", if router { "15.0(1r)M15" } else { "12.2(44)SE5" }), 900),
        ch("\nCopyright (c) 2016 by cisco Systems, Inc.", 200),
        ch(
            &format!(
                "\n\nInitializing memory...\nLoading \"flash:/{}\"...",
                if router { "c2951-universalk9-mz.SPA.152-4.M11.bin" } else { "c3560-ipservicesk9-mz.122-55.SE12.bin" }
            ),
            700,
        ),
        ch("@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@", 1500),
        ch("\n\n              Restricted Rights Legend\n\nCisco Systems, Inc.\n170 West Tasman Drive\nSan Jose, California 95134-1706", 600),
        ch("\n\nPress RETURN to get started!\n", 1200),
    ];
    CmdResult { stream, wait_return: true, ..Default::default() }
}

fn reload_proceed(s: &mut IosSession) {
    s.ask("Proceed with reload? [confirm]", false, |s, st, a| {
        if !a.is_empty() && !is_y(a) {
            return result("");
        }
        do_reload(s, st)
    });
}

fn reg_exec(reg: &mut Reg) {
    reg.d("user", "enable", |s, st, _| {
        let c = cfg(s, st);
        if s.via != "console" && c.enable_secret.is_none() && c.enable_password.is_none() {
            return result("% No password set\n💡 Över SSH behöver kontot privilege 15 (username <namn> privilege 15 secret <lösenord>) eller så måste ett enable secret vara satt.");
        }
        if c.enable_secret.is_none() && c.enable_password.is_none() {
            s.mode = "exec".into();
            return result("");
        }
        enable_ask(s, 0);
        result("")
    });
    reg.d("exec", "enable", |_, _, _| result(""));
    reg.d("exec", "disable", |s, _, _| {
        s.mode = "user".into();
        result("")
    });
    for w in ["exit", "logout", "quit"] {
        reg.d("user exec", w, |s, _, _| {
            s.closed = true;
            CmdResult { close: true, ..Default::default() }
        });
    }
    reg.d("exec", "configure terminal", |s, _, _| {
        s.mode = "config".into();
        result("Enter configuration commands, one per line.  End with CNTL/Z.")
    });
    reg.d("exec", "configure", |s, _, _| {
        s.ask("Configuring from terminal, memory, or network [terminal]? ", false, |s, _, a| {
            if a.is_empty() || "terminal".starts_with(a) {
                s.mode = "config".into();
                return result("Enter configuration commands, one per line.  End with CNTL/Z.");
            }
            result("?Must be \"terminal\", \"memory\" or \"network\"")
        });
        result("")
    });
    reg.d("exec", "write", |s, st, _| result(save_config(s, st)));
    reg.d("exec", "write memory", |s, st, _| result(save_config(s, st)));
    reg.d("exec", "copy running-config startup-config", |s, _, _| {
        s.ask("Destination filename [startup-config]? ", false, |s, st, a| {
            if !a.is_empty() && a != "startup-config" {
                return result(format!("%Error opening flash:{} (Permission denied)", a));
            }
            CmdResult::delayed(save_config(s, st), 400)
        });
        result("")
    });
    reg.d("exec", "copy startup-config running-config", |s, st, _| {
        let sc = st.dev(&s.dev).startup.clone();
        let Some(sc) = sc else {
            return result("%Error opening nvram:/startup-config (No such file or directory)");
        };
        st.dev_mut(&s.dev).config = Some(sc);
        let bytes = SH::running_config(st, &s.dev).encode_utf16().count();
        result(format!(
            "Destination filename [running-config]? \n{} bytes copied in 0.520 secs ({} bytes/sec)",
            bytes,
            (bytes as f64 / 0.52).round() as i64
        ))
    });
    reg.d("exec", "reload", |s, st, _| {
        let d = st.dev(&s.dev);
        let changed = d.config != d.startup;
        if changed {
            s.ask("\nSystem configuration has been modified. Save? [yes/no]: ", false, |s, st, a| {
                let y = is_y(a);
                if y {
                    save_config(s, st);
                } else if !a.to_lowercase().starts_with('n') {
                    return result("% Please answer 'yes' or 'no'.");
                }
                reload_proceed(s);
                result(if y { "Building configuration...\n[OK]" } else { "" })
            });
        } else {
            reload_proceed(s);
        }
        result("")
    });
}

// ------------------------------------------------------------------ show

fn if_exists(s: &IosSession, st: &State, n: &str) -> bool {
    cfg(s, st).ifaces.contains_key(n)
}

fn pad_r(s: impl std::fmt::Display, n: usize) -> String {
    U::pad(s, n)
}

fn reg_show(reg: &mut Reg) {
    reg.show("running-config", |s, st, _| result(SH::running_config(st, &s.dev)), false);
    reg.show(
        "running-config interface <if>",
        |s, st, a| {
            if !if_exists(s, st, a.s(0)) {
                return result("                                           ^\n% Invalid input detected at '^' marker.");
            }
            result(SH::running_interface(st, &s.dev, a.s(0)))
        },
        false,
    );
    reg.show("startup-config", |s, st, _| result(SH::startup_config(st, &s.dev)), false);
    reg.show("version", |s, st, _| result(SH::show_version(st, &s.dev)), true);
    reg.show("clock", |s, st, _| result(SH::show_clock(st, &s.dev)), true);
    reg.show("ntp status", |s, st, _| result(SH::ntp_status(st, &s.dev)), true);
    reg.show("clock detail", |s, st, _| result(SH::show_clock_detail(st, &s.dev)), true);
    reg.show("ntp associations", |s, st, _| result(SH::ntp_associations(st, &s.dev)), true);
    reg.show("ip cef <ip>", rt_only(|s, st, a| result(SH::ip_cef(st, &s.dev, a.s(0)))), true);
    reg.show("logging", |s, st, _| result(SH::show_logging(st, &s.dev)), false);
    reg.show("history", |s, _, _| result(s.history.iter().map(|h| format!("  {}", h)).collect::<Vec<_>>().join("\n")), true);
    reg.show("ip interface brief", |s, st, _| result(SH::ip_int_brief(st, &s.dev)), true);
    reg.show(
        "ip interface <if>",
        |s, st, a| {
            if !if_exists(s, st, a.s(0)) {
                return result(INVALID);
            }
            result(SH::ip_interface(st, &s.dev, a.s(0)))
        },
        true,
    );
    reg.show(
        "interfaces",
        |s, st, _| {
            let names = SH::iface_names(st.dev(&s.dev));
            let mut out = vec![];
            for n in names {
                if cfg(s, st).ifaces.get(&n).map(|i| i.internal).unwrap_or(false) {
                    continue;
                }
                out.push(SH::show_interface(st, &s.dev, &n));
            }
            result(out.join("\n"))
        },
        true,
    );
    reg.show(
        "interfaces <if>",
        |s, st, a| {
            if !if_exists(s, st, a.s(0)) {
                return result(INVALID);
            }
            result(SH::show_interface(st, &s.dev, a.s(0)))
        },
        true,
    );
    reg.show("interfaces description", |s, st, _| result(SH::interfaces_description(st, &s.dev)), true);
    reg.show(
        "ip route",
        |s, st, _| {
            if !s.is_router(st) && !cfg(s, st).ip_routing {
                return result(format!(
                    "Default gateway is {}\n\nHost               Gateway           Last Use    Total Uses  Interface\nICMP redirect cache is empty",
                    cfg(s, st).default_gateway.clone().unwrap_or_else(|| "not set".into())
                ));
            }
            result(SH::ip_route(st, &s.dev, None))
        },
        true,
    );
    reg.show("ip route static", |s, st, _| result(SH::ip_route(st, &s.dev, Some("static"))), true);
    reg.show("ip route connected", |s, st, _| result(SH::ip_route(st, &s.dev, Some("connected"))), true);
    reg.show("ip route <ip>", |s, st, a| result(SH::ip_route(st, &s.dev, Some(a.s(0)))), true);
    reg.show("arp", |s, st, _| result(SH::arp_table(st, &s.dev)), true);
    reg.show("ip arp", |s, st, _| result(SH::arp_table(st, &s.dev)), true);
    reg.show("cdp neighbors", |s, st, _| result(SH::cdp_neighbors(st, &s.dev)), true);
    reg.show("ip ssh", |s, st, _| result(SH::ip_ssh(st, &s.dev)), false);
    reg.show(
        "users",
        |_, _, _| {
            result("    Line       User       Host(s)              Idle       Location\n*  0 con 0                idle                 00:00:00   \n\n  Interface    User               Mode         Idle     Peer Address")
        },
        true,
    );
    reg.show("access-lists", |s, st, _| result(SH::access_lists(st, &s.dev, None, false)), false);
    reg.show("access-lists <word>", |s, st, a| result(SH::access_lists(st, &s.dev, Some(a.s(0)), false)), false);
    reg.show("ip access-lists", |s, st, _| result(SH::access_lists(st, &s.dev, None, false)), false);
    reg.show("ip access-lists <word>", |s, st, a| result(SH::access_lists(st, &s.dev, Some(a.s(0)), false)), false);
    // Switch-specifika
    reg.show("interfaces status", sw_only(|s, st, _| result(SH::interfaces_status(st, &s.dev, None))), true);
    reg.show("interfaces <if> status", sw_only(|s, st, a| result(SH::interfaces_status(st, &s.dev, Some(a.s(0))))), true);
    reg.show("interfaces trunk", sw_only(|s, st, _| result(SH::interfaces_trunk(st, &s.dev))), true);
    reg.show(
        "interfaces <if> switchport",
        sw_only(|s, st, a| {
            if !if_exists(s, st, a.s(0)) {
                return result(INVALID);
            }
            result(SH::switchport(st, &s.dev, a.s(0)))
        }),
        true,
    );
    reg.show("vlan brief", sw_only(|s, st, _| result(SH::vlan_brief(st, &s.dev))), true);
    reg.show("vlan", sw_only(|s, st, _| result(SH::vlan_brief(st, &s.dev))), true);
    reg.show("mac address-table", sw_only(|s, st, _| result(SH::mac_table(st, &s.dev, SH::MacFilter::default()))), true);
    reg.show(
        "mac address-table address <word>",
        sw_only(|s, st, a| result(SH::mac_table(st, &s.dev, SH::MacFilter { address: Some(a.s(0).to_string()), ..Default::default() }))),
        true,
    );
    reg.show(
        "mac address-table interface <if>",
        sw_only(|s, st, a| result(SH::mac_table(st, &s.dev, SH::MacFilter { iface: Some(a.s(0).to_string()), ..Default::default() }))),
        true,
    );
    reg.show(
        "mac address-table vlan <n:1-4094>",
        sw_only(|s, st, a| result(SH::mac_table(st, &s.dev, SH::MacFilter { vlan: Some(a.n(0) as u16), ..Default::default() }))),
        true,
    );
    reg.show("mac-address-table", sw_only(|s, st, _| result(SH::mac_table(st, &s.dev, SH::MacFilter::default()))), true);
    reg.show("mac address-table dynamic", sw_only(|s, st, _| result(SH::mac_table(st, &s.dev, SH::MacFilter::default()))), true);
    reg.show("spanning-tree", sw_only(|s, st, _| result(SH::spanning_tree(st, &s.dev, None))), true);
    reg.show("spanning-tree vlan <n:1-4094>", sw_only(|s, st, a| result(SH::spanning_tree(st, &s.dev, Some(a.n(0) as u16)))), true);
    reg.show("power inline", sw_only(|s, st, _| result(SH::power_inline(st, &s.dev, None))), true);
    reg.show("power inline <if>", sw_only(|s, st, a| result(SH::power_inline(st, &s.dev, Some(a.s(0))))), true);
    reg.show("port-security", sw_only(|s, st, _| result(SH::port_security(st, &s.dev, None))), false);
    reg.show("port-security interface <if>", sw_only(|s, st, a| result(SH::port_security(st, &s.dev, Some(a.s(0))))), false);
    // Router-specifika
    reg.show("ip dhcp binding", rt_only(|s, st, _| result(SH::dhcp_binding(st, &s.dev))), false);
    reg.show("ip dhcp pool", rt_only(|s, st, _| result(SH::dhcp_pool(st, &s.dev, None))), false);
    reg.show("ip dhcp pool <word>", rt_only(|s, st, a| result(SH::dhcp_pool(st, &s.dev, Some(a.s(0))))), false);
    reg.show(
        "ip dhcp conflict",
        rt_only(|s, st, _| {
            let rows: Vec<String> = st
                .dev(&s.dev)
                .rt
                .dhcp_bindings
                .iter()
                .filter(|(_, b)| b.conflict)
                .map(|(ip, _)| format!("{}{}Sep 29 2026 08:0{} AM", pad_r(ip, 18), pad_r("Ping", 19), U::hash(ip) % 9))
                .collect();
            result(format!("IP address        Detection method   Detection time          VRF\n{}", rows.join("\n")))
        }),
        false,
    );
    reg.show("ip nat translations", rt_only(|s, st, _| result(SH::nat_translations(st, &s.dev))), false);
    reg.show("ip nat statistics", rt_only(|s, st, _| result(SH::nat_statistics(st, &s.dev))), false);
}

// ------------------------------------------------------------------ clear, ping, traceroute

fn fresh_counters(time: f64) -> Counters {
    Counters { cleared: Some(time), ..Default::default() }
}

#[derive(Clone, Default)]
struct PingO {
    count: Option<i64>,
    size: Option<i64>,
    src: Option<String>,
    df: bool,
}

fn ping_cmd(s: &mut IosSession, st: &mut State, target: &str, o: &PingO) -> CmdResult {
    let dev = s.dev.clone();
    let mut head = String::new();
    let mut ip = target.to_string();
    if !U::is_ip(target) {
        let r = S::resolve(st, &dev, target);
        let ns0 = cfg(s, st).name_servers.first().cloned();
        if r.error.is_some() {
            let ns = ns0.unwrap_or_else(|| "255.255.255.255".into());
            if !cfg(s, st).domain_lookup {
                return result("% Unrecognized host or address, or protocol not running.\n");
            }
            return CmdResult::delayed(
                format!("Translating \"{}\"...domain server ({})\n% Unrecognized host or address, or protocol not running.\n", target, ns),
                2000,
            );
        }
        ip = r.ip.unwrap_or_default();
        if !cfg(s, st).hosts.contains_key(&target.to_lowercase()) {
            head = format!("Translating \"{}\"...domain server ({}) [OK]\n", target, ns0.unwrap_or_default());
        }
    }
    let count = o.count.unwrap_or(5);
    let size = o.size.unwrap_or(100);
    let res = S::ping(st, &dev, &ip, PingOpts { src: o.src.clone(), size: Some(size), df: o.df, ..Default::default() });
    let mut stream = vec![Chunk {
        text: format!(
            "{}Type escape sequence to abort.\nSending {}, {}-byte ICMP Echos to {}, timeout is 2 seconds:\n{}{}",
            head,
            count,
            size,
            ip,
            o.src.as_ref().map(|x| format!("Packet sent with a source address of {} \n", x)).unwrap_or_default(),
            if o.df { "Packet sent with the DF bit set\n" } else { "" }
        ),
        delay: 50,
    }];
    let mut ok_n = 0;
    let is_router = st.dev(&dev).kind == Kind::Router;
    let first = !st.dev(&dev).rt.pinged.get(&ip).copied().unwrap_or(false);
    for k in 0..count {
        let ch;
        if res.ok {
            let lost = st.random() < res.loss || (k == 0 && first && is_router && res.loss < 1.0 && st.random() < 0.8);
            ch = if lost { "." } else { "!" };
        } else if res.reason.as_deref() == Some("net-unreachable") && res.where_.as_deref() != Some(dev.as_str()) {
            ch = "U";
        } else if res.reason.as_deref() == Some("frag") {
            ch = "M";
        } else {
            ch = ".";
        }
        if ch == "!" {
            ok_n += 1;
        }
        stream.push(Chunk { text: ch.into(), delay: if ch == "!" { 120 } else { 1400 } });
    }
    if res.ok {
        st.dev_mut(&dev).rt.pinged.insert(ip.clone(), true);
    }
    let pct = (ok_n as f64 * 100.0 / count as f64).round() as i64;
    let rtt = if ok_n > 0 { format!(", round-trip min/avg/max = 1/{}/{} ms", 1 + res.hops.len(), 4 + res.hops.len() * 2) } else { String::new() };
    stream.push(Chunk { text: format!("\nSuccess rate is {} percent ({}/{}){}", pct, ok_n, count, rtt), delay: 50 });
    CmdResult::stream(stream)
}

/// Fråga med standardsvar i hakparentes; tomt svar ger standardvärdet.
fn ask_def<F>(s: &mut IosSession, prompt: &str, def: &str, f: F) -> CmdResult
where
    F: FnOnce(&mut IosSession, &mut State, String) -> CmdResult + 'static,
{
    let d = def.to_string();
    s.ask(format!("{} [{}]: ", prompt, def), false, move |s, st, a| {
        let a = if a.is_empty() { d } else { a.to_string() };
        f(s, st, a)
    });
    result("")
}

fn ping_step7(s: &mut IosSession, target: String, o: PingO) -> CmdResult {
    ask_def(s, "Set DF bit in IP header?", "no", move |s, _, a| {
        let mut o = o;
        o.df = is_y(&a);
        ask_def(s, "Validate reply data?", "no", move |s, _, _| {
            ask_def(s, "Data pattern", "0xABCD", move |s, _, _| {
                ask_def(s, "Loose, Strict, Record, Timestamp, Verbose", "none", move |s, _, _| {
                    ask_def(s, "Sweep range of sizes", "n", move |s, st, _| ping_cmd(s, st, &target, &o))
                })
            })
        })
    })
}

fn iface_ip(s: &IosSession, st: &State, name: &str) -> Option<String> {
    let n = U::norm_if(name)?;
    cfg(s, st).ifaces.get(&n).and_then(|i| i.ip.as_ref()).map(|ip| ip.addr.clone())
}

fn trace_cmd(s: &mut IosSession, st: &mut State, target: &str, src: Option<&str>) -> CmdResult {
    let dev = s.dev.clone();
    let mut ip = target.to_string();
    if !U::is_ip(&ip) {
        let r = S::resolve(st, &dev, &ip);
        if r.error.is_some() {
            return result("% Unrecognized host or address, or protocol not running.\n");
        }
        ip = r.ip.unwrap_or_default();
    }
    let (fwd, full) = S::traceroute(st, &dev, &ip, src);
    let mut stream = vec![Chunk {
        text: format!("Type escape sequence to abort.\nTracing the route to {}\nVRF info: (vrf in name/id, vrf out name/id)\n", ip),
        delay: 100,
    }];
    let hops = fwd.hops.clone();
    let mut n = 1;
    for h in &hops {
        if h.dev == dev {
            continue;
        }
        stream.push(Chunk { text: format!("  {} {} {} msec {} msec {} msec\n", n, h.ip, 1 + n, n, 1 + n), delay: 350 });
        n += 1;
    }
    if full.ok {
        let last = hops.last().map(|h| h.ip.clone());
        if last.as_deref() != Some(ip.as_str()) {
            stream.push(Chunk { text: format!("  {} {} {} msec {} msec {} msec\n", n, ip, 2 + n, 1 + n, 2 + n), delay: 350 });
        }
    } else {
        for _ in 0..4 {
            stream.push(Chunk { text: format!("  {}  *  *  * \n", n), delay: 1800 });
            n += 1;
        }
    }
    CmdResult::stream(stream)
}

fn reg_clear_ping(reg: &mut Reg) {
    reg.def(
        "exec",
        "clear counters",
        |s, _, _| {
            s.ask("Clear \"show interface\" counters on all interfaces [confirm]", false, |s, st, a| {
                if !a.is_empty() && !is_y(a) {
                    return result("");
                }
                let time = st.time;
                for (_, c) in st.dev_mut(&s.dev).rt.counters.iter_mut() {
                    *c = fresh_counters(time);
                }
                S::push_log(st, &s.dev, "%CLEAR-5-COUNTERS: Clear counter on all interfaces by console");
                result("")
            });
            result("")
        },
        NO_NEG,
    );
    reg.d("exec", "clear counters <if>", |s, _, a| {
        let n = a.s(0).to_string();
        s.ask("Clear \"show interface\" counters on this interface [confirm]", false, move |s, st, _| {
            let time = st.time;
            st.dev_mut(&s.dev).rt.counters.insert(n.clone(), fresh_counters(time));
            S::push_log(st, &s.dev, &format!("%CLEAR-5-COUNTERS: Clear counter on interface {} by console", n));
            result("")
        });
        result("")
    });
    reg.d("exec", "clear access-list counters", |s, st, _| {
        st.dev_mut(&s.dev).rt.acl_counters.clear();
        result("")
    });
    reg.d("exec", "clear access-list counters <word>", |s, st, a| {
        st.dev_mut(&s.dev).rt.acl_counters.shift_remove(a.s(0));
        result("")
    });
    reg.d("exec", "clear ip access-list counters", |s, st, _| {
        st.dev_mut(&s.dev).rt.acl_counters.clear();
        result("")
    });
    reg.d("exec", "clear ip nat translation <word>", |s, st, a| {
        if a.s(0) != "*" {
            return result(INVALID);
        }
        st.dev_mut(&s.dev).rt.nat_trans.clear();
        result("")
    });
    reg.d("exec", "clear mac address-table dynamic", |_, _, _| result(""));
    reg.d("exec", "clear mac-address-table dynamic", |_, _, _| result(""));
    reg.d("exec", "clear logging", |s, _, _| {
        s.ask("Clear logging buffer [confirm]", false, |s, st, _| {
            st.dev_mut(&s.dev).rt.logs.clear();
            result("")
        });
        result("")
    });
    reg.d("exec", "clear ip dhcp binding <word>", |s, st, a| {
        let rt = &mut st.dev_mut(&s.dev).rt;
        if a.s(0) == "*" {
            rt.dhcp_bindings.clear();
        } else {
            rt.dhcp_bindings.shift_remove(a.s(0));
        }
        result("")
    });
    reg.d("exec", "clear ip dhcp conflict <word>", |s, st, _| {
        st.dev_mut(&s.dev).rt.dhcp_bindings.retain(|_, b| !b.conflict);
        result("")
    });
    reg.d("user exec", "terminal length <n:0-512>", |_, _, _| result(""));
    reg.d("user exec", "terminal monitor", |s, _, _| {
        s.monitor = true;
        result("")
    });
    reg.d("user exec", "terminal no monitor", |s, _, _| {
        s.monitor = false;
        result("")
    });

    // ping / traceroute
    reg.d("user exec", "ping <iphost>", |s, st, a| ping_cmd(s, st, a.s(0), &PingO::default()));
    // Utökad ping: bara "ping" frågar steg för steg, som på en riktig router
    reg.def(
        "exec",
        "ping",
        |s, _, _| {
            ask_def(s, "Protocol", "ip", |s, _, _| {
                s.ask("Target IP address: ", false, |s, _, t| {
                    if t.is_empty() {
                        return result("% Bad IP address");
                    }
                    let target = t.to_string();
                    ask_def(s, "Repeat count", "5", move |s, _, a| {
                        let mut o = PingO::default();
                        o.count = Some(a.trim().parse::<i64>().ok().filter(|x| *x != 0).unwrap_or(5).clamp(1, 100));
                        ask_def(s, "Datagram size", "100", move |s, _, b| {
                            let z = b.trim().parse::<i64>().unwrap_or(0);
                            if !(36..=18024).contains(&z) {
                                return result("% A decimal number between 36 and 18024.");
                            }
                            o.size = Some(z);
                            ask_def(s, "Timeout in seconds", "2", move |s, _, _| {
                                ask_def(s, "Extended commands?", "n", move |s, st, e| {
                                    if !is_y(&e) {
                                        return ping_cmd(s, st, &target, &o);
                                    }
                                    ask_def(s, "Source address or interface", "", move |s, st, src| {
                                        let mut o = o;
                                        if !src.is_empty() {
                                            if U::is_ip(&src) {
                                                o.src = Some(src);
                                            } else {
                                                match iface_ip(s, st, &src) {
                                                    Some(ip) => o.src = Some(ip),
                                                    None => {
                                                        return result("% Invalid source. Must use same-VRF IP address or full interface name without spaces (e.g. Serial0/1)");
                                                    }
                                                }
                                            }
                                        }
                                        ask_def(s, "Type of service", "0", move |s, _, _| ping_step7(s, target, o))
                                    })
                                })
                            })
                        })
                    })
                });
                result("")
            })
        },
        NO_NEG,
    );
    // ping 192.168.2.1 source gi0/0.10 size 1500 df-bit repeat 10
    reg.d("user exec", "ping <iphost> <text>", |s, st, a| {
        let t: Vec<String> = a.s(1).split_whitespace().map(|x| x.to_string()).collect();
        let mut o = PingO::default();
        let mut i = 0;
        while i < t.len() {
            let w = t[i].to_lowercase();
            let is = |full: &str, min: usize| w.len() >= min && full.starts_with(&w);
            let next_digits = |i: usize| t.get(i + 1).map(|x| is_digits(x)).unwrap_or(false);
            if is("repeat", 1) && next_digits(i) {
                i += 1;
                o.count = Some(t[i].parse::<i64>().unwrap_or(100).clamp(1, 100));
                i += 1;
                continue;
            }
            if is("size", 2) && next_digits(i) {
                i += 1;
                let sz = t[i].parse::<i64>().unwrap_or(0);
                if !(36..=18024).contains(&sz) {
                    return result(s.invalid_marker(st, &a.line, &t[i]));
                }
                o.size = Some(sz);
                i += 1;
                continue;
            }
            if is("df-bit", 1) {
                o.df = true;
                i += 1;
                continue;
            }
            if is("timeout", 1) && next_digits(i) {
                i += 2;
                continue;
            }
            if is("source", 2) && t.get(i + 1).is_some() {
                i += 1;
                let sv = t[i].clone();
                if U::is_ip(&sv) {
                    o.src = Some(sv);
                    i += 1;
                    continue;
                }
                match iface_ip(s, st, &sv) {
                    Some(ip) => o.src = Some(ip),
                    None => return result("% Invalid source interface - IP not enabled or interface is down"),
                }
                i += 1;
                continue;
            }
            return result(s.invalid_marker(st, &a.line, &t[i]));
        }
        ping_cmd(s, st, a.s(0), &o)
    });
    reg.d("user exec", "traceroute <iphost>", |s, st, a| trace_cmd(s, st, a.s(0), None));
    reg.d("user exec", "traceroute <iphost> source <if>", |s, st, a| {
        let Some(ip) = iface_ip(s, st, a.s(1)) else {
            return result("% Invalid source interface - IP not enabled or interface is down");
        };
        trace_cmd(s, st, a.s(0), Some(&ip))
    });
}

// ------------------------------------------------------------------ CONFIG

/// parseInt i JS: inledande siffror, annars None
pub fn js_parse_int(s: &str) -> Option<i64> {
    let t = s.trim_start();
    let (neg, rest) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let v: i64 = digits.parse().ok()?;
    Some(if neg { -v } else { v })
}

fn hostname_ok(h: &str) -> bool {
    let mut ch = h.chars();
    match ch.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    h.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && !h.ends_with('-') && h.len() <= 63
}

fn default_hostname(s: &IosSession, st: &State) -> &'static str {
    if s.is_router(st) {
        "Router"
    } else {
        "Switch"
    }
}

fn set_secret(s: &mut IosSession, st: &mut State, a: &Args) -> CmdResult {
    if a.neg {
        let c = cfg_mut(s, st);
        c.enable_secret = None;
        c.enable_plain = None;
        return result("");
    }
    // Sista värdet är lösenordet; i "enable secret <n:0-9> <word>" står typsiffran först.
    // Ett ensamt ensiffrigt tal (t.ex. "enable secret 5") är lösenordet.
    let pw = match a.v.last() {
        Some(super::ios::Val::Num(n)) => n.to_string(),
        Some(v) => v.s().to_string(),
        None => String::new(),
    };
    let min = cfg(s, st).min_pw_len;
    if min > 0 && (pw.chars().count() as i64) < min {
        return result(format!("% Password too short - must be at least {} characters. Password configuration failed", min));
    }
    let h = SH::hash_secret(&pw);
    let c = cfg_mut(s, st);
    c.enable_secret = Some(h);
    c.enable_plain = Some(pw);
    result("")
}

fn set_user(s: &mut IosSession, st: &mut State, a: &Args) -> CmdResult {
    let name = a.s(0).to_string();
    if a.neg {
        cfg_mut(s, st).users.shift_remove(&name);
        return result("");
    }
    let priv_ = if a.has_kw("privilege") { a.n(1) } else { 1 };
    let pw = a.v.last().map(|v| v.s().to_string()).unwrap_or_default();
    let min = cfg(s, st).min_pw_len;
    if min > 0 && (pw.chars().count() as i64) < min {
        return result(format!("% Password too short - must be at least {} characters. Password configuration failed", min));
    }
    let secret = SH::hash_secret(&pw);
    let pw_type = if a.has_kw("password") { Some("password".to_string()) } else { None };
    cfg_mut(s, st).users.insert(name, User { priv_, secret, plain: pw, by_game: false, pw_type });
    result("")
}

fn domain_name(s: &mut IosSession, st: &mut State, a: &Args) -> CmdResult {
    cfg_mut(s, st).domain_name = if a.neg { None } else { Some(a.s(0).to_string()) };
    result("")
}

fn ip_route_cmd(s: &mut IosSession, st: &mut State, a: &Args) -> CmdResult {
    let net = a.s(0).to_string();
    let mask = a.s(1).to_string();
    let nh: Option<String> = if a.v.len() > 2 { Some(a.s(2).to_string()) } else { None };
    let ad = if a.v.len() > 3 { a.n(3) } else { 1 };
    if !U::is_valid_mask(&mask) {
        return result("%Inconsistent address and mask");
    }
    if U::network(&net, &mask) != net {
        return result("%Inconsistent address and mask");
    }
    let via_if: Option<String> = match &nh {
        Some(n) if !U::is_ip(n) => U::norm_if(n),
        _ => None,
    };
    if let Some(n) = &nh {
        if !U::is_ip(n) && via_if.is_none() {
            return result(INVALID);
        }
    }
    if a.neg {
        let c = cfg_mut(s, st);
        let before = c.routes.len();
        c.routes.retain(|r| {
            !(r.net == net
                && r.mask == mask
                && (nh.is_none() || r.nh == nh || (via_if.is_some() && r.iface == via_if)))
        });
        if before == c.routes.len() {
            return result("%No matching route to delete");
        }
        return result("");
    }
    if !s.is_router(st) && !cfg(s, st).ip_routing {
        return result(INVALID);
    }
    let c = cfg_mut(s, st);
    let same = c.routes.iter_mut().find(|r| r.net == net && r.mask == mask && (r.nh == nh || (via_if.is_some() && r.iface == via_if)));
    if let Some(r) = same {
        r.ad = if ad != 1 { Some(ad) } else { None };
        S::touch(st);
        return result("");
    }
    let nr = match &via_if {
        Some(i) => Route { net, mask, nh: None, iface: Some(i.clone()), ad: if ad != 1 { Some(ad) } else { None } },
        None => Route { net, mask, nh, iface: None, ad: if ad != 1 { Some(ad) } else { None } },
    };
    c.routes.push(nr);
    result("")
}

fn reg_config(reg: &mut Reg) {
    reg.def("config", "end", end_cmd, NO_NEG);
    for m in ["if", "vlan", "line", "dhcp", "acl", "isakmp", "tset", "cmap"] {
        reg.def(m, "end", end_cmd, NO_NEG);
        reg.def(
            m,
            "exit",
            |s, _, _| {
                s.leave_sub();
                s.mode = "config".into();
                result("")
            },
            NO_NEG,
        );
    }
    reg.def("config", "exit", end_cmd, NO_NEG);

    reg.d("config", "hostname <word>", |s, st, a| {
        let h = a.s(0);
        if !a.neg && !hostname_ok(h) {
            return result("% Hostname contains one or more illegal characters.");
        }
        let nm = if a.neg { default_hostname(s, st).to_string() } else { h.to_string() };
        cfg_mut(s, st).hostname = nm;
        result("")
    });
    reg.def(
        "config",
        "hostname",
        |s, st, _| {
            let nm = default_hostname(s, st).to_string();
            cfg_mut(s, st).hostname = nm;
            result("")
        },
        NO_ONLY,
    );
    // Ordningen är viktig: parsern provar parametrarna i registreringsordning, så
    // <n:0-9> <word> måste ligga före <word>, annars tolkas typsiffran som lösenord.
    reg.d("config", "enable secret <n:0-9> <word>", set_secret);
    reg.d("config", "enable secret <n:0-9>", set_secret);
    reg.d("config", "enable secret <word>", set_secret);
    reg.def("config", "enable secret", set_secret, NO_ONLY);
    reg.d("config", "enable password <word>", |s, st, a| {
        let c = cfg_mut(s, st);
        c.enable_password = if a.neg { None } else { Some(a.s(0).to_string()) };
        if c.enable_plain.is_none() && !a.neg {
            c.enable_plain = Some(a.s(0).to_string());
        }
        result("")
    });
    reg.d("config", "username <word> secret <word>", set_user);
    reg.d("config", "username <word> privilege <n:0-15> secret <word>", set_user);
    reg.d("config", "username <word> password <word>", set_user);
    reg.d("config", "username <word> privilege <n:0-15> password <word>", set_user);
    reg.def("config", "username <word>", set_user, NO_ONLY);

    reg.d("config", "ip domain-name <word>", domain_name);
    reg.d("config", "ip domain name <word>", domain_name);
    reg.def("config", "ip domain-name", domain_name, NO_ONLY);
    reg.def("config", "ip domain name", domain_name, NO_ONLY);
    reg.d("config", "ip domain-lookup", |s, st, a| {
        cfg_mut(s, st).domain_lookup = !a.neg;
        result("")
    });
    reg.d("config", "ip domain lookup", |s, st, a| {
        cfg_mut(s, st).domain_lookup = !a.neg;
        result("")
    });
    reg.d("config", "ip name-server <text>", |s, st, a| {
        let ips: Vec<String> = a.s(0).split_whitespace().map(|x| x.to_string()).collect();
        for ip in &ips {
            if !U::is_ip(ip) {
                return result(INVALID);
            }
        }
        let c = cfg_mut(s, st);
        if a.neg {
            c.name_servers.retain(|x| !ips.contains(x));
        } else {
            for x in ips {
                if !c.name_servers.contains(&x) {
                    c.name_servers.push(x);
                }
            }
        }
        result("")
    });
    reg.d("config", "ip dns server", |s, st, a| {
        cfg_mut(s, st).dns_server = !a.neg;
        result("")
    });
    reg.d("config", "ip host <word> <ip>", |s, st, a| {
        let k = a.s(0).to_lowercase();
        let c = cfg_mut(s, st);
        if a.neg {
            c.hosts.shift_remove(&k);
        } else {
            c.hosts.insert(k, a.s(1).to_string());
        }
        result("")
    });
    reg.def(
        "config",
        "ip host <word>",
        |s, st, a| {
            cfg_mut(s, st).hosts.shift_remove(&a.s(0).to_lowercase());
            result("")
        },
        NO_ONLY,
    );
    reg.d("config", "ip route <ip> <mask> <word>", ip_route_cmd);
    reg.d("config", "ip route <ip> <mask> <word> <n:1-255>", ip_route_cmd);
    reg.d("config", "ip route <ip> <mask> <if> <n:1-255>", ip_route_cmd);
    reg.d("config", "ip route <ip> <mask> <if>", ip_route_cmd);
    reg.def("config", "ip route <ip> <mask>", ip_route_cmd, NO_ONLY);
    reg.d("config", "ip default-gateway <ip>", |s, st, a| {
        cfg_mut(s, st).default_gateway = if a.neg { None } else { Some(a.s(0).to_string()) };
        result("")
    });
    reg.def(
        "config",
        "ip default-gateway",
        |s, st, _| {
            cfg_mut(s, st).default_gateway = None;
            result("")
        },
        NO_ONLY,
    );
    reg.d("config", "ip routing", |s, st, a| {
        if s.is_router(st) {
            return result("");
        }
        cfg_mut(s, st).ip_routing = !a.neg;
        result("")
    });
    reg.d("config", "ip cef", |_, _, _| result(""));
    reg.d("config", "ip http server", |_, _, _| result(""));
    reg.d("config", "ip http secure-server", |_, _, _| result(""));
    reg.d("config", "service password-encryption", |_, _, _| result(""));
    reg.d("config", "service timestamps log datetime msec", |s, st, a| {
        cfg_mut(s, st).timestamps_msec = !a.neg;
        result("")
    });
    reg.d("config", "service timestamps log uptime", |s, st, _| {
        cfg_mut(s, st).timestamps_msec = false;
        result("")
    });
    reg.d("config", "service timestamps debug datetime msec", |_, _, _| result(""));
    reg.d("config", "cdp run", |_, _, _| result(""));
    reg.d("config", "spanning-tree mode pvst", |_, _, _| result(""));
    reg.d("config", "spanning-tree mode rapid-pvst", |_, _, _| result(""));
    reg.d("config", "spanning-tree extend system-id", |_, _, _| result(""));
}

// ------------------------------------------------------------------ DHCP, ACL, NAT

fn excl(s: &mut IosSession, st: &mut State, a_ip: &str, b_ip: &str, neg: bool) -> CmdResult {
    if !s.is_router(st) {
        return result(INVALID);
    }
    if U::ipn(b_ip) < U::ipn(a_ip) {
        return result("% Invalid address range");
    }
    let c = cfg_mut(s, st);
    if neg {
        c.dhcp_excluded.retain(|x| !(x.0 == a_ip && x.1 == b_ip));
        return result("");
    }
    c.dhcp_excluded.push((a_ip.to_string(), b_ip.to_string()));
    result("")
}

fn pool_mut<'a>(s: &IosSession, st: &'a mut State) -> Option<&'a mut Pool> {
    let name = s.ctx.pool.clone()?;
    cfg_mut(s, st).dhcp_pools.get_mut(&name)
}

/// Rad i en ACL (parseAce i ios.js). Err(i) = index för det ord som är fel. seq 0 = inget angivet.
pub fn parse_ace(extended: bool, text: &str) -> Result<Ace, usize> {
    let t: Vec<String> = text.split_whitespace().map(|x| x.to_string()).collect();
    let g = |i: usize| t.get(i).map(|x| x.as_str()).unwrap_or("");
    let mut i = 0;
    let mut r = Ace::default();
    if is_digits(g(0)) {
        r.seq = g(0).parse().unwrap_or(0);
        i += 1;
    }
    let act = g(i).to_lowercase();
    if act == "remark" {
        r.remark = Some(t[i + 1..].join(" "));
        return Ok(r);
    }
    if !act.is_empty() && "permit".starts_with(&act) {
        r.action = "permit".into();
    } else if !act.is_empty() && "deny".starts_with(&act) {
        r.action = "deny".into();
    } else {
        return Err(i);
    }
    i += 1;
    fn addr(t: &[String], i: &mut usize, extended: bool) -> Option<AddrSpec> {
        let w = t.get(*i).map(|x| x.to_lowercase()).unwrap_or_default();
        if w.is_empty() {
            return None;
        }
        if w == "any" {
            *i += 1;
            return Some(AddrSpec::any());
        }
        if w == "host" {
            let n = t.get(*i + 1)?;
            if !U::is_ip(n) {
                return None;
            }
            *i += 2;
            return Some(AddrSpec::host(&t[*i - 1]));
        }
        if !U::is_ip(&w) {
            return None;
        }
        let nxt = t.get(*i + 1).map(|x| U::is_ip(x)).unwrap_or(false);
        if !extended && !nxt {
            *i += 1;
            return Some(AddrSpec::net(&w, "0.0.0.0"));
        }
        if !nxt {
            return None;
        }
        *i += 2;
        Some(AddrSpec::net(&t[*i - 2], &t[*i - 1]))
    }
    if extended {
        let proto = g(i).to_lowercase();
        if !["ip", "tcp", "udp", "icmp"].contains(&proto.as_str()) {
            return Err(i);
        }
        r.proto = Some(proto);
        i += 1;
    }
    r.src = addr(&t, &mut i, extended);
    if r.src.is_none() {
        return Err(i);
    }
    if extended {
        r.dst = addr(&t, &mut i, extended);
        if r.dst.is_none() {
            return Err(i);
        }
        let opw = g(i).to_lowercase();
        if ["eq", "neq", "gt", "lt", "range"].contains(&opw.as_str()) {
            fn port(w: &str) -> Option<i64> {
                let pn = match w.to_lowercase().as_str() {
                    "ssh" => 22,
                    "telnet" => 23,
                    "www" | "http" => 80,
                    "domain" => 53,
                    "https" => 443,
                    "ntp" => 123,
                    "smtp" => 25,
                    "ftp" => 21,
                    "ftp-data" => 20,
                    "pop3" => 110,
                    "bootps" => 67,
                    "bootpc" => 68,
                    "snmp" => 161,
                    "syslog" => 514,
                    "tftp" => 69,
                    "isakmp" => 500,
                    _ => js_parse_int(w).unwrap_or(0),
                };
                if pn == 0 {
                    None
                } else {
                    Some(pn)
                }
            }
            let pn = match port(g(i + 1)) {
                Some(p) if p <= 65535 => p,
                _ => return Err(i + 1),
            };
            r.dport = Some(pn);
            r.dop = Some(opw.clone());
            i += 2;
            if opw == "range" {
                match port(g(i)) {
                    Some(p2) if p2 >= pn && p2 <= 65535 => r.dport2 = Some(p2),
                    _ => return Err(i),
                }
                i += 1;
            }
            if opw == "eq" {
                r.dop = None;
            }
        }
    }
    while i < t.len() && ["log", "log-input"].contains(&t[i].to_lowercase().as_str()) {
        r.log = true;
        i += 1;
    }
    if i < t.len() {
        return Err(i);
    }
    Ok(r)
}

/// Felmarkör under ord nummer idx i en ACL-rad (aceErr)
pub fn ace_err(s: &IosSession, st: &State, line: &str, text: &str, idx: usize) -> String {
    let toks: Vec<Tok> = super::ios::tokenize(line);
    let nwords = text.split_whitespace().count().max(1) as i64;
    let offset = toks.len() as i64 - nwords + idx as i64;
    let mut pos = if toks.is_empty() || offset < 0 { line.len() } else { toks[(offset as usize).min(toks.len() - 1)].pos };
    if idx as i64 >= nwords {
        pos = line.len() + 1;
    }
    format!("{}^\n{}\n", " ".repeat(s.prompt(st).len() + pos), INVALID)
}

fn acl_mode(s: &mut IosSession, st: &mut State, ext: bool, name: &str, neg: bool) -> CmdResult {
    let c = cfg_mut(s, st);
    if neg {
        c.acls.shift_remove(name);
        return result("");
    }
    if let Some(x) = c.acls.get(name) {
        if x.extended != ext {
            return result("% Access-list type conflicts with prior definition");
        }
    } else {
        c.acls.insert(name.to_string(), Acl { extended: ext, rules: vec![] });
    }
    s.mode = "acl".into();
    s.ctx = Ctx { acl: Some(name.to_string()), acl_extended: ext, ..Default::default() };
    result("")
}

fn norm_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn ace_add(s: &mut IosSession, st: &mut State, a: &Args) -> CmdResult {
    let Some(name) = s.ctx.acl.clone() else { return result("") };
    let ext = s.ctx.acl_extended;
    let text = format!("{} {}", a.kws.join(" "), a.s(0));
    let mut r = match parse_ace(ext, &text) {
        Ok(r) => r,
        Err(i) => return result(ace_err(s, st, &a.line, &text, i)),
    };
    let acl = match cfg_mut(s, st).acls.get_mut(&name) {
        Some(x) => x,
        None => return result(""),
    };
    if a.neg {
        let txt = norm_ws(&SH::ace_text(ext, &r));
        acl.rules.retain(|x| norm_ws(&SH::ace_text(ext, x)) != txt);
        return result("");
    }
    if r.seq == 0 {
        r.seq = next_seq(acl);
    } else if acl.rules.iter().any(|x| x.seq == r.seq) {
        return result("% Duplicate sequence number");
    }
    acl.rules.push(r);
    acl.rules.sort_by_key(|x| x.seq);
    result("")
}

fn reg_dhcp_acl_nat(reg: &mut Reg) {
    // DHCP
    reg.d("config", "ip dhcp excluded-address <ip>", |s, st, a| excl(s, st, a.s(0), a.s(0), a.neg));
    reg.d("config", "ip dhcp excluded-address <ip> <ip>", |s, st, a| excl(s, st, a.s(0), a.s(1), a.neg));
    reg.d("config", "ip dhcp pool <word>", |s, st, a| {
        if !s.is_router(st) {
            return result(INVALID);
        }
        let name = a.s(0).to_string();
        let c = cfg_mut(s, st);
        if a.neg {
            if !c.dhcp_pools.contains_key(&name) {
                return result(format!("%Pool {} does not exist.", name));
            }
            c.dhcp_pools.shift_remove(&name);
            return result("");
        }
        if !c.dhcp_pools.contains_key(&name) {
            c.dhcp_pools.insert(name.clone(), Pool { network: None, mask: None, default_router: vec![], dns: vec![], lease: Some(1.0), opt150: None });
        }
        s.mode = "dhcp".into();
        s.ctx = Ctx { pool: Some(name), ..Default::default() };
        result("")
    });
    reg.d("dhcp", "network <ip> <mask>", |s, st, a| {
        let Some(p) = pool_mut(s, st) else { return result("") };
        if a.neg {
            p.network = None;
            p.mask = None;
            return result("");
        }
        if !U::is_valid_mask(a.s(1)) {
            return result(INVALID);
        }
        let net = U::network(a.s(0), a.s(1));
        if net != a.s(0) {
            return result(format!("% {} / {} is an invalid network.", a.s(0), a.s(1)));
        }
        p.network = Some(net);
        p.mask = Some(a.s(1).to_string());
        result("")
    });
    reg.d("dhcp", "network <ip> <word>", |s, st, a| {
        let w = a.s(1);
        let pfx = w.strip_prefix('/').filter(|x| is_digits(x)).and_then(|x| x.parse::<u32>().ok());
        let Some(pfx) = pfx else { return result(INVALID) };
        let Some(p) = pool_mut(s, st) else { return result("") };
        if a.neg {
            p.network = None;
            p.mask = None;
            return result("");
        }
        let mask = U::prefix_to_mask(pfx);
        p.network = Some(U::network(a.s(0), &mask));
        p.mask = Some(mask);
        result("")
    });
    reg.def(
        "dhcp",
        "network",
        |s, st, _| {
            if let Some(p) = pool_mut(s, st) {
                p.network = None;
                p.mask = None;
            }
            result("")
        },
        NO_ONLY,
    );
    reg.d("dhcp", "default-router <text>", |s, st, a| {
        let Some(p) = pool_mut(s, st) else { return result("") };
        if a.neg {
            p.default_router.clear();
            return result("");
        }
        let ips: Vec<String> = a.s(0).split_whitespace().map(|x| x.to_string()).collect();
        if !ips.iter().all(|x| U::is_ip(x)) {
            return result(INVALID);
        }
        p.default_router = ips.into_iter().take(8).collect();
        result("")
    });
    reg.def(
        "dhcp",
        "default-router",
        |s, st, _| {
            if let Some(p) = pool_mut(s, st) {
                p.default_router.clear();
            }
            result("")
        },
        NO_ONLY,
    );
    reg.d("dhcp", "dns-server <text>", |s, st, a| {
        let Some(p) = pool_mut(s, st) else { return result("") };
        if a.neg {
            p.dns.clear();
            return result("");
        }
        let ips: Vec<String> = a.s(0).split_whitespace().map(|x| x.to_string()).collect();
        if !ips.iter().all(|x| U::is_ip(x)) {
            return result(INVALID);
        }
        p.dns = ips.into_iter().take(8).collect();
        result("")
    });
    reg.def(
        "dhcp",
        "dns-server",
        |s, st, _| {
            if let Some(p) = pool_mut(s, st) {
                p.dns.clear();
            }
            result("")
        },
        NO_ONLY,
    );
    reg.d("dhcp", "lease <n:0-365>", |s, st, a| {
        if let Some(p) = pool_mut(s, st) {
            p.lease = Some(if a.neg { 1.0 } else { a.n(0) as f64 });
        }
        result("")
    });
    reg.d("dhcp", "lease infinite", |s, st, _| {
        if let Some(p) = pool_mut(s, st) {
            p.lease = None;
        }
        result("")
    });
    reg.d("dhcp", "domain-name <word>", |_, _, _| result(""));

    // ACL
    reg.d("config", "ip access-list extended <word>", |s, st, a| acl_mode(s, st, true, a.s(0), a.neg));
    reg.d("config", "ip access-list standard <word>", |s, st, a| acl_mode(s, st, false, a.s(0), a.neg));
    reg.d("acl", "permit <text>", ace_add);
    reg.d("acl", "deny <text>", ace_add);
    reg.d("acl", "remark <text>", ace_add);
    reg.d("acl", "<n:1-2147483647> <text>", |s, st, a| {
        let Some(name) = s.ctx.acl.clone() else { return result("") };
        let ext = s.ctx.acl_extended;
        let text = format!("{} {}", a.n(0), a.s(1));
        let r = match parse_ace(ext, &text) {
            Ok(r) => r,
            Err(i) => return result(ace_err(s, st, &a.line, &text, i)),
        };
        let Some(acl) = cfg_mut(s, st).acls.get_mut(&name) else { return result("") };
        if acl.rules.iter().any(|x| x.seq == r.seq) {
            return result("% Duplicate sequence number");
        }
        acl.rules.push(r);
        acl.rules.sort_by_key(|x| x.seq);
        result("")
    });
    reg.def(
        "acl",
        "<n:1-2147483647>",
        |s, st, a| {
            let Some(name) = s.ctx.acl.clone() else { return result("") };
            if let Some(acl) = cfg_mut(s, st).acls.get_mut(&name) {
                acl.rules.retain(|x| x.seq != a.n(0));
            }
            result("")
        },
        NO_ONLY,
    );
    reg.d("config", "access-list <n:1-199> <text>", |s, st, a| {
        let name = a.n(0).to_string();
        let ext = a.n(0) >= 100;
        let mut r = match parse_ace(ext, a.s(1)) {
            Ok(r) => r,
            Err(i) => return result(ace_err(s, st, &a.line, a.s(1), i)),
        };
        let c = cfg_mut(s, st);
        if a.neg {
            c.acls.shift_remove(&name);
            return result("");
        }
        let acl = c.acls.entry(name).or_insert_with(|| Acl { extended: ext, rules: vec![] });
        r.seq = next_seq(acl);
        acl.rules.push(r);
        result("")
    });
    reg.def(
        "config",
        "access-list <n:1-199>",
        |s, st, a| {
            cfg_mut(s, st).acls.shift_remove(&a.n(0).to_string());
            result("")
        },
        NO_ONLY,
    );

    // NAT
    reg.d("config", "ip nat inside source static <ip> <ip>", |s, st, a| {
        let n = &mut cfg_mut(s, st).nat;
        let (l, g) = (a.s(0).to_string(), a.s(1).to_string());
        if a.neg {
            n.statics.retain(|x| !(x.local == l && x.global == g));
            return result("");
        }
        if let Some(x) = n.statics.iter().find(|x| x.local == l) {
            return result(format!("% {} already mapped ({} -> {})", l, l, x.global));
        }
        n.statics.push(NatStatic { local: l, global: g });
        result("")
    });
    fn nat_list(s: &mut IosSession, st: &mut State, a: &Args) -> CmdResult {
        let ov = a.has_kw("overload");
        let acl = a.s(0).to_string();
        let ifn = a.s(1).to_string();
        if !if_exists(s, st, &ifn) {
            return result(INVALID);
        }
        if a.neg {
            cfg_mut(s, st).nat.dynamic.retain(|x| !(x.acl == acl && x.iface == ifn));
            st.dev_mut(&s.dev).rt.nat_trans.clear();
            return result("");
        }
        let n = &mut cfg_mut(s, st).nat;
        n.dynamic.retain(|x| x.acl != acl);
        n.dynamic.push(NatDyn { acl, iface: ifn, overload: ov });
        result("")
    }
    reg.d("config", "ip nat inside source list <word> interface <if> overload", nat_list);
    reg.d("config", "ip nat inside source list <word> interface <if>", nat_list);
}

// ------------------------------------------------------------------ SSH, nycklar, linjer, VLAN, STP, loggning, NTP

const KEY_INTRO: &str = "Choose the size of the key modulus in the range of 360 to 4096 for your\n  General Purpose Keys. Choosing a key modulus greater than 512 may take\n  a few minutes.\n";

fn rsa_make(s: &mut IosSession, st: &mut State, b: i64) -> CmdResult {
    if !(360..=4096).contains(&b) {
        return result("% A decimal number between 360 and 4096.");
    }
    let c = cfg_mut(s, st);
    c.crypto_key = Some(b);
    let ver = if c.ssh.version == Some(2) { "2.0" } else { "1.99" };
    S::push_log(st, &s.dev, &format!("%SSH-5-ENABLED: SSH {} has been enabled", ver));
    CmdResult::delayed(
        format!(
            "% Generating {} bit RSA keys, keys will be non-exportable...\n[OK] (elapsed time was {} seconds)\n",
            b,
            ((b as f64 / 1024.0).round() as i64).max(1)
        ),
        800,
    )
}

fn rsa_ask_bits(s: &mut IosSession) {
    s.ask("How many bits in the modulus [512]: ", false, |s, st, a| rsa_make(s, st, js_parse_int(a).filter(|x| *x != 0).unwrap_or(512)));
}

fn gen_key(s: &mut IosSession, st: &mut State, bits: Option<i64>) -> CmdResult {
    let c = cfg(s, st);
    let host = c.hostname.clone();
    if host.is_empty() || host == "Router" || host == "Switch" {
        return result(format!("% Please define a hostname other than {}.", host));
    }
    let Some(dom) = c.domain_name.clone() else {
        return result("% Please define a domain-name first.");
    };
    let has_key = c.crypto_key.is_some();
    let intro = format!("The name for the keys will be: {}.{}\n", host, dom);
    if has_key {
        s.ask(
            format!("% You already have RSA keys defined named {}.{}.\n% Do you really want to replace them? [yes/no]: ", host, dom),
            false,
            move |s, st, a| {
                if !is_y(a) {
                    return result("");
                }
                if let Some(b) = bits {
                    return rsa_make(s, st, b);
                }
                rsa_ask_bits(s);
                result(format!("Choose the size of the key modulus in the range of 360 to 4096 for your\n  General Purpose Keys. Choosing a key modulus greater than 512 may take\n  a few minutes.\n"))
            },
        );
        return result("");
    }
    if let Some(b) = bits {
        // Texten slås ihop med intron, men fördröjningen (800 ms) från rsa_make() behålls
        let r = rsa_make(s, st, b);
        return CmdResult { out: format!("{}{}", intro, r.out), ..r };
    }
    rsa_ask_bits(s);
    result(format!("{}{}", intro, KEY_INTRO))
}

fn line_mode(s: &mut IosSession, names: Vec<&str>) -> CmdResult {
    s.mode = "line".into();
    s.ctx = Ctx { lines: names.into_iter().map(|x| x.to_string()).collect(), ..Default::default() };
    result("")
}

/// Kör f för varje linje i sammanhanget (eachLine)
fn each_line<F: FnMut(&mut Line, &str)>(s: &IosSession, st: &mut State, mut f: F) {
    for n in s.ctx.lines.clone() {
        f(cfg_mut(s, st).lines.get_mut(&n), &n);
    }
}

fn vlan_default_names(c: &mut IosConfig, list: &[u16]) {
    for x in list {
        c.vlans.entry(*x).or_insert_with(|| default_vlan_name(*x));
    }
}

fn reg_ssh_line_vlan(reg: &mut Reg) {
    // SSH och nycklar
    reg.d("config", "ip ssh version <n:1-2>", |s, st, a| {
        let c = cfg_mut(s, st);
        c.ssh.version = if a.neg { None } else { Some(a.n(0)) };
        if !a.neg && c.crypto_key.is_none() {
            return result("Please create RSA keys to enable SSH (and of atleast 768 bits for SSH v2).");
        }
        result("")
    });
    reg.def(
        "config",
        "ip ssh version",
        |s, st, _| {
            cfg_mut(s, st).ssh.version = None;
            result("")
        },
        NO_ONLY,
    );
    reg.def("config", "crypto key generate rsa", |s, st, _| gen_key(s, st, None), NO_NEG);
    reg.def("config", "crypto key generate rsa general-keys", |s, st, _| gen_key(s, st, None), NO_NEG);
    reg.def("config", "crypto key generate rsa modulus <n:360-4096>", |s, st, a| gen_key(s, st, Some(a.n(0))), NO_NEG);
    reg.def("config", "crypto key generate rsa general-keys modulus <n:360-4096>", |s, st, a| gen_key(s, st, Some(a.n(0))), NO_NEG);
    reg.def(
        "config",
        "crypto key zeroize rsa",
        |s, _, _| {
            s.ask(
                "% All keys will be removed.\n% All router certs issued using these keys will also be removed.\nDo you really want to remove these keys? [yes/no]: ",
                false,
                |s, st, a| {
                    if !is_y(a) {
                        return result("");
                    }
                    cfg_mut(s, st).crypto_key = None;
                    S::push_log(st, &s.dev, "%SSH-5-DISABLED: SSH 2.0 has been disabled");
                    result("")
                },
            );
            result("")
        },
        NO_NEG,
    );

    // Linjer
    reg.def("config", "line con <n:0-0>", |s, _, _| line_mode(s, vec!["con"]), NO_NEG);
    reg.def("config", "line console <n:0-0>", |s, _, _| line_mode(s, vec!["con"]), NO_NEG);
    reg.def(
        "config",
        "line vty <n:0-15> <n:0-15>",
        |s, _, a| {
            let (lo, hi) = (a.n(0), a.n(1));
            let mut names = vec![];
            if lo <= 4 {
                names.push("vty0_4");
            }
            if hi >= 5 {
                names.push("vty5_15");
            }
            line_mode(s, names)
        },
        NO_NEG,
    );
    reg.def("config", "line vty <n:0-15>", |s, _, a| line_mode(s, vec![if a.n(0) <= 4 { "vty0_4" } else { "vty5_15" }]), NO_NEG);
    reg.def("config", "line aux <n:0-0>", |s, _, _| line_mode(s, vec![]), NO_NEG);
    reg.d("line", "transport input <text>", |s, st, a| {
        let w: Vec<String> = a.s(0).to_lowercase().split_whitespace().map(|x| x.to_string()).collect();
        let ok = w.iter().all(|x| ["ssh", "telnet", "all", "none"].contains(&x.as_str()));
        if !ok {
            return result(INVALID);
        }
        let val = if w.len() == 2 { "telnet ssh".to_string() } else { w.first().cloned().unwrap_or_default() };
        let neg = a.neg;
        each_line(s, st, |l, n| {
            if n != "con" {
                l.transport = if neg { "all".into() } else { val.clone() };
            }
        });
        result("")
    });
    reg.def(
        "line",
        "transport input",
        |s, st, _| {
            each_line(s, st, |l, _| l.transport = "all".into());
            result("")
        },
        NO_ONLY,
    );
    reg.d("line", "login local", |s, st, a| {
        let neg = a.neg;
        each_line(s, st, |l, _| l.login_local = !neg);
        result("")
    });
    reg.d("line", "login", |s, st, a| {
        let neg = a.neg;
        each_line(s, st, |l, _| {
            l.login_local = false;
            if neg {
                l.password = None;
            }
        });
        result("")
    });
    reg.d("line", "password <word>", |s, st, a| {
        let neg = a.neg;
        let pw = a.s(0).to_string();
        each_line(s, st, |l, _| l.password = if neg { None } else { Some(pw.clone()) });
        result("")
    });
    reg.def(
        "line",
        "password",
        |s, st, _| {
            each_line(s, st, |l, _| l.password = None);
            result("")
        },
        NO_ONLY,
    );
    reg.d("line", "logging synchronous", |s, st, a| {
        let neg = a.neg;
        each_line(s, st, |l, _| l.log_sync = !neg);
        result("")
    });
    reg.d("line", "exec-timeout <n:0-35791> <n:0-2147483>", |_, _, _| result(""));
    reg.d("line", "exec-timeout <n:0-35791>", |_, _, _| result(""));
    reg.d("line", "speed <n:1200-115200>", |s, st, a| {
        let v = a.n(0);
        if ![1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200].contains(&v) {
            return result(INVALID);
        }
        let neg = a.neg;
        each_line(s, st, |l, n| {
            if n == "con" {
                l.speed = if neg { 9600 } else { v as u32 };
            }
        });
        // NV.onConsoleSpeed hoppas över: spelet sköter det
        result("")
    });

    // VLAN
    reg.d("config", "vlan <vlist>", |s, st, a| {
        if !s.is_switch(st) {
            return result(INVALID);
        }
        if a.v[0].is_all() {
            return result(INVALID);
        }
        let list = a.v[0].vlans();
        let c = cfg_mut(s, st);
        if a.neg {
            for x in &list {
                if *x != 1 {
                    c.vlans.remove(x);
                }
            }
            if list.contains(&1) {
                return result("%Default VLAN 1 may not be deleted.");
            }
            return result("");
        }
        for x in &list {
            if (1002..=1005).contains(x) {
                return result(format!("%Default VLAN {} may not be modified.", x));
            }
        }
        vlan_default_names(c, &list);
        s.mode = "vlan".into();
        s.ctx = Ctx { vlans: list, ..Default::default() };
        result("")
    });
    reg.d("vlan", "name <word>", |s, st, a| {
        let list = s.ctx.vlans.clone();
        let c = cfg_mut(s, st);
        for x in list {
            c.vlans.insert(x, if a.neg { default_vlan_name(x) } else { a.s(0).to_string() });
        }
        result("")
    });
    reg.d("config", "spanning-tree vlan <vlist> priority <n:0-61440>", |s, st, a| {
        let pr = a.n(1);
        if !a.neg && pr % 4096 != 0 {
            return result("% Bridge Priority must be in increments of 4096.\n% Allowed values are:\n  0     4096  8192  12288 16384 20480 24576 28672\n  32768 36864 40960 45056 49152 53248 57344 61440");
        }
        let c = cfg_mut(s, st);
        // "all" = alla VLAN som finns i switchen (samma tolkning som "spanning-tree vlan all" nedan)
        let list: Vec<u16> = if a.v[0].is_all() { c.vlans.keys().copied().collect() } else { a.v[0].vlans() };
        for x in list {
            if a.neg {
                c.stp_priority.remove(&x);
            } else {
                c.stp_priority.insert(x, pr as u32);
            }
        }
        result("")
    });
    reg.d("config", "spanning-tree vlan <vlist>", |s, st, a| {
        let c = cfg_mut(s, st);
        let list: Vec<u16> = if a.v[0].is_all() { c.vlans.keys().copied().collect() } else { a.v[0].vlans() };
        for x in list {
            let pos = c.stp_off.iter().position(|y| *y == x);
            if a.neg && pos.is_none() {
                c.stp_off.push(x);
            }
            if !a.neg {
                if let Some(i) = pos {
                    c.stp_off.remove(i);
                }
            }
        }
        result("")
    });

    // Loggning och NTP
    reg.d("config", "logging buffered", |s, st, a| {
        cfg_mut(s, st).logging.buffered = !a.neg;
        result("")
    });
    reg.d("config", "logging buffered <n:4096-2147483647>", |s, st, a| {
        let l = &mut cfg_mut(s, st).logging;
        l.buffered = !a.neg;
        if !a.neg {
            l.size = a.n(0);
        }
        result("")
    });
    fn log_host(s: &mut IosSession, st: &mut State, a: &Args) -> CmdResult {
        let l = &mut cfg_mut(s, st).logging;
        let ip = a.s(0).to_string();
        if a.neg {
            l.hosts.retain(|h| *h != ip);
        } else if !l.hosts.contains(&ip) {
            l.hosts.push(ip);
        }
        result("")
    }
    reg.d("config", "logging host <ip>", log_host);
    reg.d("config", "logging <ip>", log_host);
    reg.d("config", "ntp server <ip>", |s, st, a| {
        let c = cfg_mut(s, st);
        let ip = a.s(0).to_string();
        if a.neg {
            c.ntp_servers.retain(|x| *x != ip);
        } else if !c.ntp_servers.contains(&ip) {
            c.ntp_servers.push(ip);
        }
        S::touch(st);
        result("")
    });
    reg.d("config", "clock timezone <word> <n:0-23>", |_, _, _| result(""));
    reg.d("config", "ntp source <if>", |s, st, a| {
        if !a.neg && !if_exists(s, st, a.s(0)) {
            return result(s.invalid_marker(st, &a.line, a.s(0)));
        }
        cfg_mut(s, st).ntp_source = if a.neg { None } else { Some(a.s(0).to_string()) };
        S::touch(st);
        result("")
    });
    reg.def(
        "config",
        "ntp source",
        |s, st, _| {
            cfg_mut(s, st).ntp_source = None;
            S::touch(st);
            result("")
        },
        NO_ONLY,
    );
}

// ------------------------------------------------------------------ INTERFACE

/// Som l2() i ios.js: portar som inte är switchportar svarar med felmarkör
fn l2<F>(s: &IosSession, st: &mut State, line: &str, word: &str, mut f: F) -> String
where
    F: FnMut(&mut Iface, &str) -> Option<String>,
{
    let sw = s.is_switch(st);
    let marker = s.invalid_marker(st, line, word);
    each_if(s, st, |i, n| {
        if kind_of(sw, i) != "l2" {
            return Some(marker.clone());
        }
        f(i, n)
    })
}

/// Samma men felmarkör om interfacet är L2 (för nat, access-group m.m.)
fn not_l2<F>(s: &IosSession, st: &mut State, line: &str, word: &str, mut f: F) -> String
where
    F: FnMut(&mut Iface, &str) -> Option<String>,
{
    let sw = s.is_switch(st);
    let marker = s.invalid_marker(st, line, word);
    each_if(s, st, |i, n| {
        if kind_of(sw, i) == "l2" {
            return Some(marker.clone());
        }
        f(i, n)
    })
}

fn all_vlans_except(except: &[u16]) -> Vec<u16> {
    (1..=4094u16).filter(|x| !except.contains(x)).collect()
}

fn ip_address_one(c: &mut IosConfig, sw: bool, n: &str, ip: &str, mask: &str, neg: bool, marker: &str) -> Option<String> {
    let (k, has_parent, has_dot1q) = {
        let i = c.ifaces.get(n)?;
        (kind_of(sw, i), i.parent.is_some(), i.dot1q.is_some())
    };
    if k == "l2" {
        return Some(marker.to_string());
    }
    if neg {
        c.ifaces.get_mut(n)?.ip = None;
        return None;
    }
    if !U::is_valid_mask(mask) {
        return Some(format!("Bad mask /{} for address {}", mask, ip));
    }
    let net = U::network(ip, mask);
    let pfx = U::mask_to_prefix(mask).unwrap_or(0);
    if net == ip && pfx < 31 {
        return Some(format!("Bad mask /{} for address {}", pfx, ip));
    }
    if U::broadcast(ip, mask) == ip && pfx < 31 {
        return Some(format!("Bad mask /{} for address {}", pfx, ip));
    }
    if has_parent && !has_dot1q {
        return Some("% Configuring IP routing on a LAN subinterface is only allowed if that\nsubinterface is already configured as part of an IEEE 802.10, IEEE 802.1Q,\nor ISL vLAN.\n".into());
    }
    let clash = c
        .ifaces
        .iter()
        .find(|(o, x)| {
            o.as_str() != n && x.ip.as_ref().map(|xi| U::same_subnet(&xi.addr, ip, mask) || U::same_subnet(ip, &xi.addr, &xi.mask)).unwrap_or(false)
        })
        .map(|(o, _)| o.clone());
    if let Some(o) = clash {
        return Some(format!("% {} overlaps with {}", net, o));
    }
    c.ifaces.get_mut(n)?.ip = Some(IpAddr { addr: ip.to_string(), mask: mask.to_string() });
    None
}

fn reg_interface(reg: &mut Reg) {
    reg.def(
        "config",
        "interface <if>",
        |s, st, a| {
            let n = a.s(0).to_string();
            let line = a.line.clone();
            let sub = n.rfind('.').map(|p| is_digits(&n[p + 1..])).unwrap_or(false);
            let svi: Option<u16> = n.strip_prefix("Vlan").filter(|x| is_digits(x)).and_then(|x| x.parse::<u32>().ok().map(|v| v.min(65535) as u16));
            let svi_raw: Option<String> = n.strip_prefix("Vlan").filter(|x| is_digits(x)).map(|x| x.to_string());
            let exists = cfg(s, st).ifaces.contains_key(&n);
            if a.neg {
                if sub && exists {
                    cfg_mut(s, st).ifaces.shift_remove(&n);
                    return result("");
                }
                if svi.is_some() && exists && n != "Vlan1" {
                    cfg_mut(s, st).ifaces.shift_remove(&n);
                    return result("");
                }
                if n.starts_with("Loopback") && exists {
                    cfg_mut(s, st).ifaces.shift_remove(&n);
                    S::touch(st);
                    return result("");
                }
                let w = line.split_whitespace().skip(2).collect::<Vec<_>>().join(" ");
                let w = if w.is_empty() { "interface".to_string() } else { w };
                return result(s.invalid_marker(st, &line, &w));
            }
            if !exists {
                let parent = n.split('.').next().unwrap_or("").to_string();
                let loop_num: Option<&str> = n.strip_prefix("Loopback").filter(|x| is_digits(x));
                if sub && s.is_router(st) && cfg(s, st).ifaces.contains_key(&parent) {
                    cfg_mut(s, st).ifaces.insert(n.clone(), Iface::sub(&parent));
                } else if loop_num.map(|x| x.parse::<u64>().map(|v| v <= 2147483647).unwrap_or(false)).unwrap_or(false) {
                    cfg_mut(s, st).ifaces.insert(n.clone(), Iface::loopback());
                    S::touch(st);
                } else if svi.is_some() && s.is_switch(st) {
                    let vid: u32 = svi_raw.as_deref().unwrap_or("0").parse().unwrap_or(0);
                    if vid < 1 || vid > 4094 {
                        return result(s.invalid_marker(st, &line, svi_raw.as_deref().unwrap_or("")));
                    }
                    cfg_mut(s, st).ifaces.insert(n.clone(), Iface::svi(vid as u16));
                } else {
                    let w = line.split_whitespace().skip(1).collect::<Vec<_>>().join(" ");
                    return result(s.invalid_marker(st, &line, &w));
                }
            }
            s.mode = "if".into();
            s.ctx = Ctx { ifs: vec![n], sub, ..Default::default() };
            result("")
        },
        DefOpts { no: true, no_only: false },
    );
    reg.def(
        "config",
        "interface range <text>",
        |s, st, a| {
            let list = U::expand_range(a.s(0));
            let ok = match &list {
                Some(l) => l.iter().all(|n| cfg(s, st).ifaces.get(n).map(|i| i.parent.is_none()).unwrap_or(false)),
                None => false,
            };
            if !ok {
                return result(s.invalid_marker(st, &a.line, a.s(0)));
            }
            s.mode = "if".into();
            s.ctx = Ctx { ifs: list.unwrap(), range: true, ..Default::default() };
            result("")
        },
        NO_NEG,
    );

    reg.d("if", "description <text>", |s, st, a| {
        let d = if a.neg { String::new() } else { a.s(0).to_string() };
        result(each_if(s, st, |i, _| {
            i.description = d.clone();
            None
        }))
    });
    reg.def(
        "if",
        "description",
        |s, st, _| {
            result(each_if(s, st, |i, _| {
                i.description.clear();
                None
            }))
        },
        NO_ONLY,
    );
    reg.d("if", "shutdown", |s, st, a| {
        let neg = a.neg;
        result(each_if_dev(s, st, |d, n| {
            if let Some(i) = d.cfg_mut().ifaces.get_mut(n) {
                i.shutdown = !neg;
            }
            if neg {
                d.rt.errdisabled.shift_remove(n);
            }
            None
        }))
    });
    reg.d("if", "ip address <ip> <mask>", |s, st, a| {
        let sw = s.is_switch(st);
        let marker = s.invalid_marker(st, &a.line, "address");
        let mut outs: Vec<String> = vec![];
        for n in s.ctx.ifs.clone() {
            if let Some(r) = ip_address_one(cfg_mut(s, st), sw, &n, a.s(0), a.s(1), a.neg, &marker) {
                if !outs.contains(&r) {
                    outs.push(r);
                }
            }
        }
        result(outs.join("\n"))
    });
    reg.def(
        "if",
        "ip address",
        |s, st, _| {
            result(each_if(s, st, |i, _| {
                i.ip = None;
                None
            }))
        },
        NO_ONLY,
    );
    reg.d("if", "switchport mode access", |s, st, a| {
        let neg = a.neg;
        result(l2(s, st, &a.line, "switchport", |i, _| {
            i.mode = if neg { "dynamic auto" } else { "access" }.into();
            None
        }))
    });
    reg.d("if", "switchport mode trunk", |s, st, a| {
        let neg = a.neg;
        result(l2(s, st, &a.line, "switchport", |i, _| {
            if neg {
                i.mode = "dynamic auto".into();
                return None;
            }
            if i.encap.as_deref() != Some("dot1q") {
                return Some("Command rejected: An interface whose trunk encapsulation is \"Auto\" can not be configured to \"trunk\" mode.".into());
            }
            i.mode = "trunk".into();
            None
        }))
    });
    reg.d("if", "switchport mode dynamic auto", |s, st, a| {
        result(l2(s, st, &a.line, "switchport", |i, _| {
            if i.nonegotiate {
                return Some("Command rejected: Conflict between 'nonegotiate' and 'dynamic' status.".into());
            }
            i.mode = "dynamic auto".into();
            None
        }))
    });
    reg.d("if", "switchport mode dynamic desirable", |s, st, a| {
        let neg = a.neg;
        result(l2(s, st, &a.line, "switchport", |i, _| {
            if i.nonegotiate {
                return Some("Command rejected: Conflict between 'nonegotiate' and 'dynamic' status.".into());
            }
            i.mode = if neg { "dynamic auto" } else { "dynamic desirable" }.into();
            None
        }))
    });
    reg.def(
        "if",
        "switchport mode",
        |s, st, a| {
            result(l2(s, st, &a.line, "switchport", |i, _| {
                i.mode = "dynamic auto".into();
                None
            }))
        },
        NO_ONLY,
    );
    reg.d("if", "switchport access vlan <n:1-4094>", |s, st, a| {
        let v = a.n(0) as u16;
        let neg = a.neg;
        let mut touched = false;
        let out = l2(s, st, &a.line, "switchport", |i, _| {
            if neg {
                i.access_vlan = 1;
                return None;
            }
            touched = true;
            i.access_vlan = v;
            None
        });
        if !out.is_empty() {
            return result(out);
        }
        let mut msg = String::new();
        if touched && !cfg(s, st).vlans.contains_key(&v) {
            cfg_mut(s, st).vlans.insert(v, default_vlan_name(v));
            msg = format!("% Access VLAN does not exist. Creating vlan {}", v);
        }
        result(msg)
    });
    reg.def(
        "if",
        "switchport access vlan",
        |s, st, a| {
            result(l2(s, st, &a.line, "switchport", |i, _| {
                i.access_vlan = 1;
                None
            }))
        },
        NO_ONLY,
    );
    reg.d("if", "switchport trunk encapsulation dot1q", |s, st, a| {
        let neg = a.neg;
        result(l2(s, st, &a.line, "switchport", |i, _| {
            if neg && i.mode == "trunk" {
                return Some("Command rejected: Encapsulation is \"Auto\" and mode is \"trunk\".".into());
            }
            i.encap = if neg { None } else { Some("dot1q".into()) };
            None
        }))
    });
    reg.d("if", "switchport trunk encapsulation negotiate", |s, st, a| {
        result(l2(s, st, &a.line, "switchport", |i, _| {
            if i.mode == "trunk" {
                return Some("Command rejected: An interface whose trunk encapsulation is \"Auto\" can not be configured to \"trunk\" mode.".into());
            }
            i.encap = None;
            None
        }))
    });
    reg.d("if", "switchport trunk native vlan <n:1-4094>", |s, st, a| {
        let (neg, v) = (a.neg, a.n(0) as u16);
        result(l2(s, st, &a.line, "switchport", |i, _| {
            i.native = if neg { 1 } else { v };
            None
        }))
    });
    reg.def(
        "if",
        "switchport trunk native vlan",
        |s, st, a| {
            result(l2(s, st, &a.line, "switchport", |i, _| {
                i.native = 1;
                None
            }))
        },
        NO_ONLY,
    );
    reg.d("if", "switchport trunk allowed vlan <vlist>", |s, st, a| {
        let neg = a.neg;
        let v = a.v[0].vl();
        result(l2(s, st, &a.line, "switchport", |i, _| {
            i.allowed = Some(if neg { VlanList::All } else { v.clone() });
            None
        }))
    });
    reg.d("if", "switchport trunk allowed vlan all", |s, st, a| {
        result(l2(s, st, &a.line, "switchport", |i, _| {
            i.allowed = Some(VlanList::All);
            None
        }))
    });
    reg.d("if", "switchport trunk allowed vlan none", |s, st, a| {
        result(l2(s, st, &a.line, "switchport", |i, _| {
            i.allowed = Some(VlanList::List(vec![]));
            None
        }))
    });
    reg.d("if", "switchport trunk allowed vlan add <vlist>", |s, st, a| {
        let v = a.v[0].vlans();
        result(l2(s, st, &a.line, "switchport", |i, _| {
            if let Some(VlanList::List(l)) = &mut i.allowed {
                for x in &v {
                    if !l.contains(x) {
                        l.push(*x);
                    }
                }
                l.sort();
            }
            None
        }))
    });
    reg.d("if", "switchport trunk allowed vlan remove <vlist>", |s, st, a| {
        let v = a.v[0].vlans();
        result(l2(s, st, &a.line, "switchport", |i, _| {
            match &mut i.allowed {
                Some(VlanList::List(l)) => l.retain(|x| !v.contains(x)),
                _ => i.allowed = Some(VlanList::List(all_vlans_except(&v))),
            }
            None
        }))
    });
    reg.d("if", "switchport trunk allowed vlan except <vlist>", |s, st, a| {
        let v = a.v[0].vlans();
        result(l2(s, st, &a.line, "switchport", |i, _| {
            i.allowed = Some(VlanList::List(all_vlans_except(&v)));
            None
        }))
    });
    reg.def(
        "if",
        "switchport trunk allowed vlan",
        |s, st, a| {
            result(l2(s, st, &a.line, "switchport", |i, _| {
                i.allowed = Some(VlanList::All);
                None
            }))
        },
        NO_ONLY,
    );
    reg.d("if", "switchport nonegotiate", |s, st, a| {
        let neg = a.neg;
        result(l2(s, st, &a.line, "switchport", |i, _| {
            if !neg && i.mode != "access" && i.mode != "trunk" {
                return Some("Command rejected: Conflict between 'nonegotiate' and 'dynamic' status.".into());
            }
            i.nonegotiate = !neg;
            None
        }))
    });
    reg.d("if", "switchport", |s, st, a| {
        let router = s.is_router(st);
        let marker = s.invalid_marker(st, &a.line, "switchport");
        let neg = a.neg;
        result(each_if(s, st, |_, _| {
            if router {
                return Some(marker.clone());
            }
            if neg {
                return Some("% Routed ports are not used in this lab. Keep the port as a switchport.".into());
            }
            None
        }))
    });
    reg.d("if", "switchport port-security", |s, st, a| {
        let neg = a.neg;
        result(l2(s, st, &a.line, "switchport", |i, n| {
            if neg {
                i.port_sec = None;
                return None;
            }
            if i.mode != "access" {
                return Some(format!("Command rejected: {} is a dynamic port.", n));
            }
            if i.port_sec.is_none() {
                i.port_sec = Some(PortSec { max: 1, violation: "shutdown".into(), sticky: false, macs: vec![], disabled: false });
            }
            None
        }))
    });
    fn psec<F>(s: &IosSession, st: &mut State, line: &str, mut f: F) -> CmdResult
    where
        F: FnMut(&mut PortSec) + 'static,
    {
        result(l2(s, st, line, "switchport", |i, n| {
            if i.mode != "access" {
                return Some(format!("Command rejected: {} is a dynamic port.", n));
            }
            if i.port_sec.is_none() {
                i.port_sec = Some(PortSec { max: 1, violation: "shutdown".into(), sticky: false, macs: vec![], disabled: true });
            }
            f(i.port_sec.as_mut().unwrap());
            None
        }))
    }
    reg.d("if", "switchport port-security maximum <n:1-132>", |s, st, a| {
        let (neg, v) = (a.neg, a.n(0) as u32);
        psec(s, st, &a.line, move |p| p.max = if neg { 1 } else { v })
    });
    reg.d("if", "switchport port-security violation shutdown", |s, st, a| psec(s, st, &a.line, |p| p.violation = "shutdown".into()));
    reg.d("if", "switchport port-security violation restrict", |s, st, a| {
        let neg = a.neg;
        psec(s, st, &a.line, move |p| p.violation = if neg { "shutdown" } else { "restrict" }.into())
    });
    reg.d("if", "switchport port-security violation protect", |s, st, a| {
        let neg = a.neg;
        psec(s, st, &a.line, move |p| p.violation = if neg { "shutdown" } else { "protect" }.into())
    });
    reg.d("if", "switchport port-security mac-address sticky", |s, st, a| {
        let neg = a.neg;
        psec(s, st, &a.line, move |p| {
            p.sticky = !neg;
            if neg {
                p.macs.clear();
            }
        })
    });
    reg.d("if", "switchport port-security mac-address sticky <word>", |s, st, a| {
        let mac: String = a.s(0).chars().filter(|c| !matches!(c, '.' | ':' | '-')).collect::<String>().to_lowercase();
        let neg = a.neg;
        psec(s, st, &a.line, move |p| {
            if neg {
                p.macs.retain(|m| *m != mac);
            } else if !p.macs.contains(&mac) {
                p.macs.push(mac.clone());
            }
        })
    });
    reg.d("if", "speed <word>", |s, st, a| {
        let val = if a.neg { "auto".to_string() } else { a.s(0).to_string() };
        if !["10", "100", "1000", "auto"].contains(&val.as_str()) {
            return result(s.invalid_marker(st, &a.line, a.s(0)));
        }
        let marker = s.invalid_marker(st, &a.line, "speed");
        result(each_if(s, st, |i, _| {
            if i.svi.is_some() || i.parent.is_some() {
                return Some(marker.clone());
            }
            i.speed = val.clone();
            None
        }))
    });
    reg.def(
        "if",
        "speed",
        |s, st, _| {
            result(each_if(s, st, |i, _| {
                i.speed = "auto".into();
                None
            }))
        },
        NO_ONLY,
    );
    reg.d("if", "duplex <word>", |s, st, a| {
        let val = if a.neg { "auto".to_string() } else { a.s(0).to_lowercase() };
        let full = ["full", "half", "auto"].iter().find(|x| x.starts_with(val.as_str())).map(|x| x.to_string());
        let Some(full) = full else { return result(s.invalid_marker(st, &a.line, a.s(0))) };
        let marker = s.invalid_marker(st, &a.line, "duplex");
        result(each_if(s, st, |i, _| {
            if i.svi.is_some() || i.parent.is_some() {
                return Some(marker.clone());
            }
            i.duplex = full.clone();
            None
        }))
    });
    reg.def(
        "if",
        "duplex",
        |s, st, _| {
            result(each_if(s, st, |i, _| {
                i.duplex = "auto".into();
                None
            }))
        },
        NO_ONLY,
    );
    reg.d("if", "power inline auto", |s, st, a| {
        result(l2(s, st, &a.line, "power", |i, _| {
            i.poe = "auto".into();
            None
        }))
    });
    reg.d("if", "power inline never", |s, st, a| {
        let neg = a.neg;
        result(l2(s, st, &a.line, "power", |i, _| {
            i.poe = if neg { "auto" } else { "never" }.into();
            None
        }))
    });
    reg.d("if", "spanning-tree portfast", |s, st, a| {
        let neg = a.neg;
        let out = l2(s, st, &a.line, "spanning-tree", |i, _| {
            i.portfast = !neg;
            None
        });
        if !out.is_empty() || neg {
            return result(out);
        }
        result("%Warning: portfast should only be enabled on ports connected to a single\n host. Connecting hubs, concentrators, switches, bridges, etc... to this\n interface  when portfast is enabled, can cause temporary bridging loops.\n Use with CAUTION")
    });
    reg.d("if", "spanning-tree portfast trunk", |s, st, a| {
        let neg = a.neg;
        result(l2(s, st, &a.line, "spanning-tree", |i, _| {
            i.portfast = !neg;
            None
        }))
    });
    reg.d("if", "encapsulation dot1q <n:1-4094>", |s, st, a| {
        let marker = s.invalid_marker(st, &a.line, "encapsulation");
        let vid = a.n(0) as u16;
        let mut outs: Vec<String> = vec![];
        for n in s.ctx.ifs.clone() {
            let c = cfg_mut(s, st);
            let Some(parent) = c.ifaces.get(&n).map(|i| i.parent.clone()) else { continue };
            let r: Option<String> = if parent.is_none() {
                Some(marker.clone())
            } else if a.neg {
                let i = c.ifaces.get_mut(&n).unwrap();
                i.dot1q = None;
                i.ip = None;
                None
            } else {
                let other = c.ifaces.iter().find(|(o, x)| o.as_str() != n && x.parent == parent && x.dot1q == Some(vid)).map(|(o, _)| o.clone());
                match other {
                    Some(o) => Some(format!(
                        "%Configuration of multiple subinterfaces of the same main\ninterface with the same VID ({}) is not permitted.\nThis VID is already configured on {}.",
                        vid, o
                    )),
                    None => {
                        c.ifaces.get_mut(&n).unwrap().dot1q = Some(vid);
                        None
                    }
                }
            };
            if let Some(r) = r {
                if !outs.contains(&r) {
                    outs.push(r);
                }
            }
        }
        result(outs.join("\n"))
    });
    reg.d("if", "ip nat inside", |s, st, a| {
        let neg = a.neg;
        result(not_l2(s, st, &a.line, "nat", |i, _| {
            i.nat_dir = if neg { if i.nat_dir == Some(NatDir::Inside) { None } else { i.nat_dir } } else { Some(NatDir::Inside) };
            None
        }))
    });
    reg.d("if", "ip nat outside", |s, st, a| {
        let neg = a.neg;
        result(not_l2(s, st, &a.line, "nat", |i, _| {
            i.nat_dir = if neg { if i.nat_dir == Some(NatDir::Outside) { None } else { i.nat_dir } } else { Some(NatDir::Outside) };
            None
        }))
    });
    reg.d("if", "ip access-group <word> in", |s, st, a| {
        let (neg, w) = (a.neg, a.s(0).to_string());
        result(not_l2(s, st, &a.line, "access-group", |i, _| {
            i.acl_in = if neg { None } else { Some(w.clone()) };
            None
        }))
    });
    reg.d("if", "ip access-group <word> out", |s, st, a| {
        let (neg, w) = (a.neg, a.s(0).to_string());
        result(not_l2(s, st, &a.line, "access-group", |i, _| {
            i.acl_out = if neg { None } else { Some(w.clone()) };
            None
        }))
    });
    reg.def(
        "if",
        "ip access-group in",
        |s, st, _| {
            result(each_if(s, st, |i, _| {
                i.acl_in = None;
                None
            }))
        },
        NO_ONLY,
    );
    reg.def(
        "if",
        "ip access-group out",
        |s, st, _| {
            result(each_if(s, st, |i, _| {
                i.acl_out = None;
                None
            }))
        },
        NO_ONLY,
    );
    reg.d("if", "ip helper-address <ip>", |_, _, _| result(""));
    reg.d("if", "cdp enable", |_, _, _| result(""));
    reg.d("if", "negotiation auto", |_, _, _| result(""));
}

// ------------------------------------------------------------------ Fler kommandon

fn reg_more(reg: &mut Reg) {
    reg.show("vlan id <n:1-4094>", sw_only(|s, st, a| result(SH::vlan_id(st, &s.dev, a.n(0) as u16))), true);
    reg.show("processes cpu", |s, st, _| result(SH::processes_cpu(st, &s.dev)), true);
    reg.show("processes cpu sorted", |s, st, _| result(SH::processes_cpu(st, &s.dev)), true);
    reg.show("cdp neighbors detail", |s, st, _| result(SH::cdp_detail(st, &s.dev)), true);
    reg.show("spanning-tree summary", sw_only(|s, st, _| result(SH::stp_summary(st, &s.dev))), true);
    reg.show("spanning-tree blockedports", sw_only(|s, st, _| result(SH::stp_blocked(st, &s.dev))), true);
    reg.show("inventory", |s, st, _| result(SH::inventory(st, &s.dev)), true);
    reg.show("interfaces counters errors", sw_only(|s, st, _| result(SH::counters_errors(st, &s.dev))), true);
    reg.def(
        "config",
        "default interface <if>",
        |s, st, a| {
            let n = a.s(0).to_string();
            let old = cfg(s, st).ifaces.get(&n).cloned();
            let Some(old) = old.filter(|i| i.parent.is_none() && i.svi.is_none()) else {
                return result(s.invalid_marker(st, &a.line, &n));
            };
            // Ett loopback förblir loopback med standardvärden (inte en vanlig routerport)
            let fresh = if old.is_loop {
                Iface::loopback()
            } else if s.is_switch(st) {
                let mut f = Iface::switch_port(0);
                f.sfp = old.sfp;
                f
            } else {
                Iface::router_port()
            };
            cfg_mut(s, st).ifaces.insert(n.clone(), fresh);
            st.dev_mut(&s.dev).rt.errdisabled.shift_remove(&n);
            result(format!("Interface {} set to default configuration", n))
        },
        NO_NEG,
    );
    reg.d("config", "errdisable recovery cause psecure-violation", |s, st, a| {
        cfg_mut(s, st).err_recovery = !a.neg;
        result("")
    });
    reg.d("user exec", "terminal history size <n:0-256>", |_, _, _| result(""));
}

// ------------------------------------------------------------------ IPsec och MSS

fn crypto_mut<'a>(s: &IosSession, st: &'a mut State) -> &'a mut Crypto {
    cfg_mut(s, st).crypto.get_or_insert_with(Crypto::default)
}

/// Kommando som bara finns på routrar och som räknar om simuleringen efteråt
fn cr_only<F>(f: F) -> impl Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static
where
    F: Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static,
{
    move |s, st, a| {
        if !s.is_router(st) {
            return result(s.invalid_marker(st, &a.line, "crypto"));
        }
        let r = f(s, st, a);
        S::touch(st);
        r
    }
}

fn pol_mut<'a>(s: &IosSession, st: &'a mut State) -> Option<&'a mut IsakmpPolicy> {
    let p = s.ctx.policy?;
    crypto_mut(s, st).policies.get_mut(&p)
}

fn ent_mut<'a>(s: &IosSession, st: &'a mut State) -> Option<&'a mut CmapEntry> {
    let m = s.ctx.map.clone()?;
    let q = s.ctx.seq?;
    crypto_mut(s, st).maps.get_mut(&m)?.get_mut(&q)
}

fn cmap_enter(s: &mut IosSession, st: &mut State, a: &Args) -> CmdResult {
    let (name, seq) = (a.s(0).to_string(), a.n(1));
    let cr = crypto_mut(s, st);
    let m = cr.maps.entry(name.clone()).or_default();
    if a.neg {
        m.remove(&seq);
        if m.is_empty() {
            cr.maps.shift_remove(&name);
        }
        return result("");
    }
    let fresh = !m.contains_key(&seq);
    if fresh {
        m.insert(seq, CmapEntry::default());
    }
    s.mode = "cmap".into();
    s.ctx = Ctx { map: Some(name), seq: Some(seq), ..Default::default() };
    result(if fresh { "% NOTE: This new crypto map will remain disabled until a peer\n        and a valid access list have been configured." } else { "" })
}

fn reg_ipsec(reg: &mut Reg) {
    reg.d(
        "config",
        "crypto isakmp policy <n:1-10000>",
        cr_only(|s, st, a| {
            let n = a.n(0);
            let cr = crypto_mut(s, st);
            if a.neg {
                cr.policies.remove(&n);
                return result("");
            }
            cr.policies.entry(n).or_insert_with(|| IsakmpPolicy { enc: "des".into(), hash: "sha".into(), auth: "rsa-sig".into(), group: 1, lifetime: 86400 });
            s.mode = "isakmp".into();
            s.ctx = Ctx { policy: Some(n), ..Default::default() };
            result("")
        }),
    );
    reg.d("isakmp", "encryption <text>", |s, st, a| {
        let t = a.s(0).to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
        let ok = match t.as_str() {
            "des" => "des",
            "3des" => "3des",
            "aes" | "aes 128" => "aes",
            "aes 192" => "aes 192",
            "aes 256" => "aes 256",
            _ => return result(s.invalid_marker(st, &a.line, a.s(0))),
        };
        let v = if a.neg { "des" } else { ok };
        if let Some(p) = pol_mut(s, st) {
            p.enc = v.into();
        }
        S::touch(st);
        result("")
    });
    reg.d("isakmp", "hash <word>", |s, st, a| {
        let h = a.s(0).to_lowercase();
        if !["sha", "sha256", "sha384", "sha512", "md5"].contains(&h.as_str()) {
            return result(s.invalid_marker(st, &a.line, a.s(0)));
        }
        let neg = a.neg;
        if let Some(p) = pol_mut(s, st) {
            p.hash = if neg { "sha".into() } else { h };
        }
        S::touch(st);
        result("")
    });
    reg.d("isakmp", "authentication pre-share", |s, st, a| {
        let neg = a.neg;
        if let Some(p) = pol_mut(s, st) {
            p.auth = if neg { "rsa-sig" } else { "pre-share" }.into();
        }
        S::touch(st);
        result("")
    });
    reg.d("isakmp", "authentication rsa-sig", |s, st, _| {
        if let Some(p) = pol_mut(s, st) {
            p.auth = "rsa-sig".into();
        }
        S::touch(st);
        result("")
    });
    reg.d("isakmp", "group <n:1-24>", |s, st, a| {
        let g = a.n(0);
        if ![1, 2, 5, 14, 15, 16, 19, 20, 24].contains(&g) {
            return result(s.invalid_marker(st, &a.line, &g.to_string()));
        }
        let neg = a.neg;
        if let Some(p) = pol_mut(s, st) {
            p.group = if neg { 1 } else { g };
        }
        S::touch(st);
        result("")
    });
    reg.d("isakmp", "lifetime <n:60-86400>", |s, st, a| {
        let (neg, v) = (a.neg, a.n(0));
        if let Some(p) = pol_mut(s, st) {
            p.lifetime = if neg { 86400 } else { v };
        }
        result("")
    });
    // crypto isakmp key [0|6] <nyckel> address <ip>
    reg.d(
        "config",
        "crypto isakmp key <text>",
        cr_only(|s, st, a| {
            let mut t: Vec<String> = a.s(0).split_whitespace().map(|x| x.to_string()).collect();
            if t.len() == 4 && (t[0] == "0" || t[0] == "6") {
                t.remove(0);
            }
            if t.len() != 3 || !t[1].eq_ignore_ascii_case("address") || !U::is_ip(&t[2]) {
                let w = t.get(1).or(t.first()).cloned().unwrap_or_default();
                return result(s.invalid_marker(st, &a.line, &w));
            }
            let cr = crypto_mut(s, st);
            if a.neg {
                cr.keys.shift_remove(&t[2]);
                return result("");
            }
            cr.keys.insert(t[2].clone(), t[0].clone());
            result("")
        }),
    );
    reg.d(
        "config",
        "crypto ipsec transform-set <word> <text>",
        cr_only(|s, st, a| {
            let name = a.s(0).to_string();
            if a.neg {
                crypto_mut(s, st).transform_sets.shift_remove(&name);
                return result("");
            }
            let t: Vec<String> = a.s(1).to_lowercase().split_whitespace().map(|x| x.to_string()).collect();
            let ok_t = ["esp-aes", "128", "192", "256", "esp-3des", "esp-des", "esp-sha-hmac", "esp-sha256-hmac", "esp-sha384-hmac", "esp-md5-hmac", "ah-sha-hmac", "esp-gcm"];
            for w in &t {
                if !ok_t.contains(&w.as_str()) {
                    return result(s.invalid_marker(st, &a.line, w));
                }
            }
            crypto_mut(s, st).transform_sets.insert(name.clone(), TransformSet { transforms: t, mode: "tunnel".into() });
            s.mode = "tset".into();
            s.ctx = Ctx { ts: Some(name), ..Default::default() };
            result("")
        }),
    );
    reg.def(
        "config",
        "crypto ipsec transform-set <word>",
        cr_only(|s, st, a| {
            crypto_mut(s, st).transform_sets.shift_remove(a.s(0));
            result("")
        }),
        NO_ONLY,
    );
    reg.d("tset", "mode tunnel", |s, st, _| {
        if let Some(n) = s.ctx.ts.clone() {
            if let Some(t) = crypto_mut(s, st).transform_sets.get_mut(&n) {
                t.mode = "tunnel".into();
            }
        }
        result("")
    });
    reg.d("tset", "mode transport", |s, st, a| {
        let neg = a.neg;
        if let Some(n) = s.ctx.ts.clone() {
            if let Some(t) = crypto_mut(s, st).transform_sets.get_mut(&n) {
                t.mode = if neg { "tunnel" } else { "transport" }.into();
            }
        }
        result("")
    });
    reg.d("config", "crypto map <word> <n:1-65535> ipsec-isakmp", cr_only(cmap_enter));
    reg.d("config", "crypto map <word> <n:1-65535>", cr_only(cmap_enter));
    reg.d("cmap", "set peer <ip>", |s, st, a| {
        let (neg, ip) = (a.neg, a.s(0).to_string());
        if let Some(e) = ent_mut(s, st) {
            e.peer = if neg { None } else { Some(ip) };
        }
        S::touch(st);
        result("")
    });
    reg.d("cmap", "set transform-set <word>", |s, st, a| {
        let w = a.s(0).to_string();
        if !a.neg && !crypto_mut(s, st).transform_sets.contains_key(&w) {
            return result(format!("ERROR: transform set with tag \"{}\" does not exist.", w));
        }
        let neg = a.neg;
        if let Some(e) = ent_mut(s, st) {
            e.ts = if neg { None } else { Some(w) };
        }
        S::touch(st);
        result("")
    });
    reg.d("cmap", "match address <word>", |s, st, a| {
        let (neg, w) = (a.neg, a.s(0).to_string());
        if let Some(e) = ent_mut(s, st) {
            e.acl = if neg { None } else { Some(w) };
        }
        S::touch(st);
        result("")
    });
    reg.d("cmap", "set pfs <word>", |_, _, _| result(""));
    reg.d("cmap", "set security-association lifetime seconds <n:120-86400>", |_, _, _| result(""));
    reg.d("cmap", "description <text>", |_, _, _| result(""));
    reg.d("if", "crypto map <word>", |s, st, a| {
        if !s.is_router(st) {
            return result(s.invalid_marker(st, &a.line, "crypto"));
        }
        let (neg, w) = (a.neg, a.s(0).to_string());
        let out = not_l2(s, st, &a.line, "crypto", |i, _| {
            i.crypto_map = if neg { None } else { Some(w.clone()) };
            None
        });
        S::touch(st);
        if !a.neg && out.is_empty() {
            S::push_log(st, &s.dev, "%CRYPTO-6-ISAKMP_ON_OFF: ISAKMP is ON");
        }
        result(out)
    });
    reg.def(
        "if",
        "crypto map",
        |s, st, _| {
            let r = each_if(s, st, |i, _| {
                i.crypto_map = None;
                None
            });
            S::touch(st);
            result(r)
        },
        NO_ONLY,
    );
    reg.d("if", "ip tcp adjust-mss <n:500-1460>", |s, st, a| {
        let (neg, v) = (a.neg, a.n(0));
        let out = not_l2(s, st, &a.line, "tcp", |i, _| {
            i.adjust_mss = if neg { None } else { Some(v) };
            None
        });
        S::touch(st);
        result(out)
    });
    reg.def(
        "if",
        "ip tcp adjust-mss",
        |s, st, _| {
            let r = each_if(s, st, |i, _| {
                i.adjust_mss = None;
                None
            });
            S::touch(st);
            result(r)
        },
        NO_ONLY,
    );
    reg.d("if", "ip mtu <n:68-1500>", |s, st, a| {
        let (neg, v) = (a.neg, a.n(0));
        result(not_l2(s, st, &a.line, "mtu", |i, _| {
            i.mtu = if neg { None } else { Some(v) };
            None
        }))
    });
    reg.def(
        "if",
        "ip mtu",
        |s, st, _| {
            result(each_if(s, st, |i, _| {
                i.mtu = None;
                None
            }))
        },
        NO_ONLY,
    );
    reg.show("crypto isakmp sa", rt_only(|s, st, _| result(SH::crypto_isakmp_sa(st, &s.dev))), false);
    reg.show("crypto isakmp policy", rt_only(|s, st, _| result(SH::crypto_isakmp_policy(st, &s.dev))), false);
    reg.show("crypto ipsec sa", rt_only(|s, st, _| result(SH::crypto_ipsec_sa(st, &s.dev))), false);
    reg.show("crypto map", rt_only(|s, st, _| result(SH::crypto_map_show(st, &s.dev))), false);
    reg.show("crypto session", rt_only(|s, st, _| result(SH::crypto_session(st, &s.dev))), false);
    reg.show(
        "crypto ipsec transform-set",
        rt_only(|s, st, _| {
            let Some(cr) = &cfg(s, st).crypto else { return result("") };
            result(
                cr.transform_sets
                    .iter()
                    .map(|(n, t)| {
                        format!(
                            "Transform set default/{}: {{ {} }}\n   will negotiate = {{ {},  }},\n",
                            n,
                            t.transforms.join(" "),
                            if t.mode == "transport" { "Transport" } else { "Tunnel" }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        }),
        false,
    );
    reg.d("exec", "clear crypto sa", |s, st, _| {
        S::clear_crypto(st, &s.dev);
        result("")
    });
    reg.d("exec", "clear crypto isakmp", |s, st, _| {
        S::clear_crypto(st, &s.dev);
        result("")
    });
    reg.d("exec", "clear crypto session", |s, st, _| {
        S::clear_crypto(st, &s.dev);
        result("")
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::ios::IosSession;

    fn world() -> State {
        State::new(build_golden(0))
    }

    /// Kör rader direkt mot exec (utan S::refresh, så testet går även med en ofärdig simulator)
    fn run(s: &mut IosSession, st: &mut State, lines: &[&str]) -> Vec<String> {
        lines.iter().map(|l| s.exec(st, l).out).collect()
    }

    fn sess(st: &State, dev: &str) -> IosSession {
        let mut s = IosSession::new(st, dev, "console", true, None);
        s.mode = "config".into();
        s
    }

    #[test]
    fn interface_shutdown_och_noshut() {
        let mut st = world();
        let mut s = sess(&st, "SW1");
        run(&mut s, &mut st, &["interface gi0/5", "shutdown"]);
        assert_eq!(s.mode, "if");
        assert!(st.dev("SW1").cfg().ifaces["GigabitEthernet0/5"].shutdown);
        run(&mut s, &mut st, &["no shutdown"]);
        assert!(!st.dev("SW1").cfg().ifaces["GigabitEthernet0/5"].shutdown);
    }

    #[test]
    fn switchport_och_vlan() {
        let mut st = world();
        let mut s = sess(&st, "SW1");
        let o = run(&mut s, &mut st, &["vlan 50", "name Gaster", "exit", "interface gi0/6", "switchport mode access", "switchport access vlan 50"]);
        assert_eq!(st.dev("SW1").cfg().vlans[&50], "Gaster");
        let i = &st.dev("SW1").cfg().ifaces["GigabitEthernet0/6"];
        assert_eq!(i.mode, "access");
        assert_eq!(i.access_vlan, 50);
        assert_eq!(o[5], "");
        // trunk kräver dot1q-inkapsling
        let o = run(&mut s, &mut st, &["switchport mode trunk"]);
        assert!(o[0].contains("Command rejected"));
        run(&mut s, &mut st, &["switchport trunk encapsulation dot1q", "switchport mode trunk", "switchport trunk allowed vlan 10,20,50"]);
        let i = &st.dev("SW1").cfg().ifaces["GigabitEthernet0/6"];
        assert_eq!(i.mode, "trunk");
        assert_eq!(i.allowed, Some(VlanList::List(vec![10, 20, 50])));
    }

    #[test]
    fn vlan_skapas_vid_access() {
        let mut st = world();
        let mut s = sess(&st, "SW1");
        let o = run(&mut s, &mut st, &["interface gi0/7", "switchport access vlan 77"]);
        assert!(o[1].contains("Creating vlan 77"));
        assert_eq!(st.dev("SW1").cfg().vlans[&77], "VLAN0077");
    }

    #[test]
    fn ip_route_och_ta_bort() {
        let mut st = world();
        let mut s = sess(&st, "R1");
        let o = run(&mut s, &mut st, &["ip route 10.99.0.0 255.255.0.0 192.168.1.1"]);
        assert_eq!(o[0], "");
        let n = st.dev("R1").cfg().routes.len();
        assert!(st.dev("R1").cfg().routes.iter().any(|r| r.net == "10.99.0.0" && r.nh.as_deref() == Some("192.168.1.1")));
        let o = run(&mut s, &mut st, &["ip route 10.99.1.0 255.255.0.0 192.168.1.1"]);
        assert_eq!(o[0], "%Inconsistent address and mask");
        run(&mut s, &mut st, &["no ip route 10.99.0.0 255.255.0.0 192.168.1.1"]);
        assert_eq!(st.dev("R1").cfg().routes.len(), n - 1);
        let o = run(&mut s, &mut st, &["no ip route 10.99.0.0 255.255.0.0 192.168.1.1"]);
        assert_eq!(o[0], "%No matching route to delete");
    }

    #[test]
    fn access_list_namngiven_och_numrerad() {
        let mut st = world();
        let mut s = sess(&st, "R1");
        run(&mut s, &mut st, &["ip access-list extended TEST", "permit tcp 10.0.0.0 0.0.0.255 any eq 80", "deny ip any any log"]);
        assert_eq!(s.mode, "acl");
        let acl = &st.dev("R1").cfg().acls["TEST"];
        assert!(acl.extended);
        assert_eq!(acl.rules.len(), 2);
        assert_eq!(acl.rules[0].seq, 10);
        assert_eq!(acl.rules[0].dport, Some(80));
        assert_eq!(acl.rules[1].seq, 20);
        assert!(acl.rules[1].log);
        let o = run(&mut s, &mut st, &["permit tcp any"]);
        assert!(o[0].contains("Invalid input"));
        run(&mut s, &mut st, &["exit", "access-list 5 permit 10.1.1.0 0.0.0.255"]);
        let a5 = &st.dev("R1").cfg().acls["5"];
        assert!(!a5.extended);
        assert_eq!(a5.rules.len(), 1);
    }

    #[test]
    fn parse_ace_fel_och_remark() {
        assert!(parse_ace(true, "permit foo any any").is_err());
        assert_eq!(parse_ace(true, "permit tcp any any eq 80 extra"), Err(6));
        let r = parse_ace(false, "20 deny host 1.2.3.4").unwrap();
        assert_eq!(r.seq, 20);
        assert_eq!(r.src, Some(AddrSpec::host("1.2.3.4")));
        let r = parse_ace(true, "remark hej alla").unwrap();
        assert_eq!(r.remark.as_deref(), Some("hej alla"));
    }

    #[test]
    fn dhcp_pool_och_nat() {
        let mut st = world();
        let mut s = sess(&st, "R1");
        run(&mut s, &mut st, &["ip dhcp pool TESTPOOL", "network 10.55.0.0 255.255.255.0", "default-router 10.55.0.1", "dns-server 9.9.9.9", "lease 2"]);
        let p = &st.dev("R1").cfg().dhcp_pools["TESTPOOL"];
        assert_eq!(p.network.as_deref(), Some("10.55.0.0"));
        assert_eq!(p.default_router, vec!["10.55.0.1".to_string()]);
        assert_eq!(p.lease, Some(2.0));
        let o = run(&mut s, &mut st, &["network 10.55.0.5 255.255.255.0"]);
        assert!(o[0].contains("is an invalid network"));
        run(&mut s, &mut st, &["exit", "ip nat inside source static 10.55.0.9 203.0.113.9"]);
        assert!(st.dev("R1").cfg().nat.statics.iter().any(|x| x.local == "10.55.0.9"));
    }

    #[test]
    fn hostname_och_user() {
        let mut st = world();
        let mut s = sess(&st, "SW2");
        let o = run(&mut s, &mut st, &["hostname 1bad"]);
        assert!(o[0].contains("illegal characters"));
        run(&mut s, &mut st, &["hostname KALLE", "ip domain-name nordvik.example"]);
        assert_eq!(st.dev("SW2").cfg().hostname, "KALLE");
        assert_eq!(st.dev("SW2").cfg().domain_name.as_deref(), Some("nordvik.example"));
    }

    #[test]
    fn crypto_isakmp_och_map() {
        let mut st = world();
        let mut s = sess(&st, "R1");
        run(&mut s, &mut st, &[
            "crypto isakmp policy 10", "encryption aes 256", "hash sha256", "authentication pre-share", "group 14", "exit",
            "crypto isakmp key hemlig address 1.2.3.4", "crypto ipsec transform-set TS esp-aes 256 esp-sha-hmac", "exit",
            "crypto map CM 10 ipsec-isakmp", "set peer 1.2.3.4", "set transform-set TS", "match address VPN",
        ]);
        let cr = st.dev("R1").cfg().crypto.as_ref().unwrap();
        assert_eq!(cr.policies[&10].enc, "aes 256");
        assert_eq!(cr.policies[&10].group, 14);
        assert_eq!(cr.keys["1.2.3.4"], "hemlig");
        assert_eq!(cr.transform_sets["TS"].transforms, vec!["esp-aes", "256", "esp-sha-hmac"]);
        let e = &cr.maps["CM"][&10];
        assert_eq!(e.peer.as_deref(), Some("1.2.3.4"));
        assert_eq!(e.ts.as_deref(), Some("TS"));
        assert_eq!(e.acl.as_deref(), Some("VPN"));
    }

    #[test]
    fn hjalptexter() {
        assert_eq!(super::super::ios_help::help_text("shutdown", false), "Shutdown the selected interface");
        assert_eq!(super::super::ios_help::help_text("interface", true), "IP interface status and configuration");
        assert_eq!(super::super::ios_help::help_text("interface", false), "Select an interface to configure");
        assert_eq!(super::super::ios_help::help_text("nonsens", true), "");
    }

    /// Hela vägen via handle_line (kräver färdig simulator)
    #[test]
    
    fn handle_line_hela_vagen() {
        let mut st = world();
        let mut s = IosSession::new(&st, "SW1", "console", true, None);
        s.handle_line(&mut st, "configure terminal");
        s.handle_line(&mut st, "interface gi0/5");
        s.handle_line(&mut st, "shutdown");
        assert!(st.dev("SW1").cfg().ifaces["GigabitEthernet0/5"].shutdown);
        s.handle_line(&mut st, "end");
        assert_eq!(s.mode, "exec");
    }

    #[test]
    fn enable_secret_med_typnummer() {
        let mut st = world();
        let mut s = IosSession::new(&st, "R1", "console", true, None);
        s.handle_line(&mut st, "configure terminal");
        assert_eq!(s.handle_line(&mut st, "enable secret 5 hemligt").out, "");
        assert_eq!(st.dev("R1").cfg().enable_plain.as_deref(), Some("hemligt"));
        s.handle_line(&mut st, "enable secret 0 annat");
        assert_eq!(st.dev("R1").cfg().enable_plain.as_deref(), Some("annat"));
        s.handle_line(&mut st, "enable secret 9 tredje");
        assert_eq!(st.dev("R1").cfg().enable_plain.as_deref(), Some("tredje"));
        // ett ensamt ord är lösenordet
        s.handle_line(&mut st, "enable secret ensamt");
        assert_eq!(st.dev("R1").cfg().enable_plain.as_deref(), Some("ensamt"));
        // ett ensiffrigt tal utan typ är lösenordet
        assert_eq!(s.handle_line(&mut st, "enable secret 5").out, "");
        assert_eq!(st.dev("R1").cfg().enable_plain.as_deref(), Some("5"));
        // tal som inte är 0-9 är ett vanligt lösenord
        s.handle_line(&mut st, "enable secret 42");
        assert_eq!(st.dev("R1").cfg().enable_plain.as_deref(), Some("42"));
    }

    #[test]
    fn crypto_modulus_behaller_fordrojning() {
        let mut st = world();
        st.dev_mut("R1").cfg_mut().crypto_key = None;
        let mut s = IosSession::new(&st, "R1", "console", true, None);
        s.handle_line(&mut st, "configure terminal");
        s.handle_line(&mut st, "hostname R-Test");
        s.handle_line(&mut st, "ip domain-name test.local");
        let r = s.handle_line(&mut st, "crypto key generate rsa modulus 1024");
        assert_eq!(r.delay, 800);
        assert!(r.out.contains("The name for the keys will be: R-Test.test.local"), "{}", r.out);
        assert!(r.out.contains("Generating 1024 bit RSA keys"), "{}", r.out);
        assert_eq!(st.dev("R1").cfg().crypto_key, Some(1024));
    }

    #[test]
    fn default_interface_loopback_forblir_loopback() {
        let mut st = world();
        let mut s = IosSession::new(&st, "R1", "console", true, None);
        s.handle_line(&mut st, "configure terminal");
        s.handle_line(&mut st, "interface loopback 9");
        s.handle_line(&mut st, "ip address 10.9.9.9 255.255.255.255");
        s.handle_line(&mut st, "description test");
        s.handle_line(&mut st, "exit");
        let o = s.handle_line(&mut st, "default interface Loopback9").out;
        assert!(o.contains("set to default configuration"), "{}", o);
        let k = st.dev("R1").cfg().ifaces.keys().find(|k| k.ends_with('9') && k.starts_with("Loop")).cloned().unwrap();
        let i = &st.dev("R1").cfg().ifaces[&k];
        assert!(i.is_loop);
        assert!(i.ip.is_none());
        assert!(!i.shutdown);
        assert!(i.description.is_empty());
        assert!(i.l2 == false && i.mode.is_empty());
    }

    #[test]
    fn spanning_tree_vlan_all_priority() {
        let mut st = world();
        let mut s = IosSession::new(&st, "SW1", "console", true, None);
        s.handle_line(&mut st, "configure terminal");
        assert_eq!(s.handle_line(&mut st, "spanning-tree vlan all priority 8192").out, "");
        let c = st.dev("SW1").cfg();
        assert!(!c.vlans.is_empty());
        for v in c.vlans.keys() {
            assert_eq!(c.stp_priority.get(v), Some(&8192), "VLAN {}", v);
        }
    }

    #[test]
    fn dhcp_lease_infinite_i_running_config() {
        let mut st = world();
        let mut s = IosSession::new(&st, "R1", "console", true, None);
        s.handle_line(&mut st, "configure terminal");
        s.handle_line(&mut st, "ip dhcp pool TESTLEASE");
        s.handle_line(&mut st, "lease infinite");
        s.handle_line(&mut st, "end");
        let rc = s.handle_line(&mut st, "show running-config").out;
        let rad = rc.lines().skip_while(|l| *l != "ip dhcp pool TESTLEASE").nth(1).unwrap_or("");
        assert_eq!(rad, " lease infinite", "{}", rc);
    }
}
