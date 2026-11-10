//! OSPF- och BGP-kommandon från js/cli/ios_dyn.js.
//! Konfigurationen sparas i config.ospf och config.bgp; simulatorn räknar grannskap och rutter (net::dynroute).
use super::ios::{Args, Ctx, IosSession, Reg, Val, NO_NEG, NO_ONLY};
use super::ios_show as SH;
use super::CmdResult;
use crate::net::dynroute::{self as DR, BgpInfo, OspfRouter};
use crate::net::model::*;
use crate::net::sim::{self as S, State};
use crate::net::util as U;
use std::collections::HashSet;

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
fn invalid_plain() -> String {
    "% Invalid input detected at '^' marker.".to_string()
}
fn hms(sec: f64) -> String {
    let sec = sec.max(0.0).floor() as i64;
    format!("{:02}:{:02}:{:02}", (sec / 3600) % 24, (sec / 60) % 60, sec % 60)
}
fn prefix(mask: &str) -> u32 {
    U::mask_to_prefix(mask).unwrap_or(0)
}

fn router_only<F>(f: F) -> impl Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static
where
    F: Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static,
{
    move |s, st, a| {
        if !s.is_router(st) {
            let w = a.kws.first().cloned().unwrap_or_else(|| "router".into());
            return out(s.invalid_marker(st, &a.line, &w));
        }
        f(s, st, a)
    }
}

/// OSPF-routerns härledda data för enheten
fn ospf_r(s: &IosSession, st: &mut State) -> Option<OspfRouter> {
    let d = S::get(st);
    d.dynr.ospf.get(&s.dev).cloned()
}
fn bgp_r(s: &IosSession, st: &mut State) -> Option<BgpInfo> {
    let d = S::get(st);
    d.dynr.bgp.get(&s.dev).cloned()
}

fn area_num(v: &Val) -> i64 {
    match v {
        Val::Num(n) => *n,
        Val::Str(s) | Val::Text(s) => {
            if U::is_ip(s) {
                U::ip_to_int(s).map(|x| x as i64).unwrap_or(0)
            } else {
                s.parse().unwrap_or(0)
            }
        }
        _ => 0,
    }
}

fn class_mask(ip: &str) -> &'static str {
    let f: u32 = ip.split('.').next().and_then(|x| x.parse().ok()).unwrap_or(0);
    if f < 128 {
        "255.0.0.0"
    } else if f < 192 {
        "255.255.0.0"
    } else {
        "255.255.255.0"
    }
}

fn end_fn(s: &mut IosSession, st: &mut State, _a: &Args) -> CmdResult {
    s.mode = "exec".into();
    s.ctx = Ctx::default();
    let via = if s.via == "console" { "console by console" } else { "vty0 (192.168.1.200)" };
    S::push_log(st, &s.dev.clone(), &format!("%SYS-5-CONFIG_I: Configured from {}", via));
    ok()
}

// Interfacekommandon: sätt/ta bort ett fält på alla valda interface
fn set_if<G>(set: G) -> impl Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static
where
    G: Fn(&mut Iface, Option<i64>) + 'static,
{
    move |s, st, a| {
        let names = s.ctx.ifs.clone();
        let v = if a.neg { None } else { Some(a.n(0)) };
        let c = cfg(s, st);
        for n in &names {
            if let Some(i) = c.ifaces.get_mut(n) {
                set(i, v);
            }
        }
        S::touch(st);
        ok()
    }
}
fn clear_if<G>(clr: G) -> impl Fn(&mut IosSession, &mut State, &Args) -> CmdResult + 'static
where
    G: Fn(&mut Iface) + 'static,
{
    move |s, st, _| {
        let names = s.ctx.ifs.clone();
        let c = cfg(s, st);
        for n in &names {
            if let Some(i) = c.ifaces.get_mut(n) {
                clr(i);
            }
        }
        S::touch(st);
        ok()
    }
}

fn network_cmd(s: &mut IosSession, st: &mut State, a: &Args) -> CmdResult {
    let net = a.s(0).to_string();
    let wild = a.s(1).to_string();
    let area = a.v.get(2).map(area_num).unwrap_or(0);
    let neg = a.neg;
    if let Some(o) = cfg(s, st).ospf.as_mut() {
        let same = |x: &OspfNet| x.net == net && x.wild == wild;
        if neg {
            o.networks.retain(|x| !(same(x) && x.area == area));
        } else {
            if let Some(ex) = o.networks.iter().find(|x| same(x)) {
                if ex.area != area {
                    return out(format!("% OSPF: \"network {} {} area {}\" is already configured", net, wild, ex.area));
                }
            } else {
                o.networks.push(OspfNet { net, wild, area });
            }
        }
    }
    S::touch(st);
    ok()
}

fn bgp_network(s: &mut IosSession, st: &mut State, net: &str, mask: &str, neg: bool) -> CmdResult {
    if let Some(b) = cfg(s, st).bgp.as_mut() {
        b.networks.retain(|n| !(n.net == net && n.mask == mask));
        if !neg {
            b.networks.push(BgpNet { net: net.into(), mask: mask.into() });
        }
    }
    S::touch(st);
    ok()
}

pub fn register(reg: &mut Reg) {
    // ------------------------------------------------------------ Lägen
    for m in ["ospf", "bgp"] {
        reg.def(m, "end", end_fn, NO_NEG);
        reg.def(
            m,
            "exit",
            |s, _st, _| {
                s.leave_sub();
                s.mode = "config".into();
                ok()
            },
            NO_NEG,
        );
    }

    // ------------------------------------------------------------ OSPF: konfiguration
    reg.d(
        "config",
        "router ospf <n:1-65535>",
        router_only(|s, st, a| {
            let pid = a.n(0);
            let neg = a.neg;
            let c = cfg(s, st);
            if neg {
                if c.ospf.as_ref().map(|o| o.pid == pid).unwrap_or(false) {
                    c.ospf = None;
                }
                S::touch(st);
                return ok();
            }
            match &c.ospf {
                None => {
                    c.ospf = Some(Ospf {
                        pid,
                        router_id: None,
                        networks: vec![],
                        passive_default: false,
                        passive: vec![],
                        no_passive: vec![],
                        ref_bw: 100,
                        default_originate: false,
                        default_always: false,
                        log_adj: true,
                    })
                }
                Some(o) if o.pid != pid => {
                    return out(format!("% OSPF process {} is already running. Only one process is supported here.", o.pid));
                }
                _ => {}
            }
            s.mode = "ospf".into();
            s.ctx = Ctx::default();
            S::touch(st);
            ok()
        }),
    );
    reg.d("ospf", "router-id <ip>", |s, st, a| {
        let ip = a.s(0).to_string();
        if let Some(o) = cfg(s, st).ospf.as_mut() {
            o.router_id = if a.neg { None } else { Some(ip) };
        }
        S::touch(st);
        ok()
    });
    reg.d("ospf", "network <ip> <wild> area <n:0-4294967295>", network_cmd);
    reg.d("ospf", "network <ip> <wild> area <ip>", network_cmd);
    reg.d("ospf", "passive-interface <if>", |s, st, a| {
        let n = a.s(0).to_string();
        let neg = a.neg;
        let c = cfg(s, st);
        if !c.ifaces.contains_key(&n) {
            return out(invalid_plain());
        }
        if let Some(o) = c.ospf.as_mut() {
            if o.passive_default {
                if neg {
                    if !o.no_passive.contains(&n) {
                        o.no_passive.push(n);
                    }
                } else {
                    o.no_passive.retain(|x| *x != n);
                }
            } else if neg {
                o.passive.retain(|x| *x != n);
            } else if !o.passive.contains(&n) {
                o.passive.push(n);
            }
        }
        S::touch(st);
        ok()
    });
    reg.d("ospf", "passive-interface default", |s, st, a| {
        let neg = a.neg;
        if let Some(o) = cfg(s, st).ospf.as_mut() {
            o.passive_default = !neg;
            o.passive.clear();
            o.no_passive.clear();
        }
        S::touch(st);
        ok()
    });
    reg.d("ospf", "auto-cost reference-bandwidth <n:1-4294967>", |s, st, a| {
        let (neg, v) = (a.neg, a.n(0));
        if let Some(o) = cfg(s, st).ospf.as_mut() {
            o.ref_bw = if neg { 100 } else { v };
        }
        S::touch(st);
        if neg {
            ok()
        } else {
            out("% OSPF: Reference bandwidth is changed.\n        Please ensure reference bandwidth is consistent across all routers.")
        }
    });
    reg.d("ospf", "default-information originate", |s, st, a| {
        let neg = a.neg;
        if let Some(o) = cfg(s, st).ospf.as_mut() {
            o.default_originate = !neg;
            if neg {
                o.default_always = false;
            }
        }
        S::touch(st);
        ok()
    });
    reg.d("ospf", "default-information originate always", |s, st, a| {
        let neg = a.neg;
        if let Some(o) = cfg(s, st).ospf.as_mut() {
            o.default_originate = !neg;
            o.default_always = !neg;
        }
        S::touch(st);
        ok()
    });
    for p in ["log-adjacency-changes", "log-adjacency-changes detail"] {
        reg.d("ospf", p, |s, st, a| {
            let neg = a.neg;
            if let Some(o) = cfg(s, st).ospf.as_mut() {
                o.log_adj = !neg;
            }
            ok()
        });
    }
    reg.def(
        "ospf",
        "router-id",
        |s, st, _| {
            if let Some(o) = cfg(s, st).ospf.as_mut() {
                o.router_id = None;
            }
            S::touch(st);
            ok()
        },
        NO_ONLY,
    );
    reg.def(
        "ospf",
        "auto-cost reference-bandwidth",
        |s, st, _| {
            if let Some(o) = cfg(s, st).ospf.as_mut() {
                o.ref_bw = 100;
            }
            S::touch(st);
            ok()
        },
        NO_ONLY,
    );

    // Interfacekommandon
    reg.d("if", "ip ospf hello-interval <n:1-65535>", set_if(|i, v| i.ospf_hello = v));
    reg.d("if", "ip ospf dead-interval <n:1-65535>", set_if(|i, v| i.ospf_dead = v));
    reg.d("if", "ip ospf cost <n:1-65535>", set_if(|i, v| i.ospf_cost = v));
    reg.d("if", "bandwidth <n:1-10000000>", set_if(|i, v| i.bandwidth = v));
    reg.def("if", "ip ospf hello-interval", clear_if(|i| i.ospf_hello = None), NO_ONLY);
    reg.def("if", "ip ospf dead-interval", clear_if(|i| i.ospf_dead = None), NO_ONLY);
    reg.def("if", "ip ospf cost", clear_if(|i| i.ospf_cost = None), NO_ONLY);
    reg.def("if", "bandwidth", clear_if(|i| i.bandwidth = None), NO_ONLY);
    reg.def("if", "clock rate", clear_if(|i| i.clock_rate = None), NO_ONLY);
    reg.d("if", "clock rate <n:300-8000000>", |s, st, a| {
        let names = s.ctx.ifs.clone();
        let v = if a.neg { None } else { Some(a.n(0)) };
        let c = cfg(s, st);
        let mut outs: Vec<String> = vec![];
        for n in &names {
            if let Some(i) = c.ifaces.get_mut(n) {
                if !n.starts_with("Serial") {
                    let m = invalid_plain();
                    if !outs.contains(&m) {
                        outs.push(m);
                    }
                    continue;
                }
                i.clock_rate = v;
            }
        }
        S::touch(st);
        out(outs.join("\n"))
    });

    // ------------------------------------------------------------ BGP: konfiguration
    reg.d(
        "config",
        "router bgp <n:1-4294967295>",
        router_only(|s, st, a| {
            let asn = a.n(0);
            let neg = a.neg;
            let c = cfg(s, st);
            if neg {
                if c.bgp.as_ref().map(|b| b.as_ == asn).unwrap_or(false) {
                    c.bgp = None;
                }
                S::touch(st);
                return ok();
            }
            if let Some(b) = &c.bgp {
                if b.as_ != 0 && b.as_ != asn {
                    return out(format!("BGP is already running; AS is {}", b.as_));
                }
            }
            if c.bgp.is_none() {
                c.bgp = Some(Bgp { as_: asn, router_id: None, neighbors: Default::default(), networks: vec![], log_changes: true });
            }
            s.mode = "bgp".into();
            s.ctx = Ctx::default();
            S::touch(st);
            ok()
        }),
    );
    reg.d("bgp", "bgp router-id <ip>", |s, st, a| {
        let ip = a.s(0).to_string();
        if let Some(b) = cfg(s, st).bgp.as_mut() {
            b.router_id = if a.neg { None } else { Some(ip) };
        }
        S::touch(st);
        ok()
    });
    reg.d("bgp", "bgp log-neighbor-changes", |s, st, a| {
        let neg = a.neg;
        if let Some(b) = cfg(s, st).bgp.as_mut() {
            b.log_changes = !neg;
        }
        ok()
    });
    reg.d("bgp", "neighbor <ip> remote-as <n:1-4294967295>", |s, st, a| {
        let ip = a.s(0).to_string();
        let asn = a.n(1);
        let neg = a.neg;
        if let Some(b) = cfg(s, st).bgp.as_mut() {
            if neg {
                b.neighbors.shift_remove(&ip);
            } else {
                b.neighbors.entry(ip).or_default().remote_as = Some(asn);
            }
        }
        S::touch(st);
        ok()
    });
    reg.d("bgp", "neighbor <ip> shutdown", |s, st, a| {
        let ip = a.s(0).to_string();
        let neg = a.neg;
        match cfg(s, st).bgp.as_mut().and_then(|b| b.neighbors.get_mut(&ip)) {
            None => return out("% Specify remote-as or peer-group commands first"),
            Some(nb) => nb.shutdown = !neg,
        }
        S::touch(st);
        ok()
    });
    reg.d("bgp", "neighbor <ip> description <text>", |s, st, a| {
        let ip = a.s(0).to_string();
        let neg = a.neg;
        // texten efter "description"
        let l = a.line.to_ascii_lowercase();
        let txt = match l.find("description") {
            Some(i) => a.line[i + "description".len()..].trim().to_string(),
            None => String::new(),
        };
        match cfg(s, st).bgp.as_mut().and_then(|b| b.neighbors.get_mut(&ip)) {
            None => out("% Specify remote-as or peer-group commands first"),
            Some(nb) => {
                nb.description = if neg { None } else { Some(txt) };
                ok()
            }
        }
    });
    reg.d("bgp", "network <ip> mask <mask>", |s, st, a| {
        let (n, m, neg) = (a.s(0).to_string(), a.s(1).to_string(), a.neg);
        bgp_network(s, st, &n, &m, neg)
    });
    reg.d("bgp", "network <ip>", |s, st, a| {
        let n = a.s(0).to_string();
        let m = class_mask(&n);
        let neg = a.neg;
        bgp_network(s, st, &n, m, neg)
    });

    reg.d("exec", "clear ip ospf process", |s, st, _| {
        if cfg_r(s, st).ospf.is_none() {
            return ok();
        }
        s.ask("Reset ALL OSPF processes? [no]: ", false, |s, st, a| {
            if a.to_lowercase().starts_with('y') {
                let pre = format!("{}|", s.dev);
                st.dyn_mem.ospf.retain(|k, _| !k.starts_with(&pre));
                S::touch(st);
            }
            ok()
        });
        ok()
    });
    reg.d("exec", "clear ip bgp *", |_, st, _| {
        S::touch(st);
        ok()
    });
    reg.d("exec", "clear ip bgp <ip>", |_, st, _| {
        S::touch(st);
        ok()
    });

    // ------------------------------------------------------------ OSPF: show
    reg.show(
        "ip ospf neighbor",
        |s, st, _| {
            let r = match ospf_r(s, st) {
                Some(r) => r,
                None => return ok(),
            };
            let mut o = vec![String::new(), "Neighbor ID     Pri   State           Dead Time   Address         Interface".to_string()];
            for nb in &r.nbrs {
                let stt = format!("FULL/{}", if nb.role == "-" { "  -" } else { &nb.role });
                o.push(format!(
                    "{}{}   {}{}{}{}",
                    U::pad(&nb.rid, 16),
                    U::pad_l(if nb.serial { 0 } else { 1 }, 3),
                    U::pad(stt, 16),
                    U::pad(hms(dead_left(st, nb.dead)), 12),
                    U::pad(&nb.ip, 16),
                    nb.iface
                ));
            }
            out(o.join("\n"))
        },
        true,
    );
    reg.show(
        "ip ospf neighbor detail",
        |s, st, _| {
            let r = match ospf_r(s, st) {
                Some(r) => r,
                None => return ok(),
            };
            let o: Vec<String> = r
                .nbrs
                .iter()
                .map(|nb| {
                    let up = st.time - DR::since(st, &format!("{}|{}@{}", s.dev, nb.rid, nb.iface));
                    format!(
                        " Neighbor {}, interface address {}\n    In the area 0 via interface {}\n    Neighbor priority is {}, State is FULL, 6 state changes\n    Options is 0x12 in Hello (E-bit, L-bit)\n    Dead timer due in {}\n    Neighbor is up for {}",
                        nb.rid,
                        nb.ip,
                        nb.iface,
                        if nb.serial { 0 } else { 1 },
                        hms(dead_left(st, nb.dead)),
                        hms(up)
                    )
                })
                .collect();
            out(o.join("\n"))
        },
        true,
    );
    reg.show(
        "ip ospf interface brief",
        |s, st, _| {
            let r = match ospf_r(s, st) {
                Some(r) => r,
                None => return ok(),
            };
            let pid = cfg_r(s, st).ospf.as_ref().map(|o| o.pid).unwrap_or(0);
            let mut o = vec!["Interface    PID   Area            IP Address/Mask    Cost  State Nbrs F/C".to_string()];
            for n in SH::iface_names(st.dev(&s.dev)) {
                let f = match r.ifs.get(&n) {
                    Some(f) => f,
                    None => continue,
                };
                let nb = r.nbrs.iter().filter(|x| x.iface == n).count();
                o.push(format!(
                    "{}{}{}{}{}{}{}/{}",
                    U::pad(U::short_if(&n), 13),
                    U::pad(pid, 6),
                    U::pad(f.area, 16),
                    U::pad(format!("{}/{}", f.ip, prefix(&f.mask)), 19),
                    U::pad(f.cost, 6),
                    U::pad(if_role(&r, &n), 6),
                    nb,
                    nb
                ));
            }
            out(o.join("\n"))
        },
        true,
    );
    reg.show(
        "ip ospf interface",
        |s, st, _| {
            let r = match ospf_r(s, st) {
                Some(r) => r,
                None => return ok(),
            };
            let names: Vec<String> = SH::iface_names(st.dev(&s.dev)).into_iter().filter(|n| r.ifs.contains_key(n)).collect();
            out(names.iter().map(|n| if_detail(s, st, &r, n)).collect::<Vec<_>>().join("\n"))
        },
        true,
    );
    reg.show(
        "ip ospf interface <if>",
        |s, st, a| {
            let n = a.s(0).to_string();
            if !cfg_r(s, st).ifaces.contains_key(&n) {
                return out(invalid_plain());
            }
            match ospf_r(s, st) {
                None => ok(),
                Some(r) => out(if_detail(s, st, &r, &n)),
            }
        },
        true,
    );
    reg.show(
        "ip ospf",
        |s, st, _| {
            let r = ospf_r(s, st);
            let o = cfg_r(s, st).ospf.clone();
            let (r, o) = match (r, o) {
                (Some(r), Some(o)) => (r, o),
                _ => return out("%OSPF: Router process not running"),
            };
            let nloop = r.ifs.values().filter(|f| f.is_loop).count();
            let lines = [
                format!(" Routing Process \"ospf {}\" with ID {}", o.pid, r.rid.clone().unwrap_or_default()),
                " Start time: 00:00:05.120, Time elapsed: 12d05h".to_string(),
                " Supports only single TOS(TOS0) routes".to_string(),
                " Supports opaque LSA".to_string(),
                if o.default_originate { " It is an autonomous system boundary router".to_string() } else { String::new() },
                " Router is not originating router-LSAs with maximum metric".to_string(),
                " Initial SPF schedule delay 5000 msecs".to_string(),
                " Number of areas in this router is 1. 1 normal 0 stub 0 nssa".to_string(),
                format!(" Reference bandwidth unit is {} mbps", o.ref_bw),
                "    Area BACKBONE(0)".to_string(),
                format!("        Number of interfaces in this area is {} ({} loopback)", r.ifs.len(), nloop),
                "        Area has no authentication".to_string(),
                "        SPF algorithm last executed 00:01:12.344 ago".to_string(),
                "        SPF algorithm executed 7 times".to_string(),
            ];
            out(lines.into_iter().filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n"))
        },
        true,
    );
    reg.show(
        "ip ospf database",
        |s, st, _| {
            let d = S::get(st);
            let r = match d.dynr.ospf.get(&s.dev) {
                Some(r) => r,
                None => return ok(),
            };
            let pid = cfg_r(s, st).ospf.as_ref().map(|o| o.pid).unwrap_or(0);
            let mut o = vec![
                String::new(),
                format!("            OSPF Router with ID ({}) (Process ID {})", r.rid.clone().unwrap_or_default(), pid),
                String::new(),
                "                Router Link States (Area 0)".to_string(),
                String::new(),
                "Link ID         ADV Router      Age         Seq#       Checksum Link count".to_string(),
            ];
            for (id, x) in &d.dynr.ospf {
                if !r.dist.contains_key(id) && *id != r.id {
                    continue;
                }
                let rid = x.rid.clone().unwrap_or_default();
                o.push(format!(
                    "{}{}{}{}{}{}",
                    U::pad(&rid, 16),
                    U::pad(&rid, 16),
                    U::pad(300 + (U::hash(id) % 900), 12),
                    U::pad(format!("0x8000000{}", 3 + U::hash(id) % 6), 11),
                    U::pad(format!("0x00{:X}", U::hash(&rid) % 0xFFFF), 9),
                    x.ifs.len()
                ));
            }
            out(o.join("\n"))
        },
        true,
    );
    reg.show("ip route ospf", |s, st, _| out(SH::ip_route(st, &s.dev.clone(), Some("ospf"))), true);
    reg.show("ip route bgp", |s, st, _| out(SH::ip_route(st, &s.dev.clone(), Some("bgp"))), true);

    // ------------------------------------------------------------ show ip protocols
    reg.show(
        "ip protocols",
        |s, st, _| {
            if !s.is_router(st) && !cfg_r(s, st).ip_routing {
                return ok();
            }
            let mut o: Vec<String> = vec!["*** IP Routing is NSF aware ***".into(), String::new()];
            let r = ospf_r(s, st);
            let q = cfg_r(s, st).ospf.clone();
            let b = cfg_r(s, st).bgp.clone();
            let time = st.time;
            if let (Some(q), Some(r)) = (&q, &r) {
                o.push(format!("Routing Protocol is \"ospf {}\"", q.pid));
                o.push("  Outgoing update filter list for all interfaces is not set".into());
                o.push("  Incoming update filter list for all interfaces is not set".into());
                o.push(format!("  Router ID {}", r.rid.clone().unwrap_or_default()));
                if q.default_originate {
                    o.push("  It is an autonomous system boundary router".into());
                    o.push("  Redistributing External Routes from,".into());
                }
                o.push("  Number of areas in this router is 1. 1 normal 0 stub 0 nssa".into());
                o.push("  Maximum path: 4".into());
                o.push("  Routing for Networks:".into());
                for x in &q.networks {
                    o.push(format!("    {} {} area {}", x.net, x.wild, x.area));
                }
                let pas: Vec<&String> = r.ifs.iter().filter(|(_, f)| f.passive).map(|(n, _)| n).collect();
                if !pas.is_empty() {
                    o.push("  Passive Interface(s):".into());
                    for n in pas {
                        o.push(format!("    {}", n));
                    }
                }
                o.push("  Routing Information Sources:".into());
                o.push("    Gateway         Distance      Last Update".into());
                for nb in &r.nbrs {
                    o.push(format!(
                        "    {}{}      {}",
                        U::pad(&nb.rid, 16),
                        U::pad_l(110, 8),
                        hms(time - DR::since(st, &format!("{}|{}@{}", s.dev, nb.rid, nb.iface)))
                    ));
                }
                o.push("  Distance: (default is 110)".into());
                o.push(String::new());
            }
            if let Some(b) = &b {
                if b.as_ != 0 {
                    let bs = bgp_r(s, st);
                    o.push(format!("Routing Protocol is \"bgp {}\"", b.as_));
                    o.push("  Outgoing update filter list for all interfaces is not set".into());
                    o.push("  Incoming update filter list for all interfaces is not set".into());
                    o.push("  IGP synchronization is disabled".into());
                    o.push("  Automatic route summarization is disabled".into());
                    o.push("  Neighbor(s):".into());
                    o.push("    Address          FiltIn FiltOut DistIn DistOut Weight RouteMap".into());
                    for ip in b.neighbors.keys() {
                        o.push(format!("    {}", ip));
                    }
                    o.push("  Maximum path: 1".into());
                    o.push("  Routing Information Sources:".into());
                    o.push("    Gateway         Distance      Last Update".into());
                    if let Some(bs) = &bs {
                        for p in bs.peers.iter().filter(|p| p.up) {
                            o.push(format!(
                                "    {}{}      {}",
                                U::pad(&p.ip, 16),
                                U::pad_l(20, 8),
                                hms(time - DR::since(st, &format!("b|{}|{}", s.dev, p.ip)))
                            ));
                        }
                    }
                    o.push("  Distance: external 20 internal 200 local 200".into());
                    o.push(String::new());
                }
            }
            out(o.join("\n"))
        },
        true,
    );

    // ------------------------------------------------------------ BGP: show
    reg.show(
        "ip bgp summary",
        |s, st, _| {
            let b = match bgp_r(s, st) {
                Some(b) => b,
                None => return out("% BGP not active"),
            };
            let pfx: HashSet<String> = b.table.iter().map(|t| format!("{}/{}", t.net, t.mask)).collect();
            let np = pfx.len();
            let ver = 1 + np * 2;
            let mut o = vec![
                format!("BGP router identifier {}, local AS number {}", b.rid.clone().unwrap_or("0.0.0.0".into()), b.as_),
                format!("BGP table version is {}, main routing table version {}", ver, ver),
            ];
            if np > 0 {
                o.push(format!("{} network entries using {} bytes of memory", np, np * 144));
                o.push(format!("{} path entries using {} bytes of memory", b.table.len(), b.table.len() * 80));
                o.push(format!("{}/{} BGP path/bestpath attribute entries using {} bytes of memory", np + 1, np, (np + 1) * 136));
                o.push(format!("{} BGP AS-PATH entries using {} bytes of memory", np, np * 40));
                o.push("0 BGP route-map cache entries using 0 bytes of memory".into());
                o.push("0 BGP filter-list cache entries using 0 bytes of memory".into());
                o.push(format!("BGP using {} total bytes of memory", np * 400));
                o.push(format!("BGP activity {}/0 prefixes, {}/0 paths, scan interval 60 secs", np, b.table.len()));
            }
            o.push(String::new());
            o.push("Neighbor        V           AS MsgRcvd MsgSent   TblVer  InQ OutQ Up/Down  State/PfxRcd".into());
            for p in &b.peers {
                let m = msgs(st, &s.dev, p.up, &p.ip);
                o.push(format!(
                    "{}4{}{}{}{}{}{} {}{}",
                    U::pad(&p.ip, 16),
                    U::pad_l(p.remote_as.map(|x| x.to_string()).unwrap_or("?".into()), 13),
                    U::pad_l(m.0, 8),
                    U::pad_l(m.1, 8),
                    U::pad_l(if p.up { ver } else { 1 }, 9),
                    U::pad_l(0, 5),
                    U::pad_l(0, 5),
                    U::pad(if p.up { bgp_up(st, &s.dev, &p.ip) } else { "never".to_string() }, 9),
                    if p.up { U::pad_l(p.pfx, 8) } else { p.state.clone() }
                ));
            }
            out(o.join("\n"))
        },
        true,
    );
    reg.show(
        "ip bgp",
        |s, st, _| {
            let b = match bgp_r(s, st) {
                Some(b) => b,
                None => return out("% BGP not active"),
            };
            let d = S::get(st);
            let conn: Vec<String> = S::dev_eps(&d, &s.dev)
                .iter()
                .filter(|e| e.up && e.ip.is_some())
                .map(|e| format!("{}/{}", U::network(e.ip.as_deref().unwrap(), e.mask.as_deref().unwrap_or("")), e.mask.clone().unwrap_or_default()))
                .collect();
            let mut o = vec![
                format!("BGP table version is {}, local router ID is {}", 1 + b.table.len() * 2, b.rid.clone().unwrap_or("0.0.0.0".into())),
                "Status codes: s suppressed, d damped, h history, * valid, > best, i - internal,".to_string(),
                "              r RIB-failure, S Stale, m multipath, b backup-path, f RT-Filter,".to_string(),
                "              x best-external, a additional-path, c RIB-compressed,".to_string(),
                "Origin codes: i - IGP, e - EGP, ? - incomplete".to_string(),
                "RPKI validation codes: V valid, I invalid, N Not found".to_string(),
                String::new(),
                "     Network          Next Hop            Metric LocPrf Weight Path".to_string(),
            ];
            let mut rows = b.table.clone();
            rows.sort_by(|x, y| {
                U::ipn(&x.net).cmp(&U::ipn(&y.net)).then((y.best as i32).cmp(&(x.best as i32)))
            });
            if rows.is_empty() {
                return ok();
            }
            let mut last: Option<String> = None;
            for t in &rows {
                let k = format!("{}/{}", t.net, t.mask);
                let classful = t.mask == class_mask(&t.net);
                let shown = if last.as_deref() == Some(k.as_str()) {
                    String::new()
                } else if classful {
                    t.net.clone()
                } else {
                    format!("{}/{}", t.net, prefix(&t.mask))
                };
                last = Some(k.clone());
                let rib = t.best && !t.local && conn.contains(&k);
                let code = format!(" {}{}", if rib { "r" } else { "*" }, if t.best { ">" } else { " " });
                let hop = if t.local { "0.0.0.0".to_string() } else { t.nh.clone() };
                let metric = if t.local || t.path.len() == 1 { "0".to_string() } else { String::new() };
                let weight = if t.local { 32768 } else { 0 };
                let path = if t.path.is_empty() { String::new() } else { format!("{} ", t.path.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ")) };
                o.push(format!("{}  {}{}{}       {} {}i", code, U::pad(shown, 17), U::pad(hop, 20), U::pad_l(metric, 6), U::pad_l(weight, 6), path));
            }
            out(o.join("\n"))
        },
        true,
    );
    reg.show(
        "ip bgp neighbors",
        |s, st, _| {
            let b = match bgp_r(s, st) {
                Some(b) => b,
                None => return out("% BGP not active"),
            };
            let o: Vec<String> = b
                .peers
                .iter()
                .map(|p| {
                    let m = msgs(st, &s.dev, p.up, &p.ip);
                    let state = if p.up { format!("Established, up for {}", bgp_up(st, &s.dev, &p.ip)) } else { p.state.split(' ').next().unwrap_or("").to_string() };
                    format!(
                        "BGP neighbor is {},  remote AS {}, external link\n  BGP version 4, remote router ID {}\n  BGP state = {}\n  Last read 00:00:21, last write 00:00:12, hold time is 180, keepalive interval is 60 seconds\n  Message statistics:\n    InQ depth is 0\n    OutQ depth is 0\n                         Sent       Rcvd\n    Total:          {}  {}{}",
                        p.ip,
                        p.remote_as.map(|x| x.to_string()).unwrap_or("?".into()),
                        if p.up { p.ip.clone() } else { "0.0.0.0".into() },
                        state,
                        U::pad_l(m.1, 9),
                        U::pad_l(m.0, 9),
                        if p.reason.as_deref() == Some("bad-as") { "\n  Last reset 00:00:12, due to BGP Notification sent, peer in wrong AS" } else { "" }
                    )
                })
                .collect();
            out(o.join("\n\n"))
        },
        true,
    );
}

fn dead_left(st: &State, dead: i64) -> f64 {
    let t = st.time.floor() as i64;
    (dead - (t % 10) - 3) as f64
}

fn msgs(st: &State, dev: &str, up: bool, ip: &str) -> (i64, i64) {
    if !up {
        return (0, 0);
    }
    let age = st.time - DR::since(st, &format!("b|{}|{}", dev, ip));
    let n = 4 + (age / 60.0).floor() as i64;
    (n, n + 2)
}
fn bgp_up(st: &State, dev: &str, ip: &str) -> String {
    hms(st.time - DR::since(st, &format!("b|{}|{}", dev, ip)))
}

fn if_role(r: &OspfRouter, n: &str) -> &'static str {
    let f = match r.ifs.get(n) {
        Some(f) => f,
        None => return "DOWN",
    };
    if f.is_loop {
        return "LOOP";
    }
    if !f.up {
        return "DOWN";
    }
    if f.serial {
        return "P2P";
    }
    let nbs: Vec<_> = r.nbrs.iter().filter(|x| x.iface == n).collect();
    if nbs.is_empty() {
        return "DR";
    }
    if nbs.iter().any(|x| x.role == "DR") { "BDR" } else { "DR" }
}

fn if_detail(s: &IosSession, st: &State, r: &OspfRouter, n: &str) -> String {
    let c = cfg_r(s, st);
    let i = match c.ifaces.get(n) {
        Some(i) => i,
        None => return String::new(),
    };
    let f = r.ifs.get(n);
    let up = f.map(|f| f.up).unwrap_or(false);
    let head = format!(
        "{} is {}, line protocol is {}",
        n,
        if i.shutdown { "administratively down" } else if up { "up" } else { "down" },
        if up { "up" } else { "down" }
    );
    let f = match f {
        Some(f) => f,
        None => {
            return format!(
                "{}\n  Internet Address {}\n  %OSPF: OSPF not enabled on {}",
                head,
                i.ip.as_ref().map(|a| format!("{}/{}", a.addr, prefix(&a.mask))).unwrap_or("unassigned".into()),
                n
            );
        }
    };
    let role = if_role(r, n);
    let nbs: Vec<_> = r.nbrs.iter().filter(|x| x.iface == n).collect();
    let pid = c.ospf.as_ref().map(|o| o.pid).unwrap_or(0);
    let mut o = vec![
        head,
        format!("  Internet Address {}/{}, Area {}, Attached via Network Statement", f.ip, prefix(&f.mask), f.area),
        format!(
            "  Process ID {}, Router ID {}, Network Type {}, Cost: {}",
            pid,
            r.rid.clone().unwrap_or_default(),
            if f.is_loop { "LOOPBACK" } else if f.serial { "POINT_TO_POINT" } else { "BROADCAST" },
            f.cost
        ),
        "  Topology-MTID    Cost    Disabled    Shutdown      Topology Name".to_string(),
        format!("        0           {}no          no            Base", U::pad(f.cost, 10)),
    ];
    if f.is_loop {
        o.push("  Loopback interface is treated as a stub Host".into());
        return o.join("\n");
    }
    if f.serial {
        o.push("  Transmit Delay is 1 sec, State POINT_TO_POINT".into());
    } else {
        o.push(format!("  Transmit Delay is 1 sec, State {}, Priority 1", role));
        let dr: Option<(String, String)> = if role == "DR" {
            Some((r.rid.clone().unwrap_or_default(), f.ip.clone()))
        } else {
            nbs.iter().find(|x| x.role == "DR").map(|x| (x.rid.clone(), x.ip.clone()))
        };
        if let Some((rid, ip)) = dr {
            o.push(format!("  Designated Router (ID) {}, Interface address {}", rid, ip));
        }
        let bdr: Option<(String, String)> = if role == "BDR" {
            Some((r.rid.clone().unwrap_or_default(), f.ip.clone()))
        } else {
            nbs.iter().find(|x| x.role == "BDR").map(|x| (x.rid.clone(), x.ip.clone()))
        };
        match bdr {
            Some((rid, ip)) => o.push(format!("  Backup Designated router (ID) {}, Interface address {}", rid, ip)),
            None => o.push("  No backup designated router on this network".into()),
        }
    }
    o.push(format!("  Timer intervals configured, Hello {}, Dead {}, Wait {}, Retransmit 5", f.hello, f.dead, f.dead));
    o.push(format!("    oob-resync timeout {}", f.dead));
    if f.passive {
        o.push("    No Hellos (Passive interface)".into());
    } else {
        let t = st.time.floor() as i64;
        o.push(format!("    Hello due in 00:00:0{}", (f.hello - 1 - (t % f.hello.max(1)) + 10) % 10));
    }
    o.push("  Supports Link-local Signaling (LLS)".into());
    o.push("  Index 1/1, flood queue length 0".into());
    o.push("  Last flood scan length is 1, maximum is 3".into());
    o.push(format!("  Neighbor Count is {}, Adjacent neighbor count is {}", nbs.len(), nbs.len()));
    for x in &nbs {
        let suffix = if x.role == "DR" {
            "  (Designated Router)"
        } else if x.role == "BDR" {
            "  (Backup Designated Router)"
        } else {
            ""
        };
        o.push(format!("    Adjacent with neighbor {}{}", x.rid, suffix));
    }
    o.push("  Suppress hello for 0 neighbor(s)".into());
    o.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::ios::IosSession;
    use crate::net::model::build_golden;

    fn first_router(st: &State) -> String {
        st.devices.iter().find(|(_, d)| d.kind == Kind::Router && d.os == Os::Ios).map(|(k, _)| k.clone()).unwrap()
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

    #[test]
    fn registrerar_utan_panik() {
        let mut reg = Reg::default();
        register(&mut reg);
        assert!(reg.tries.contains_key("ospf") && reg.tries.contains_key("bgp"));
    }

    #[test]
    fn ospf_konfiguration() {
        let mut st = State::new(build_golden(14));
        let r = first_router(&st);
        // börja utan befintlig OSPF/BGP
        st.dev_mut(&r).cfg_mut().ospf = None;
        st.dev_mut(&r).cfg_mut().bgp = None;
        let mut s = IosSession::new(&st, &r, "console", true, None);
        s.mode = "config".into();
        run(&mut s, &mut st, &["router ospf 1", "router-id 1.1.1.1", "network 10.0.0.0 0.0.0.255 area 0", "passive-interface default", "auto-cost reference-bandwidth 1000", "default-information originate always"]);
        assert_eq!(s.mode, "ospf");
        let o = st.dev(&r).cfg().ospf.clone().unwrap();
        assert_eq!(o.pid, 1);
        assert_eq!(o.router_id.as_deref(), Some("1.1.1.1"));
        assert_eq!(o.networks.len(), 1);
        assert_eq!(o.networks[0].area, 0);
        assert!(o.passive_default && o.default_originate && o.default_always);
        assert_eq!(o.ref_bw, 1000);
        run(&mut s, &mut st, &["exit"]);
        assert_eq!(s.mode, "config");
    }

    #[test]
    fn bgp_konfiguration() {
        let mut st = State::new(build_golden(14));
        let r = first_router(&st);
        // börja utan befintlig OSPF/BGP
        st.dev_mut(&r).cfg_mut().ospf = None;
        st.dev_mut(&r).cfg_mut().bgp = None;
        let mut s = IosSession::new(&st, &r, "console", true, None);
        s.mode = "config".into();
        run(&mut s, &mut st, &["router bgp 65001", "bgp router-id 2.2.2.2", "neighbor 203.0.113.1 remote-as 65000", "network 192.0.2.0 mask 255.255.255.0"]);
        assert_eq!(s.mode, "bgp");
        let b = st.dev(&r).cfg().bgp.clone().unwrap();
        assert_eq!(b.as_, 65001);
        assert_eq!(b.neighbors["203.0.113.1"].remote_as, Some(65000));
        assert_eq!(b.networks.len(), 1);
        run(&mut s, &mut st, &["neighbor 203.0.113.1 shutdown"]);
        assert!(st.dev(&r).cfg().bgp.as_ref().unwrap().neighbors["203.0.113.1"].shutdown);
    }

    #[test]
    fn interface_ospf_och_klocka() {
        let mut st = State::new(build_golden(14));
        let r = first_router(&st);
        let ifn = st.dev(&r).cfg().ifaces.keys().find(|k| k.starts_with("GigabitEthernet")).cloned().unwrap();
        let mut s = IosSession::new(&st, &r, "console", true, None);
        s.mode = "if".into();
        s.ctx.ifs = vec![ifn.clone()];
        run(&mut s, &mut st, &["ip ospf hello-interval 5", "ip ospf dead-interval 20", "ip ospf cost 7", "bandwidth 512"]);
        let i = st.dev(&r).cfg().ifaces[&ifn].clone();
        assert_eq!((i.ospf_hello, i.ospf_dead, i.ospf_cost, i.bandwidth), (Some(5), Some(20), Some(7), Some(512)));
        run(&mut s, &mut st, &["no ip ospf cost"]);
        assert_eq!(st.dev(&r).cfg().ifaces[&ifn].ospf_cost, None);
    }
}
