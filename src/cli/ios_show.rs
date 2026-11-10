//! show-utdata för IOS. Port av js/cli/ios_show.js. Alla funktioner returnerar texten som IOS skulle skriva.
#![allow(unused_variables, dead_code)]
use crate::net::dynroute;
use crate::net::model::{Ace, AddrSpec, Device, IosConfig, Iface, Kind, NatDir, Os};
use crate::net::sim::{self, Derived, State};
use crate::net::util::{self as U, pad, pad_l};
use std::rc::Rc;

/// Filter för show mac address-table
#[derive(Clone, Debug, Default)]
pub struct MacFilter {
    pub address: Option<String>,
    pub iface: Option<String>,
    pub vlan: Option<u16>,
}

// ---------------------------------------------------------------- små hjälpare

fn is_sw(d: &Device) -> bool {
    d.kind == Kind::Switch
}
fn two(n: i64) -> String {
    if n < 10 { format!("0{}", n) } else { n.to_string() }
}
/// JS-tal till text: heltal utan decimaler
fn num(x: f64) -> String {
    if x.is_finite() && x.fract() == 0.0 { format!("{}", x as i64) } else { format!("{}", x) }
}
fn fl(x: f64) -> i64 {
    x.floor() as i64
}
fn nz(o: Option<i64>) -> Option<i64> {
    o.filter(|v| *v != 0)
}
fn prefix(mask: &str) -> u32 {
    U::mask_to_prefix(mask).unwrap_or(0)
}
fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}
fn slice_chars(s: &str, a: usize, b: usize) -> String {
    s.chars().skip(a).take(b.saturating_sub(a)).collect()
}
fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())
}
fn vlan4(v: impl std::fmt::Display) -> String {
    format!("VLAN{:0>4}", v.to_string())
}
fn get_d(st: &mut State) -> Rc<Derived> {
    sim::get(st)
}

// ---------------------------------------------------------------- interfacenamn och portstatus

/// Interfacenamnen i den ordning IOS visar dem (ifaceNames)
pub fn iface_names(d: &Device) -> Vec<String> {
    let c = match &d.config {
        Some(c) => c,
        None => return vec![],
    };
    fn rank(n: &str) -> (i64, i64, i64) {
        if n.starts_with("Embedded") {
            return (0, 0, 0);
        }
        if let Some(r) = n.strip_prefix("GigabitEthernet0/") {
            let (a, b) = match r.split_once('.') {
                Some((a, b)) => (a, Some(b)),
                None => (r, None),
            };
            if is_digits(a) && b.map(is_digits).unwrap_or(true) {
                return (1, a.parse().unwrap_or(0), b.map(|b| b.parse().unwrap_or(0)).unwrap_or(-1));
            }
        }
        if let Some(r) = n.strip_prefix("Serial0/0/") {
            if is_digits(r) {
                return (1, 50 + r.parse::<i64>().unwrap_or(0), -1);
            }
        }
        if let Some(r) = n.strip_prefix("Loopback") {
            if is_digits(r) {
                return (2, r.parse().unwrap_or(0), 0);
            }
        }
        if let Some(r) = n.strip_prefix("Vlan") {
            if is_digits(r) {
                return (3, r.parse().unwrap_or(0), 0);
            }
        }
        (4, 0, 0)
    }
    let mut names: Vec<String> = c.ifaces.keys().cloned().collect();
    names.sort_by_key(|n| rank(n));
    names
}

/// Fysiska portar (physPorts)
pub fn phys_ports(d: &Device) -> Vec<String> {
    iface_names(d)
        .into_iter()
        .filter(|n| match n.strip_prefix("GigabitEthernet0/") {
            Some(r) => is_digits(r),
            None => false,
        })
        .collect()
}

fn port_state_i(st: &State, dd: &Derived, d: &Device, n: &str) -> String {
    let i = match d.ifc(n) {
        Some(i) => i,
        None => return "notconnect".into(),
    };
    let p = dd.ports.get(&sim::key(&d.id, n));
    if i.shutdown {
        return "disabled".into();
    }
    if d.rt.errdisabled.contains_key(n) {
        return "err-disabled".into();
    }
    if p.map(|p| !p.up).unwrap_or(true) {
        return "notconnect".into();
    }
    if is_sw(d) && sim::op_mode(st, &d.id, n) == "access" && !sim::vlan_exists(d, i.access_vlan) {
        return "inactive".into();
    }
    "connected".into()
}

/// "connected", "notconnect", "disabled", "err-disabled" ... (portState)
pub fn port_state(st: &mut State, dev: &str, n: &str) -> String {
    let dd = get_d(st);
    port_state_i(st, &dd, st.dev(dev), n)
}

// ---------------------------------------------------------------- running-config

fn port_name(p: i64) -> String {
    match p {
        21 => "ftp",
        22 => "22",
        23 => "telnet",
        25 => "smtp",
        53 => "domain",
        67 => "bootps",
        68 => "bootpc",
        80 => "www",
        110 => "pop3",
        123 => "ntp",
        161 => "snmp",
        443 => "443",
        514 => "syslog",
        _ => return p.to_string(),
    }
    .to_string()
}

/// Texten för en ACL-rad, t.ex. "deny   ip 192.168.1.128 0.0.0.63 192.168.1.0 0.0.0.255" (aceText)
pub fn ace_text(extended: bool, r: &Ace) -> String {
    if let Some(rm) = r.remark.as_ref().filter(|s| !s.is_empty()) {
        return format!("remark {}", rm);
    }
    let addr = |a: &Option<AddrSpec>| -> String {
        let a = match a {
            None => return "any".into(),
            Some(a) => a,
        };
        if a.any {
            return "any".into();
        }
        if let Some(h) = a.host.as_ref().filter(|s| !s.is_empty()) {
            return format!("host {}", h);
        }
        let ip = a.ip.clone().unwrap_or_default();
        let wild_zero = a.wild.as_deref().map(|w| w.is_empty() || w == "0.0.0.0").unwrap_or(true);
        if !extended && wild_zero {
            return ip;
        }
        format!("{} {}", ip, a.wild.clone().unwrap_or_default())
    };
    let act = if r.action == "deny" { "deny  " } else { "permit" };
    if !extended {
        return format!("{} {}", act, addr(&r.src));
    }
    let mut s = format!("{} {} {} {}", act, r.proto.clone().unwrap_or_else(|| "ip".into()), addr(&r.src), addr(&r.dst));
    if let Some(dp) = nz(r.dport) {
        if r.dop.as_deref() == Some("range") {
            s += &format!(" range {} {}", port_name(dp), port_name(r.dport2.unwrap_or(0)));
        } else {
            s += &format!(" {} {}", r.dop.clone().unwrap_or_else(|| "eq".into()), port_name(dp));
        }
    }
    if r.log {
        s += " log";
    }
    s
}

fn line_cfg(l: &crate::net::model::Line, is_con: bool, enc: bool) -> Vec<String> {
    let mut out = vec![];
    if let Some(ac) = l.access_class.as_ref().filter(|s| !s.is_empty()) {
        if !is_con {
            out.push(format!(" access-class {} in", ac));
        }
    }
    if let Some(p) = l.priv_level {
        out.push(format!(" privilege level {}", p));
    }
    if is_con && l.log_sync {
        out.push(" logging synchronous".into());
    }
    let pw = l.password.as_ref().filter(|s| !s.is_empty());
    if let Some(pw) = pw {
        out.push(if enc { format!(" password 7 {}", fake_type7(pw)) } else { format!(" password {}", pw) });
    }
    if l.login_local {
        out.push(" login local".into());
    } else if pw.is_some() && !is_con {
        out.push(" login".into());
    }
    if is_con && l.speed != 0 && l.speed != 9600 {
        out.push(format!(" speed {}", l.speed));
    }
    if !is_con && l.transport != "all" {
        out.push(format!(" transport input {}", l.transport));
    }
    for v in l.x.values() {
        out.push(format!(" {}", v));
    }
    out
}

fn fake_type7(s: &str) -> String {
    let h = format!("{:X}", U::hash(s));
    let full = format!("0{}{}", h, h);
    full.chars().take(18).collect()
}

/// Typ 9-hash av ett lösenord (hashSecret)
pub fn hash_secret(s: &str) -> String {
    let alpha: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars().collect();
    let mut out = String::from("$9$");
    let mut h = U::hash(s);
    for _ in 0..14 {
        out.push(alpha[(h as usize) % alpha.len()]);
        h = U::hash(&format!("{}{}", out, s));
    }
    out.push('$');
    for _ in 0..43 {
        out.push(alpha[(h as usize) % alpha.len()]);
        h = U::hash(&out);
    }
    out
}

fn iface_block(d: &Device, n: &str, c: &IosConfig) -> Vec<String> {
    let i: &Iface = match c.ifaces.get(n) {
        Some(i) => i,
        None => return vec![],
    };
    let mut o: Vec<String> = vec![format!("interface {}", n)];
    if !i.description.is_empty() {
        o.push(format!(" description {}", i.description));
    }
    if i.parent.is_some() {
        if let Some(v) = nz(i.dot1q.map(|v| v as i64)) {
            o.push(format!(" encapsulation dot1Q {}", v));
        }
    }
    let svi = i.svi.map(|v| v != 0).unwrap_or(false);
    if is_sw(d) && !svi && !i.is_loop {
        let m = i.mode.as_str();
        if m == "trunk" || m == "dynamic auto" || m == "dynamic desirable" || m == "access" {
            if i.access_vlan != 1 {
                o.push(format!(" switchport access vlan {}", i.access_vlan));
            }
            if i.encap.as_deref() == Some("dot1q") {
                o.push(" switchport trunk encapsulation dot1q".into());
            }
            if i.native != 1 {
                o.push(format!(" switchport trunk native vlan {}", i.native));
            }
            if !i.allowed_all() {
                o.push(format!(" switchport trunk allowed vlan {}", U::vlan_list_str(&i.allowed_list())));
            }
            if m != "dynamic auto" {
                o.push(format!(" switchport mode {}", m));
            }
            if i.nonegotiate {
                o.push(" switchport nonegotiate".into());
            }
            if let Some(ps) = &i.port_sec {
                o.push(" switchport port-security".into());
                if ps.max != 1 {
                    o.push(format!(" switchport port-security maximum {}", ps.max));
                }
                if ps.violation != "shutdown" {
                    o.push(format!(" switchport port-security violation {}", ps.violation));
                }
                if ps.sticky {
                    o.push(" switchport port-security mac-address sticky".into());
                    for mac in &ps.macs {
                        o.push(format!(" switchport port-security mac-address sticky {}", U::mac_dots(mac)));
                    }
                }
            }
        }
        if i.speed != "auto" {
            o.push(format!(" speed {}", i.speed));
        }
        if i.duplex != "auto" {
            o.push(format!(" duplex {}", i.duplex));
        }
        if i.poe == "never" {
            o.push(" power inline never".into());
        }
        if i.portfast && i.mode != "trunk" {
            o.push(" spanning-tree portfast".into());
        } else if i.portfast && i.mode == "trunk" {
            o.push(" spanning-tree portfast trunk".into());
        }
    } else {
        if let Some(b) = nz(i.bandwidth) {
            o.push(format!(" bandwidth {}", b));
        }
        if let Some(ip) = &i.ip {
            o.push(format!(" ip address {} {}", ip.addr, ip.mask));
        } else if i.parent.is_none() {
            o.push(" no ip address".into());
        }
        if let Some(v) = nz(i.ospf_hello) {
            o.push(format!(" ip ospf hello-interval {}", v));
        }
        if let Some(v) = nz(i.ospf_dead) {
            o.push(format!(" ip ospf dead-interval {}", v));
        }
        if let Some(v) = nz(i.ospf_cost) {
            o.push(format!(" ip ospf cost {}", v));
        }
        if let Some(a) = i.acl_in.as_ref().filter(|s| !s.is_empty()) {
            o.push(format!(" ip access-group {} in", a));
        }
        if let Some(a) = i.acl_out.as_ref().filter(|s| !s.is_empty()) {
            o.push(format!(" ip access-group {} out", a));
        }
        if let Some(nd) = i.nat_dir {
            o.push(format!(" ip nat {}", if nd == NatDir::Inside { "inside" } else { "outside" }));
        }
        if let Some(v) = nz(i.mtu) {
            o.push(format!(" ip mtu {}", v));
        }
        if let Some(v) = nz(i.adjust_mss) {
            o.push(format!(" ip tcp adjust-mss {}", v));
        }
        if let Some(cm) = i.crypto_map.as_ref().filter(|s| !s.is_empty()) {
            o.push(format!(" crypto map {}", cm));
        }
        if n.starts_with("Serial") {
            if let Some(v) = nz(i.clock_rate) {
                o.push(format!(" clock rate {}", v));
            }
        }
        if !svi && i.parent.is_none() && !i.internal && !i.is_loop && !n.starts_with("Serial") && d.kind == Kind::Router {
            o.push(if i.duplex == "auto" { " duplex auto".to_string() } else { format!(" duplex {}", i.duplex) });
            o.push(if i.speed == "auto" { " speed auto".to_string() } else { format!(" speed {}", i.speed) });
        }
    }
    if let Some(v) = i.voice_vlan.filter(|v| *v != 0) {
        o.push(format!(" switchport voice vlan {}", v));
    }
    if let Some(ch) = &i.ch_group {
        o.push(format!(" channel-group {} mode {}", ch.n, ch.mode));
    }
    if let Some(h) = &i.hsrp {
        if let Some(ip) = h.ip.as_ref().filter(|s| !s.is_empty()) {
            o.push(format!(" standby {} ip {}", h.grp, ip));
            if h.pri != 100 {
                o.push(format!(" standby {} priority {}", h.grp, h.pri));
            }
            if h.preempt {
                o.push(format!(" standby {} preempt", h.grp));
            }
        }
    }
    for v in i.x.values() {
        o.push(format!(" {}", v));
    }
    if i.shutdown {
        o.push(" shutdown".into());
    }
    o
}

fn running_config_text(d: &Device, c: &IosConfig) -> String {
    let router = d.kind == Kind::Router;
    let mut o: Vec<String> = vec![];
    let mut p = |s: String| o.push(s);
    p("!".into());
    p(format!("version {}", if router { "15.2" } else { "12.2" }));
    p("no service pad".into());
    p("service timestamps debug datetime msec".into());
    p(if c.timestamps_msec { "service timestamps log datetime msec" } else { "service timestamps log uptime" }.into());
    p(if c.pw_encrypt { "service password-encryption" } else { "no service password-encryption" }.into());
    p("!".into());
    p(format!("hostname {}", c.hostname));
    p("!".into());
    p("boot-start-marker".into());
    p("boot-end-marker".into());
    p("!".into());
    if c.logging.buffered {
        p(format!("logging buffered {}{}", c.logging.size, c.logging.level.as_ref().filter(|s| !s.is_empty()).map(|l| format!(" {}", l)).unwrap_or_default()));
    } else {
        p("no logging buffered".into());
    }
    if let Some(s) = c.enable_secret.as_ref().filter(|s| !s.is_empty()) {
        p(format!("enable secret 9 {}", s));
    }
    if let Some(pw) = c.enable_password.as_ref().filter(|s| !s.is_empty()) {
        p(format!("enable password {}", if c.pw_encrypt { format!("7 {}", fake_type7(pw)) } else { pw.clone() }));
    }
    p("!".into());
    for (u, us) in &c.users {
        let tail = if us.pw_type.as_deref() == Some("password") {
            format!(" password {}", if c.pw_encrypt { format!("7 {}", fake_type7(&us.plain)) } else { format!("0 {}", us.plain) })
        } else {
            format!(" secret 9 {}", us.secret)
        };
        p(format!("username {} privilege {}{}", u, us.priv_, tail));
    }
    p("no aaa new-model".into());
    if c.min_pw_len != 0 {
        p(format!("security passwords min-length {}", c.min_pw_len));
    }
    if let Some(v) = &c.vtp {
        if !v.domain.is_empty() || v.mode != "server" {
            if !v.domain.is_empty() {
                p(format!("vtp domain {}", v.domain));
            }
            if v.mode != "server" {
                p(format!("vtp mode {}", v.mode));
            }
        }
    }
    if !router {
        p("system mtu routing 1500".into());
    }
    if !router && c.ip_routing {
        p("ip routing".into());
    }
    if router {
        p("!".into());
        for x in &c.dhcp_excluded {
            let second = if !x.1.is_empty() && x.1 != x.0 { format!(" {}", x.1) } else { String::new() };
            p(format!("ip dhcp excluded-address {}{}", x.0, second));
        }
        p("!".into());
        for (name, pl) in &c.dhcp_pools {
            p(format!("ip dhcp pool {}", name));
            if let Some(net) = pl.network.as_ref().filter(|s| !s.is_empty()) {
                p(format!(" network {} {}", net, pl.mask.clone().unwrap_or_default()));
            }
            if !pl.default_router.is_empty() {
                p(format!(" default-router {}", pl.default_router.join(" ")));
            }
            if !pl.dns.is_empty() {
                p(format!(" dns-server {}", pl.dns.join(" ")));
            }
            match pl.lease {
                None => p(" lease infinite".into()),
                Some(l) if l != 1.0 => {
                    if l == l.floor() {
                        p(format!(" lease {}", num(l)));
                    } else {
                        let mins = (l * 1440.0).round() as i64;
                        let mut s = format!(" lease {} {}", mins / 1440, (mins % 1440) / 60);
                        if mins % 60 != 0 {
                            s += &format!(" {}", mins % 60);
                        }
                        p(s);
                    }
                }
                _ => {}
            }
            if let Some(o150) = pl.opt150.as_ref().filter(|s| !s.is_empty()) {
                p(format!(" option 150 ip {}", o150));
            }
            p("!".into());
        }
    }
    p("!".into());
    if !c.domain_lookup {
        p("no ip domain lookup".into());
    }
    if let Some(dn) = c.domain_name.as_ref().filter(|s| !s.is_empty()) {
        p(format!("{}{}", if router { "ip domain name " } else { "ip domain-name " }, dn));
    }
    for (h, ip) in &c.hosts {
        p(format!("ip host {} {}", h, ip));
    }
    if !c.name_servers.is_empty() {
        p(format!("ip name-server {}", c.name_servers.join(" ")));
    }
    if router {
        p("ip cef".into());
    }
    p("no ipv6 cef".into());
    p("!".into());
    if let Some(t) = nz(c.ssh.timeout) {
        p(format!("ip ssh time-out {}", t));
    }
    if let Some(r) = c.ssh.retries {
        p(format!("ip ssh authentication-retries {}", r));
    }
    if let Some(v) = nz(c.ssh.version) {
        p(format!("ip ssh version {}", v));
    }
    if c.scp_server {
        p("ip scp server enable".into());
    }
    p("!".into());
    p("spanning-tree mode pvst".into());
    p("spanning-tree extend system-id".into());
    if !router {
        let mut pr: std::collections::BTreeMap<u32, Vec<u16>> = Default::default();
        for (v, prio) in &c.stp_priority {
            pr.entry(*prio).or_default().push(*v);
        }
        for (prio, mut vs) in pr {
            vs.sort();
            p(format!("spanning-tree vlan {} priority {}", U::vlan_list_str(&vs), prio));
        }
        if !c.stp_off.is_empty() {
            let mut v = c.stp_off.clone();
            v.sort();
            p(format!("no spanning-tree vlan {}", U::vlan_list_str(&v)));
        }
        p("!".into());
        p("vlan internal allocation policy ascending".into());
        p("!".into());
        for (v, name) in &c.vlans {
            if *v == 1 {
                continue;
            }
            p(format!("vlan {}", v));
            p(format!(" name {}", name));
            match c.vlan_state.get(v).map(|s| s.as_str()) {
                Some("suspend") => p(" state suspend".into()),
                Some("shutdown") => p(" shutdown".into()),
                _ => {}
            }
            p("!".into());
        }
    }
    p("!".into());
    for l in crypto_config(c) {
        p(l);
    }
    for n in iface_names(d) {
        if !c.ifaces.contains_key(&n) {
            continue;
        }
        for l in iface_block(d, &n, c) {
            p(l);
        }
        p("!".into());
    }
    for l in router_sections(c) {
        p(l);
    }
    if !router {
        if let Some(gw) = c.default_gateway.as_ref().filter(|s| !s.is_empty()) {
            p(format!("ip default-gateway {}", gw));
        }
    }
    p("ip forward-protocol nd".into());
    p("!".into());
    p("no ip http server".into());
    p("no ip http secure-server".into());
    if c.dns_server {
        p("ip dns server".into());
    }
    for n in &c.nat.dynamic {
        p(format!("ip nat inside source list {} interface {}{}", n.acl, n.iface, if n.overload { " overload" } else { "" }));
    }
    for n in &c.nat.statics {
        p(format!("ip nat inside source static {} {}", n.local, n.global));
    }
    for r in &c.routes {
        let via = r.nh.clone().filter(|s| !s.is_empty()).or_else(|| r.iface.clone()).unwrap_or_default();
        let ad = match r.ad {
            Some(a) if a != 0 && a != 1 => format!(" {}", a),
            _ => String::new(),
        };
        p(format!("ip route {} {} {}{}", r.net, r.mask, via, ad));
    }
    p("!".into());
    for (name, a) in &c.acls {
        if is_digits(name) {
            continue;
        }
        p(format!("ip access-list {} {}", a.type_str(), name));
        for r in &a.rules {
            p(format!(" {}", ace_text(a.extended, r)));
        }
        p("!".into());
    }
    for h in &c.logging.hosts {
        p(format!("logging host {}", h));
    }
    if c.cdp_off {
        p("no cdp run".into());
    }
    if c.lldp {
        p("lldp run".into());
    }
    for v in c.x.values() {
        p(v.clone());
    }
    if let Some(sn) = &c.snmp {
        for (k, v) in &sn.communities {
            p(format!("snmp-server community {} {}", k, v));
        }
        if let Some(l) = sn.location.as_ref().filter(|s| !s.is_empty()) {
            p(format!("snmp-server location {}", l));
        }
        if let Some(l) = sn.contact.as_ref().filter(|s| !s.is_empty()) {
            p(format!("snmp-server contact {}", l));
        }
    }
    for (name, a) in &c.acls {
        if !is_digits(name) {
            continue;
        }
        for r in &a.rules {
            p(format!("access-list {} {}", name, ace_text(a.extended, r)));
        }
    }
    p("!".into());
    for k in ["exec", "login", "motd"] {
        if let Some(b) = c.banners.get(k) {
            p(format!("banner {} ^C{}^C", k, b.text));
        }
    }
    p("!".into());
    p("line con 0".into());
    for l in line_cfg(&c.lines.con, true, c.pw_encrypt) {
        p(l);
    }
    if router {
        p("line aux 0".into());
    }
    p("line vty 0 4".into());
    for l in line_cfg(&c.lines.vty0_4, false, c.pw_encrypt) {
        p(l);
    }
    p("line vty 5 15".into());
    for l in line_cfg(&c.lines.vty5_15, false, c.pw_encrypt) {
        p(l);
    }
    p("!".into());
    if let Some(s) = c.ntp_source.as_ref().filter(|s| !s.is_empty()) {
        p(format!("ntp source {}", s));
    }
    for n in &c.ntp_servers {
        p(format!("ntp server {}{}", n, if c.ntp_prefer.as_deref() == Some(n.as_str()) { " prefer" } else { "" }));
    }
    p("end".into());
    let body = o.join("\n");
    format!("Building configuration...\n\nCurrent configuration : {} bytes\n{}", js_len(&body) + 40, body)
}

pub fn running_config(st: &mut State, dev: &str) -> String {
    let c = st.dev(dev).cfg().clone();
    running_config_of(st, dev, &c)
}
/// Running-config-text för en given konfiguration (runningConfig(state, d, c))
pub fn running_config_of(st: &mut State, dev: &str, c: &IosConfig) -> String {
    running_config_text(st.dev(dev), c)
}
pub fn startup_config(st: &mut State, dev: &str) -> String {
    let d = st.dev(dev);
    let sc = match &d.startup {
        Some(s) => s,
        None => return "startup-config is not present".into(),
    };
    let full = running_config_text(d, sc);
    let mut lines: Vec<&str> = full.split('\n').collect();
    let head = format!("Using {} out of 524288 bytes", js_len(&full));
    let mut out = vec![head.as_str()];
    out.extend(lines.drain(3..));
    out.join("\n")
}
pub fn running_interface(st: &mut State, dev: &str, n: &str) -> String {
    let d = st.dev(dev);
    let mut v = vec!["!".to_string()];
    v.extend(iface_block(d, n, d.cfg()));
    v.push("end".into());
    let body = v.join("\n");
    format!("Building configuration...\n\nCurrent configuration : {} bytes\n{}", js_len(&body), body)
}

// ---------------------------------------------------------------- Crypto (kapitel 10)

fn enc_text(e: &str) -> Option<&'static str> {
    Some(match e {
        "des" => "DES - Data Encryption Standard (56 bit keys).",
        "3des" => "Three key triple DES",
        "aes" => "AES - Advanced Encryption Standard (128 bit keys).",
        "aes 192" => "AES - Advanced Encryption Standard (192 bit keys).",
        "aes 256" => "AES - Advanced Encryption Standard (256 bit keys).",
        _ => return None,
    })
}
fn hash_text(h: &str) -> Option<&'static str> {
    Some(match h {
        "sha" => "Secure Hash Standard",
        "sha256" => "Secure Hash Standard 2 (256 bit)",
        "sha384" => "Secure Hash Standard 2 (384 bit)",
        "md5" => "Message Digest 5",
        _ => return None,
    })
}
fn group_bits(g: i64) -> i64 {
    match g {
        1 => 768,
        2 => 1024,
        5 => 1536,
        14 => 2048,
        15 => 3072,
        16 => 4096,
        19 => 256,
        20 => 384,
        24 => 2048,
        _ => 1024,
    }
}

fn crypto_config(c: &IosConfig) -> Vec<String> {
    let cr = match &c.crypto {
        Some(cr) => cr,
        None => return vec![],
    };
    let mut o: Vec<String> = vec![];
    for (n, p) in &cr.policies {
        o.push(format!("crypto isakmp policy {}", n));
        if p.enc != "des" {
            o.push(format!(" encr {}", p.enc));
        }
        if p.hash != "sha" {
            o.push(format!(" hash {}", p.hash));
        }
        if p.auth != "rsa-sig" {
            o.push(format!(" authentication {}", p.auth));
        }
        if p.group != 1 {
            o.push(format!(" group {}", p.group));
        }
        if p.lifetime != 86400 {
            o.push(format!(" lifetime {}", p.lifetime));
        }
    }
    for (ip, k) in &cr.keys {
        o.push(format!("crypto isakmp key {} address {}", k, ip));
    }
    o.push("!".into());
    o.push("!".into());
    for (n, t) in &cr.transform_sets {
        o.push(format!("crypto ipsec transform-set {} {}", n, t.transforms.join(" ")));
        o.push(format!(" mode {}", if t.mode.is_empty() { "tunnel" } else { t.mode.as_str() }));
    }
    o.push("!".into());
    o.push("!".into());
    o.push("!".into());
    for (m, entries) in &cr.maps {
        for (seq, e) in entries {
            o.push(format!("crypto map {} {} ipsec-isakmp", m, seq));
            if let Some(p) = e.peer.as_ref().filter(|s| !s.is_empty()) {
                o.push(format!(" set peer {}", p));
            }
            if let Some(t) = e.ts.as_ref().filter(|s| !s.is_empty()) {
                o.push(format!(" set transform-set {}", t));
            }
            if let Some(a) = e.acl.as_ref().filter(|s| !s.is_empty()) {
                o.push(format!(" match address {}", a));
            }
        }
    }
    o.push("!".into());
    o
}

/// router ospf och router bgp (kurs 2)
fn router_sections(c: &IosConfig) -> Vec<String> {
    let mut o = vec![];
    if let Some(q) = &c.ospf {
        o.push(format!("router ospf {}", q.pid));
        if let Some(r) = q.router_id.as_ref().filter(|s| !s.is_empty()) {
            o.push(format!(" router-id {}", r));
        }
        if !q.log_adj {
            o.push(" no log-adjacency-changes".into());
        }
        if q.ref_bw != 0 && q.ref_bw != 100 {
            o.push(format!(" auto-cost reference-bandwidth {}", q.ref_bw));
        }
        if q.passive_default {
            o.push(" passive-interface default".into());
            for n in &q.no_passive {
                o.push(format!(" no passive-interface {}", n));
            }
        } else {
            for n in &q.passive {
                o.push(format!(" passive-interface {}", n));
            }
        }
        for x in &q.networks {
            o.push(format!(" network {} {} area {}", x.net, x.wild, x.area));
        }
        if q.default_originate {
            o.push(format!(" default-information originate{}", if q.default_always { " always" } else { "" }));
        }
        o.push("!".into());
    }
    if let Some(b) = &c.bgp {
        if b.as_ != 0 {
            o.push(format!("router bgp {}", b.as_));
            if let Some(r) = b.router_id.as_ref().filter(|s| !s.is_empty()) {
                o.push(format!(" bgp router-id {}", r));
            }
            if b.log_changes {
                o.push(" bgp log-neighbor-changes".into());
            }
            for (ip, nb) in &b.neighbors {
                if let Some(a) = nz(nb.remote_as) {
                    o.push(format!(" neighbor {} remote-as {}", ip, a));
                }
                if let Some(ds) = nb.description.as_ref().filter(|s| !s.is_empty()) {
                    o.push(format!(" neighbor {} description {}", ip, ds));
                }
                if nb.shutdown {
                    o.push(format!(" neighbor {} shutdown", ip));
                }
            }
            for n in &b.networks {
                o.push(format!(" network {} mask {}", n.net, n.mask));
            }
            o.push("!".into());
        }
    }
    o
}

fn wild_to_mask(w: Option<&str>) -> String {
    U::int_to_ip(!U::ipn(w.filter(|s| !s.is_empty()).unwrap_or("0.0.0.0")))
}
fn ident_of(spec: &Option<AddrSpec>) -> String {
    let spec = match spec {
        Some(s) if !s.any => s,
        _ => return "0.0.0.0/0.0.0.0".into(),
    };
    if let Some(h) = spec.host.as_ref().filter(|s| !s.is_empty()) {
        return format!("{}/255.255.255.255", h);
    }
    let n = sim::spec_norm(Some(spec));
    format!("{}/{}", n.split('/').next().unwrap_or(""), wild_to_mask(spec.wild.as_deref()))
}

pub fn crypto_isakmp_sa(st: &mut State, dev: &str) -> String {
    let s = sim::crypto_status(st, dev);
    let mut o = vec![
        "IPv4 Crypto ISAKMP SA".to_string(),
        "dst             src             state          conn-id status".to_string(),
    ];
    for x in &s.isakmp {
        o.push(format!("{}{}{}{} {}", pad(&x.dst, 16), pad(&x.src, 16), pad(&x.state, 15), pad_l(x.conn_id, 7), x.status));
    }
    o.push(String::new());
    o.push("IPv6 Crypto ISAKMP SA".into());
    o.push(String::new());
    o.join("\n")
}

pub fn crypto_ipsec_sa(st: &mut State, dev: &str) -> String {
    let s = sim::crypto_status(st, dev);
    if s.sas.is_empty() {
        return String::new();
    }
    let mut o: Vec<String> = vec![];
    let mut last_if: Option<&str> = None;
    for x in &s.sas {
        if last_if != Some(x.iface.as_str()) {
            o.push(String::new());
            o.push(format!("interface: {}", x.iface));
            o.push(format!("    Crypto map tag: {}, local addr {}", x.map, x.local));
            last_if = Some(x.iface.as_str());
        }
        let (enc, dec) = if x.up { (num(x.encaps), num(x.decaps)) } else { ("0".to_string(), "0".to_string()) };
        let spec_hex = format!("0x{:X}({})", x.spi, x.spi);
        let lines: Vec<String> = vec![
            String::new(),
            "   protected vrf: (none)".into(),
            format!("   local  ident (addr/mask/prot/port): ({}/0/0)", ident_of(&x.rule.src)),
            format!("   remote ident (addr/mask/prot/port): ({}/0/0)", ident_of(&x.rule.dst)),
            format!("   current_peer {} port 500", x.peer),
            "     PERMIT, flags={origin_is_acl,}".into(),
            format!("    #pkts encaps: {}, #pkts encrypt: {}, #pkts digest: {}", enc, enc, enc),
            format!("    #pkts decaps: {}, #pkts decrypt: {}, #pkts verify: {}", dec, dec, dec),
            "    #pkts compressed: 0, #pkts decompressed: 0".into(),
            "    #pkts not compressed: 0, #pkts compr. failed: 0".into(),
            "    #pkts not decompressed: 0, #pkts decompress failed: 0".into(),
            "    #send errors 0, #recv errors 0".into(),
            String::new(),
            format!("     local crypto endpt.: {}, remote crypto endpt.: {}", x.local, x.peer),
            format!("     plaintext mtu {}, path mtu 1500, ip mtu 1500, ip mtu idb {}", x.mtu, x.iface),
            format!("     current outbound spi: {}", if x.up { spec_hex.clone() } else { "0x0(0)".to_string() }),
            "     PFS (Y/N): N, DH group: none".into(),
            String::new(),
            "     inbound esp sas:".into(),
        ];
        o.extend(lines);
        let sa_tail = |spi: u32| -> Vec<String> {
            vec![
                format!("      spi: 0x{:X}({})", spi, spi),
                "        transform: esp-256-aes esp-sha256-hmac ,".into(),
                "        in use settings ={Tunnel, }".into(),
                "        sa timing: remaining key lifetime (k/sec): (4607998/3284)".into(),
                "        IV size: 16 bytes".into(),
                "        replay detection support: Y".into(),
                "        Status: ACTIVE(ACTIVE)".into(),
            ]
        };
        if x.up {
            o.extend(sa_tail(x.spi ^ 0x5a5a5a5a));
        }
        for l in ["", "     inbound ah sas:", "", "     inbound pcp sas:", "", "     outbound esp sas:"] {
            o.push(l.to_string());
        }
        if x.up {
            o.extend(sa_tail(x.spi));
        }
        for l in ["", "     outbound ah sas:", "", "     outbound pcp sas:"] {
            o.push(l.to_string());
        }
    }
    o.join("\n")
}

pub fn crypto_map_show(st: &mut State, dev: &str) -> String {
    let d = st.dev(dev);
    let c = d.cfg();
    let cr = match &c.crypto {
        Some(cr) if !cr.maps.is_empty() => cr,
        _ => return String::new(),
    };
    let mut o: Vec<String> = vec![];
    for (m, entries) in &cr.maps {
        let ifs: Vec<&String> = c.ifaces.iter().filter(|(_, i)| i.crypto_map.as_deref() == Some(m.as_str())).map(|(n, _)| n).collect();
        for (seq, e) in entries {
            let peer = e.peer.clone().unwrap_or_default();
            o.push(format!("Crypto Map IPv4 \"{}\" {} ipsec-isakmp", m, seq));
            o.push(format!("\tPeer = {}", peer));
            let acl = e.acl.as_ref().and_then(|a| c.acls.get(a));
            if let (Some(acl), Some(an)) = (acl, e.acl.as_ref()) {
                o.push(format!("\tExtended IP access list {}", an));
                for r in &acl.rules {
                    if r.remark.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                        o.push(format!("\t    access-list {} {}", an, collapse_ws(&ace_text(acl.extended, r))));
                    }
                }
            } else {
                o.push("\tNo matching address list set.".into());
            }
            o.push(format!("\tCurrent peer: {}", peer));
            o.push("\tSecurity association lifetime: 4608000 kilobytes/3600 seconds".into());
            o.push("\tResponder-Only (Y/N): N".into());
            o.push("\tPFS (Y/N): N".into());
            o.push("\tMixed-mode : Disabled".into());
            o.push("\tTransform sets={ ".into());
            if let Some(tsn) = &e.ts {
                if let Some(t) = cr.transform_sets.get(tsn) {
                    o.push(format!("\t\t{}:  {{ {}  }} , ", tsn, t.transforms.join(" ")));
                }
            }
            o.push("\t}".into());
            let empty = |x: &Option<String>| x.as_ref().map(|s| s.is_empty()).unwrap_or(true);
            if empty(&e.peer) || empty(&e.ts) || empty(&e.acl) {
                o.push("\tWARNING: This crypto map is incomplete!".into());
                o.push("\t  To remedy the situation add a peer, a transform-set and an access list.".into());
            }
        }
        o.push(format!("\tInterfaces using crypto map {}:", m));
        for n in ifs {
            o.push(format!("\t\t{}", n));
        }
        o.push(String::new());
    }
    o.join("\n")
}

pub fn crypto_isakmp_policy(st: &mut State, dev: &str) -> String {
    let d = st.dev(dev);
    let mut o: Vec<String> = vec![String::new(), "Global IKE policy".into()];
    if let Some(cr) = &d.cfg().crypto {
        for (n, p) in &cr.policies {
            o.push(format!("Protection suite of priority {}", n));
            o.push(format!("\tencryption algorithm:\t{}", enc_text(&p.enc).unwrap_or(&p.enc)));
            o.push(format!("\thash algorithm:\t\t{}", hash_text(&p.hash).unwrap_or(&p.hash)));
            o.push(format!(
                "\tauthentication method:\t{}",
                if p.auth == "pre-share" { "Pre-Shared Key" } else { "Rivest-Shamir-Adleman Signature" }
            ));
            o.push(format!("\tDiffie-Hellman group:\t#{} ({} bit)", p.group, group_bits(p.group)));
            o.push(format!("\tlifetime:\t\t{} seconds, no volume limit", p.lifetime));
        }
    }
    for l in [
        "Default protection suite",
        "\tencryption algorithm:\tDES - Data Encryption Standard (56 bit keys).",
        "\thash algorithm:\t\tSecure Hash Standard",
        "\tauthentication method:\tRivest-Shamir-Adleman Signature",
        "\tDiffie-Hellman group:\t#1 (768 bit)",
        "\tlifetime:\t\t86400 seconds, no volume limit",
    ] {
        o.push(l.to_string());
    }
    o.join("\n")
}

pub fn crypto_session(st: &mut State, dev: &str) -> String {
    let s = sim::crypto_status(st, dev);
    let mut o: Vec<String> = vec!["Crypto session current status".into(), String::new()];
    for x in &s.isakmp {
        let up = s.sas.iter().any(|a| a.peer == x.dst && a.up);
        let qm = x.state == "QM_IDLE";
        o.push(format!("Interface: {}", s.sas.first().map(|a| a.iface.as_str()).unwrap_or("")));
        o.push(format!("Session status: {}", if qm { if up { "UP-ACTIVE" } else { "UP-IDLE" } } else { "DOWN-NEGOTIATING" }));
        o.push(format!("Peer: {} port 500 ", x.dst));
        o.push(format!("  IKEv1 SA: local {}/500 remote {}/500 {}", x.src, x.dst, if qm { "Active" } else { "Inactive" }));
        for a in s.sas.iter().filter(|a| a.peer == x.dst) {
            o.push(format!("  IPSEC FLOW: permit ip {} {}", ident_of(&a.rule.src), ident_of(&a.rule.dst)));
            o.push(format!("        Active SAs: {}, origin: crypto map", if a.up { 2 } else { 0 }));
        }
        o.push(String::new());
    }
    o.join("\n")
}

// ---------------------------------------------------------------- Interface-utskrifter

pub fn interfaces_status(st: &mut State, dev: &str, filter: Option<&str>) -> String {
    let dd = get_d(st);
    let d = st.dev(dev);
    let mut o = vec![String::new(), "Port      Name               Status       Vlan       Duplex  Speed Type".to_string()];
    for n in phys_ports(d) {
        if let Some(f) = filter {
            if f != n {
                continue;
            }
        }
        let i = d.ifc(&n).unwrap();
        let p = dd.ports.get(&sim::key(&d.id, &n));
        let ps = port_state_i(st, &dd, d, &n);
        let vlan = if (sim::op_mode(st, &d.id, &n) == "trunk" && ps == "connected") || i.mode == "trunk" {
            "trunk".to_string()
        } else {
            i.access_vlan.to_string()
        };
        let (dup, spd) = match p.and_then(|p| p.neg.as_ref()) {
            Some(neg) if ps == "connected" => {
                let a = if neg.auto { "a-" } else { "" };
                (format!("{}{}", a, neg.duplex), format!("{}{}", a, neg.speed))
            }
            _ => (i.duplex.clone(), i.speed.clone()),
        };
        let ty = if i.sfp {
            if p.is_some() { "1000BaseSX SFP" } else { "Not Present" }
        } else {
            "10/100/1000BaseTX"
        };
        o.push(format!(
            "{}{}{}{}{} {} {}",
            pad(U::short_if(&n), 10),
            pad(slice_chars(&i.description, 0, 18), 19),
            pad(&ps, 13),
            pad(&vlan, 11),
            pad_l(&dup, 6),
            pad_l(&spd, 6),
            ty
        ));
    }
    o.join("\n")
}

/// (status, protocol) som show ip interface brief visar
fn brief_status(st: &State, dd: &Derived, d: &Device, n: &str) -> (String, String) {
    let i = match d.ifc(n) {
        Some(i) => i,
        None => return ("down".into(), "down".into()),
    };
    let svi = i.svi.filter(|v| *v != 0);
    let down = || ("down".to_string(), "down".to_string());
    let admin = || ("administratively down".to_string(), "down".to_string());
    let upup = || ("up".to_string(), "up".to_string());
    if svi.is_some() || i.parent.is_some() || i.internal || i.is_loop || d.kind == Kind::Router {
        let ep = dd.eps.get(&format!("E:{}:{}", d.id, n));
        if i.shutdown {
            admin()
        } else if i.is_loop {
            upup()
        } else if let Some(par) = &i.parent {
            let parent = d.ifc(par);
            let pp = dd.ports.get(&sim::key(&d.id, par));
            if parent.map(|p| p.shutdown).unwrap_or(false) {
                admin()
            } else if pp.map(|p| p.up).unwrap_or(false) {
                upup()
            } else {
                down()
            }
        } else if let Some(v) = svi {
            let upv = ep.map(|e| e.up).unwrap_or(false);
            if !sim::vlan_exists(d, v) {
                down()
            } else {
                ("up".to_string(), if upv { "up" } else { "down" }.to_string())
            }
        } else {
            let p = dd.ports.get(&sim::key(&d.id, n));
            if p.map(|p| p.up).unwrap_or(false) { upup() } else { down() }
        }
    } else {
        match port_state_i(st, dd, d, n).as_str() {
            "disabled" => admin(),
            "connected" => upup(),
            _ => down(),
        }
    }
}

fn brief_line(st: &State, dd: &Derived, d: &Device, n: &str) -> String {
    let i = d.ifc(n).unwrap();
    let (s, pr) = brief_status(st, dd, d, n);
    let ip = i.ip.as_ref().map(|i| i.addr.clone()).unwrap_or_else(|| "unassigned".into());
    let method = if i.ip.is_some() || d.kind == Kind::Router { "NVRAM" } else { "unset" };
    format!("{}{}YES {}{}{}", pad(n, 27), pad(ip, 16), pad(method, 7), pad(s, 22), pr)
}

pub fn ip_int_brief(st: &mut State, dev: &str) -> String {
    let dd = get_d(st);
    let d = st.dev(dev);
    let mut o = vec!["Interface                  IP-Address      OK? Method Status                Protocol".to_string()];
    for n in iface_names(d) {
        o.push(brief_line(st, &dd, d, &n));
    }
    o.join("\n")
}

fn fmt_ago(s: f64) -> String {
    let s = s.max(0.0).floor() as i64;
    format!("{}:{}:{}", two(s / 3600), two(s % 3600 / 60), two(s % 60))
}

fn port_mac(d: &Device, n: &str) -> String {
    let h = format!("0{:x}", sim::port_num(n) + 1);
    let tail: String = h.chars().rev().take(2).collect::<Vec<_>>().into_iter().rev().collect();
    format!("{}{}", slice_chars(&d.base_mac, 0, 10), tail)
}

fn pfx_txt(ip: &crate::net::model::IpAddr) -> String {
    format!("{}/{}", ip.addr, prefix(&ip.mask))
}

pub fn show_interface(st: &mut State, dev: &str, n: &str) -> String {
    let dd = get_d(st);
    let load = {
        let is_phys = st.dev(dev).ifc(n).map(|i| !i.is_loop && i.parent.is_none() && i.svi.is_none()).unwrap_or(false);
        if is_phys { sim::link_load(st, dev, n) } else { 0.0 }
    };
    let d = st.dev(dev);
    let i = match d.ifc(n) {
        Some(i) => i,
        None => return String::new(),
    };
    if i.is_loop {
        let lst = if i.shutdown { "administratively down" } else { "up" };
        let mut s = format!("{} is {}, line protocol is {} \n  Hardware is Loopback\n", n, lst, if i.shutdown { "down" } else { "up" });
        if !i.description.is_empty() {
            s += &format!("  Description: {}\n", i.description);
        }
        if let Some(ip) = &i.ip {
            s += &format!("  Internet address is {}\n", pfx_txt(ip));
        }
        s += "  MTU 1514 bytes, BW 8000000 Kbit/sec, DLY 5000 usec, \n     reliability 255/255, txload 1/255, rxload 1/255\n  Encapsulation LOOPBACK, loopback not set\n  Keepalive set (10 sec)\n  Last input never, output never, output hang never\n  Last clearing of \"show interface\" counters never\n  Input queue: 0/75/0/0 (size/max/drops/flushes); Total output drops: 0\n  Queueing strategy: fifo\n  Output queue: 0/0 (size/max)\n  5 minute input rate 0 bits/sec, 0 packets/sec\n  5 minute output rate 0 bits/sec, 0 packets/sec\n     0 packets input, 0 bytes, 0 no buffer\n     0 packets output, 0 bytes, 0 underruns";
        return s;
    }
    let p = dd.ports.get(&sim::key(&d.id, n));
    let mac = U::mac_dots(&d.base_mac);
    if let Some(par_name) = &i.parent {
        let pp = dd.ports.get(&sim::key(&d.id, par_name));
        let par_shut = d.ifc(par_name).map(|p| p.shutdown).unwrap_or(false);
        let up = if i.shutdown || par_shut {
            "administratively down"
        } else if pp.map(|p| p.up).unwrap_or(false) {
            "up"
        } else {
            "down"
        };
        let lp = if up == "up" { "up" } else { "down" };
        let mut v = vec![
            format!("{} is {}, line protocol is {}", n, up, lp),
            format!("  Hardware is iGbE, address is {} (bia {})", mac, mac),
        ];
        if !i.description.is_empty() {
            v.push(format!("  Description: {}", i.description));
        }
        if let Some(ip) = &i.ip {
            v.push(format!("  Internet address is {}", pfx_txt(ip)));
        }
        v.push("  MTU 1500 bytes, BW 1000000 Kbit/sec, DLY 10 usec,".into());
        v.push("     reliability 255/255, txload 1/255, rxload 1/255".into());
        v.push(format!(
            "  Encapsulation 802.1Q Virtual LAN, Vlan ID  {}.",
            i.dot1q.filter(|v| *v != 0).map(|v| v.to_string()).unwrap_or_else(|| "unset".into())
        ));
        v.push("  ARP type: ARPA, ARP Timeout 04:00:00".into());
        v.push("  Keepalive set (10 sec)".into());
        v.push("  Last clearing of \"show interface\" counters never".into());
        return v.join("\n");
    }
    if i.svi.filter(|v| *v != 0).is_some() {
        let ep = dd.eps.get(&format!("E:{}:{}", d.id, n));
        let up = if i.shutdown { "administratively down" } else { "up" };
        let lp = if ep.map(|e| e.up).unwrap_or(false) { "up" } else { "down" };
        let mut v = vec![
            format!("{} is {}, line protocol is {}", n, up, lp),
            format!("  Hardware is EtherSVI, address is {} (bia {})", mac, mac),
        ];
        if let Some(ip) = &i.ip {
            v.push(format!("  Internet address is {}", pfx_txt(ip)));
        }
        v.push("  MTU 1500 bytes, BW 1000000 Kbit/sec, DLY 10 usec,".into());
        return v.join("\n");
    }
    let ps = port_state_i(st, &dd, d, n);
    let sw = is_sw(d);
    let (up, lp, extra) = match ps.as_str() {
        "disabled" => ("administratively down", "down", " (disabled)"),
        "err-disabled" => ("down", "down", " (err-disabled)"),
        "connected" => ("up", "up", if sw { " (connected)" } else { "" }),
        _ => ("down", "down", if sw { " (notconnect)" } else { "" }),
    };
    let c = d.rt.counters.get(n).cloned().unwrap_or_default();
    let neg = p.filter(|p| p.up).and_then(|p| p.neg.as_ref());
    let dup_txt = match neg {
        Some(ng) => if ng.duplex == "full" { "Full-duplex" } else { "Half-duplex" },
        None => match i.duplex.as_str() {
            "half" => "Half-duplex",
            "full" => "Full-duplex",
            _ => "Auto-duplex",
        },
    };
    let spd_txt = match neg {
        Some(ng) => format!("{}Mb/s", ng.speed),
        None => if i.speed == "auto" { "Auto-speed".to_string() } else { format!("{}Mb/s", i.speed) },
    };
    let bw: i64 = match neg {
        Some(ng) => ng.speed as i64 * 1000,
        None => 1000000,
    };
    let in_rate = (load * 1000000.0 * 0.45).round() as i64;
    let out_rate = (load * 1000000.0 * 0.4).round() as i64;
    let crc = fl(c.crc);
    let runts = fl(c.runts);
    let late = fl(c.late);
    let coll = fl(c.coll);
    let in_err = crc + runts;
    let pm = U::mac_dots(&port_mac(d, n));
    let lp_up = lp == "up";
    let mut o = vec![
        format!("{} is {}, line protocol is {}{}", n, up, lp, extra),
        format!("  Hardware is Gigabit Ethernet, address is {} (bia {})", pm, pm),
    ];
    if !i.description.is_empty() {
        o.push(format!("  Description: {}", i.description));
    }
    if let Some(ip) = &i.ip {
        o.push(format!("  Internet address is {}", pfx_txt(ip)));
    }
    o.push(format!("  MTU 1500 bytes, BW {} Kbit/sec, DLY 10 usec,", bw));
    o.push(format!("     reliability {}/255, txload 1/255, rxload 1/255", if in_err > 0 { 250 } else { 255 }));
    o.push("  Encapsulation ARPA, loopback not set".into());
    o.push("  Keepalive set (10 sec)".into());
    o.push(format!("  {}, {}, media type is {}", dup_txt, spd_txt, if i.sfp { "1000BaseSX SFP" } else { "10/100/1000BaseTX" }));
    o.push("  input flow-control is off, output flow-control is unsupported".into());
    o.push("  ARP type: ARPA, ARP Timeout 04:00:00".into());
    o.push("  Last input 00:00:00, output 00:00:00, output hang never".into());
    o.push(format!(
        "  Last clearing of \"show interface\" counters {}",
        match c.cleared {
            Some(t) => fmt_ago(st.time - t),
            None => "never".to_string(),
        }
    ));
    o.push("  Input queue: 0/75/0/0 (size/max/drops/flushes); Total output drops: 0".into());
    o.push("  Queueing strategy: fifo".into());
    o.push("  Output queue: 0/40 (size/max)".into());
    o.push(format!("  5 minute input rate {} bits/sec, {} packets/sec", if lp_up { in_rate } else { 0 }, if lp_up { (in_rate as f64 / 8000.0).round() as i64 } else { 0 }));
    o.push(format!("  5 minute output rate {} bits/sec, {} packets/sec", if lp_up { out_rate } else { 0 }, if lp_up { (out_rate as f64 / 8000.0).round() as i64 } else { 0 }));
    o.push(format!("     {} packets input, {} bytes, 0 no buffer", fl(c.inp), fl(c.inp * 412.0)));
    o.push(format!("     Received {} broadcasts ({} multicasts)", fl(c.inp * 0.03), fl(c.inp * 0.02)));
    o.push(format!("     {} runts, 0 giants, 0 throttles", runts));
    o.push(format!("     {} input errors, {} CRC, 0 frame, 0 overrun, 0 ignored", in_err, crc));
    o.push(format!("     0 watchdog, {} multicast, 0 pause input", fl(c.inp * 0.02)));
    o.push("     0 input packets with dribble condition detected".into());
    o.push(format!("     {} packets output, {} bytes, 0 underruns", fl(c.outp), fl(c.outp * 398.0)));
    o.push(format!("     0 output errors, {} collisions, {} interface resets", coll, num(1.0 + c.flaps)));
    o.push("     0 unknown protocol drops".into());
    o.push(format!("     0 babbles, {} late collision, {} deferred", late, coll / 3));
    o.push("     0 lost carrier, 0 no carrier, 0 pause output".into());
    o.push("     0 output buffer failures, 0 output buffers swapped out".into());
    o.join("\n")
}

pub fn switchport(st: &mut State, dev: &str, n: &str) -> String {
    let dd = get_d(st);
    let d = st.dev(dev);
    let i = match d.ifc(n) {
        Some(i) => i,
        None => return String::new(),
    };
    let c = d.cfg();
    let op = sim::op_mode(st, dev, n);
    let ps = port_state_i(st, &dd, d, n);
    let adm = match i.mode.as_str() {
        "access" => "static access".to_string(),
        m => m.to_string(),
    };
    let oper = if ps != "connected" && ps != "inactive" { "down" } else if op == "trunk" { "trunk" } else { "static access" };
    let vname = |v: u16| c.vlans.get(&v).cloned().unwrap_or_else(|| "Inactive".into());
    vec![
        format!("Name: {}", U::short_if(n)),
        "Switchport: Enabled".to_string(),
        format!("Administrative Mode: {}", adm),
        format!("Operational Mode: {}", oper),
        format!("Administrative Trunking Encapsulation: {}", if i.encap.as_deref() == Some("dot1q") { "dot1q" } else { "negotiate" }),
        format!("Operational Trunking Encapsulation: {}", if op == "trunk" { "dot1q" } else { "native" }),
        format!("Negotiation of Trunking: {}", if i.nonegotiate || i.mode == "access" { "Off" } else { "On" }),
        format!("Access Mode VLAN: {} ({})", i.access_vlan, vname(i.access_vlan)),
        format!("Trunking Native Mode VLAN: {} ({})", i.native, vname(i.native)),
        "Administrative Native VLAN tagging: enabled".to_string(),
        "Voice VLAN: none".to_string(),
        "Administrative private-vlan host-association: none".to_string(),
        "Operational private-vlan: none".to_string(),
        format!("Trunking VLANs Enabled: {}", if i.allowed_all() { "ALL".to_string() } else { U::vlan_list_str(&i.allowed_list()) }),
        "Pruning VLANs Enabled: 2-1001".to_string(),
        "Capture Mode Disabled".to_string(),
        "Capture VLANs Allowed: ALL".to_string(),
        String::new(),
        "Protected: false".to_string(),
        "Unknown unicast blocked: disabled".to_string(),
        "Unknown multicast blocked: disabled".to_string(),
        "Appliance trust: none".to_string(),
    ]
    .join("\n")
}

pub fn vlan_brief(st: &mut State, dev: &str) -> String {
    let d = st.dev(dev);
    let c = d.cfg();
    let mut o = vec![
        String::new(),
        "VLAN Name                             Status    Ports".to_string(),
        "---- -------------------------------- --------- -------------------------------".to_string(),
    ];
    let phys = phys_ports(d);
    for (v, name) in &c.vlans {
        let ports: Vec<String> = phys
            .iter()
            .filter(|n| {
                let i = d.ifc(n).unwrap();
                if sim::op_mode(st, &d.id, n) == "trunk" || i.mode == "trunk" {
                    return false;
                }
                i.access_vlan == *v
            })
            .map(|n| U::short_if(n))
            .collect();
        let mut lines: Vec<String> = ports.chunks(4).map(|ch| ch.join(", ")).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        let status = match c.vlan_state.get(v).map(|s| s.as_str()) {
            Some("suspend") => "suspended",
            Some("shutdown") => "act/lshut",
            _ => "active",
        };
        o.push(format!("{}{}{}{}", pad(v, 5), pad(name, 33), pad(status, 10), lines[0]));
        for l in &lines[1..] {
            o.push(format!("{}{}", pad("", 48), l));
        }
    }
    o.push("1002 fddi-default                     act/unsup ".into());
    o.push("1003 token-ring-default               act/unsup ".into());
    o.push("1004 fddinet-default                  act/unsup ".into());
    o.push("1005 trnet-default                    act/unsup ".into());
    // Portar vars VLAN saknas
    for n in &phys {
        let i = d.ifc(n).unwrap();
        if sim::op_mode(st, &d.id, n) != "trunk" && !c.vlans.contains_key(&i.access_vlan) {
            o.push(format!("% Port {} is assigned to VLAN {}, which does not exist", U::short_if(n), i.access_vlan));
        }
    }
    o.join("\n")
}

pub fn interfaces_trunk(st: &mut State, dev: &str) -> String {
    let dd = get_d(st);
    let d = st.dev(dev);
    let trunks: Vec<String> = phys_ports(d)
        .into_iter()
        .filter(|n| sim::op_mode(st, &d.id, n) == "trunk" && port_state_i(st, &dd, d, n) == "connected")
        .collect();
    if trunks.is_empty() {
        return String::new();
    }
    let mut o = vec![String::new(), "Port        Mode             Encapsulation  Status        Native vlan".to_string()];
    for n in &trunks {
        let i = d.ifc(n).unwrap();
        let mode = match i.mode.as_str() {
            "trunk" => "on",
            "dynamic desirable" => "desirable",
            _ => "auto",
        };
        o.push(format!("{}{}{}{}{}", pad(U::short_if(n), 12), pad(mode, 17), pad("802.1q", 15), pad("trunking", 14), i.native));
    }
    o.push(String::new());
    o.push("Port        Vlans allowed on trunk".into());
    for n in &trunks {
        let i = d.ifc(n).unwrap();
        o.push(format!("{}{}", pad(U::short_if(n), 12), if i.allowed_all() { "1-4094".to_string() } else { U::vlan_list_str(&i.allowed_list()) }));
    }
    o.push(String::new());
    o.push("Port        Vlans allowed and active in management domain".into());
    for n in &trunks {
        o.push(format!("{}{}", pad(U::short_if(n), 12), U::vlan_list_str(&sim::trunk_vlans(d, d.ifc(n).unwrap()))));
    }
    o.push(String::new());
    o.push("Port        Vlans in spanning tree forwarding state and not pruned".into());
    for n in &trunks {
        let list: Vec<u16> = sim::trunk_vlans(d, d.ifc(n).unwrap())
            .into_iter()
            .filter(|v| !dd.blocked.contains_key(&format!("{}#{}", sim::key(&d.id, n), v)))
            .collect();
        o.push(format!("{}{}", pad(U::short_if(n), 12), U::vlan_list_str(&list)));
    }
    o.join("\n")
}

struct MacRow {
    vlan: u16,
    mac: String,
    port: String,
}

pub fn mac_table(st: &mut State, dev: &str, filt: MacFilter) -> String {
    let dd = get_d(st);
    let d = st.dev(dev);
    let no_filter = filt.address.is_none() && filt.iface.is_none() && filt.vlan.is_none();
    let mut rows: Vec<MacRow> = vec![];
    // Vilka ändpunkter kan nås genom varje port, per VLAN
    for n in phys_ports(d) {
        let p = match dd.ports.get(&sim::key(&d.id, &n)) {
            Some(p) if p.up => p,
            _ => continue,
        };
        let i = d.ifc(&n).unwrap();
        let vlans: Vec<u16> = if sim::op_mode(st, &d.id, &n) == "trunk" { sim::trunk_vlans(d, i) } else { vec![i.access_vlan] };
        for v in vlans {
            if dd.blocked.contains_key(&format!("{}#{}", sim::key(&d.id, &n), v)) {
                continue;
            }
            let node = format!("S:{}:{}", d.id, v);
            // Följ kanten via just denna port
            if let Some(edges) = dd.adj.get(&node) {
                for e in edges {
                    if e.link.as_deref() != Some(p.link.as_str()) {
                        continue;
                    }
                    let mut seen: std::collections::HashSet<String> = Default::default();
                    seen.insert(node.clone());
                    let mut q: std::collections::VecDeque<String> = Default::default();
                    q.push_back(e.to.clone());
                    seen.insert(e.to.clone());
                    while let Some(x) = q.pop_front() {
                        if x.starts_with("E:") && x.matches(':').count() >= 2 {
                            if let Some(ep) = dd.eps.get(&x) {
                                if ep.up && !rows.iter().any(|r| r.mac == ep.mac && r.vlan == v) {
                                    rows.push(MacRow { vlan: v, mac: ep.mac.clone(), port: n.clone() });
                                }
                            }
                        }
                        if let Some(fs) = dd.adj.get(&x) {
                            for f in fs {
                                if seen.insert(f.to.clone()) {
                                    q.push_back(f.to.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(a) = filt.address.as_ref().filter(|s| !s.is_empty()) {
        let al = a.to_lowercase();
        let plain: String = a.chars().filter(|c| !matches!(c, '.' | ':' | '-')).collect::<String>().to_lowercase();
        rows.retain(|r| U::mac_dots(&r.mac).starts_with(&al) || r.mac == plain);
    }
    if let Some(i) = filt.iface.as_ref().filter(|s| !s.is_empty()) {
        rows.retain(|r| &r.port == i);
    }
    if let Some(v) = filt.vlan.filter(|v| *v != 0) {
        rows.retain(|r| r.vlan == v);
    }
    rows.sort_by(|a, b| a.vlan.cmp(&b.vlan).then(sim::port_num(&a.port).cmp(&sim::port_num(&b.port))));
    let mut o = vec![
        "          Mac Address Table".to_string(),
        "-------------------------------------------".to_string(),
        String::new(),
        "Vlan    Mac Address       Type        Ports".to_string(),
        "----    -----------       --------    -----".to_string(),
    ];
    if no_filter {
        o.push(" All    0100.0ccc.cccc    STATIC      CPU".into());
        o.push(" All    0180.c200.0000    STATIC      CPU".into());
        o.push(" All    ffff.ffff.ffff    STATIC      CPU".into());
    }
    for r in &rows {
        o.push(format!("{}    {}    DYNAMIC     {}", pad_l(r.vlan, 4), U::mac_dots(&r.mac), U::short_if(&r.port)));
    }
    o.push(format!("Total Mac Addresses for this criterion: {}", rows.len() + if no_filter { 3 } else { 0 }));
    o.join("\n")
}

pub fn spanning_tree(st: &mut State, dev: &str, only_vlan: Option<u16>) -> String {
    let dd = get_d(st);
    let d = st.dev(dev);
    let c = d.cfg();
    let only = only_vlan.filter(|v| *v != 0);
    let mut o: Vec<String> = vec![];
    let mut vl: Vec<u16> = c.vlans.keys().copied().collect();
    if let Some(ov) = only {
        vl.retain(|v| *v == ov);
        if vl.is_empty() {
            return format!("Spanning tree instance(s) for vlan {} does not exist.", ov);
        }
    }
    let phys = phys_ports(d);
    for v in vl {
        let ports: Vec<&String> = phys
            .iter()
            .filter(|n| {
                let p = match dd.ports.get(&sim::key(&d.id, n)) {
                    Some(p) if p.up => p,
                    _ => return false,
                };
                let i = d.ifc(n).unwrap();
                if sim::op_mode(st, &d.id, n) == "trunk" {
                    return sim::trunk_vlans(d, i).contains(&v);
                }
                i.access_vlan == v
            })
            .collect();
        if ports.is_empty() {
            continue;
        }
        o.push(String::new());
        o.push(vlan4(v));
        if c.stp_off.contains(&v) {
            o.push(format!("  Spanning tree instance for vlan {} is disabled", v));
            continue;
        }
        let info = dd.stp.get(&d.id).and_then(|m| m.get(&v));
        let root_dev = info.and_then(|i| st.devices.get(&i.root)).unwrap_or(d);
        let rb = sim::bridge_id(root_dev, v);
        let mb = sim::bridge_id(d, v);
        o.push("  Spanning tree enabled protocol ieee".into());
        o.push(format!("  Root ID    Priority    {}", rb.pr));
        o.push(format!("             Address     {}", U::mac_dots(&root_dev.base_mac)));
        match info {
            Some(inf) if !inf.is_root => {
                let rp = inf.root_port.clone().unwrap_or_default();
                o.push("             Cost        4".into());
                o.push(format!("             Port        {} ({})", sim::port_num(&rp), rp));
            }
            _ => o.push("             This bridge is the root".into()),
        }
        o.push("             Hello Time   2 sec  Max Age 20 sec  Forward Delay 15 sec".into());
        o.push(String::new());
        o.push(format!("  Bridge ID  Priority    {}  (priority {} sys-id-ext {})", mb.pr, mb.pr - v as u32, v));
        o.push(format!("             Address     {}", U::mac_dots(&d.base_mac)));
        o.push("             Hello Time   2 sec  Max Age 20 sec  Forward Delay 15 sec".into());
        o.push("             Aging Time  300 sec".into());
        o.push(String::new());
        o.push("Interface           Role Sts Cost      Prio.Nbr Type".into());
        o.push("------------------- ---- --- --------- -------- --------------------------------".into());
        for n in ports {
            let k = format!("{}#{}", sim::key(&d.id, n), v);
            let (mut role, mut sts, mut ty) = ("Desg", "FWD", "P2p");
            let i = d.ifc(n).unwrap();
            match dd.blocked.get(&k).map(|s| s.as_str()) {
                Some("BLK") => {
                    role = "Altn";
                    sts = "BLK";
                }
                Some("BKN*") => {
                    role = "Desg";
                    sts = "BKN*";
                    ty = "P2p *PVID_Inc";
                }
                _ => {
                    if let Some(inf) = info {
                        if !inf.is_root && inf.root_port.as_deref() == Some(n.as_str()) {
                            role = "Root";
                        }
                    }
                }
            }
            if i.portfast && sim::op_mode(st, &d.id, n) != "trunk" {
                ty = "P2p Edge";
            }
            let p = dd.ports.get(&sim::key(&d.id, n));
            let cost = if p.and_then(|p| p.neg.as_ref()).map(|n| n.speed == 100).unwrap_or(false) { 19 } else { 4 };
            o.push(format!("{}{}{}{}{}{}", pad(U::short_if(n), 20), pad(role, 5), pad(sts, 4), pad(cost, 10), pad(format!("128.{}", sim::port_num(n)), 9), ty));
        }
    }
    o.join("\n")
}

pub fn power_inline(st: &mut State, dev: &str, only: Option<&str>) -> String {
    let dd = get_d(st);
    let d = st.dev(dev);
    let info = match dd.poe.get(&d.id) {
        Some(i) if d.poe_budget != 0.0 => i,
        _ => return "% This switch does not support inline power".into(),
    };
    let mut o = vec![
        String::new(),
        format!("Available:{:.1}(w)  Used:{:.1}(w)  Remaining:{:.1}(w)", info.available, info.used, info.remaining),
        String::new(),
        "Interface Admin  Oper       Power   Device              Class Max".to_string(),
        "                           (Watts)                            ".to_string(),
        "--------- ------ ---------- ------- ------------------- ----- ----".to_string(),
    ];
    for (n, p) in &info.ports {
        if let Some(on) = only {
            if on != n {
                continue;
            }
        }
        o.push(format!(
            "{}{}{}{} {}{}15.4",
            pad(U::short_if(n), 10),
            pad(&p.admin, 7),
            pad(&p.oper, 11),
            pad_l(format!("{:.1}", p.watts), 7),
            pad(&p.device, 20),
            pad(&p.cls, 6)
        ));
    }
    o.join("\n")
}

pub fn port_security(st: &mut State, dev: &str, only: Option<&str>) -> String {
    let dd = get_d(st);
    let d = st.dev(dev);
    if let Some(only) = only {
        let i = d.ifc(only);
        let ps = match i.and_then(|i| i.port_sec.as_ref()) {
            Some(ps) => ps,
            None => return "Port Security              : Disabled\nPort Status                : Secure-down\nViolation Mode             : Shutdown".into(),
        };
        let i = i.unwrap();
        let empty: Vec<String> = vec![];
        let learned = d.rt.sec_learned.get(only).unwrap_or(&empty);
        let ed = d.rt.errdisabled.contains_key(only);
        let status = if ed {
            "Secure-shutdown"
        } else if port_state_i(st, &dd, d, only) == "connected" {
            "Secure-up"
        } else {
            "Secure-down"
        };
        return vec![
            "Port Security              : Enabled".to_string(),
            format!("Port Status                : {}", status),
            format!(
                "Violation Mode             : {}",
                match ps.violation.as_str() {
                    "shutdown" => "Shutdown",
                    "restrict" => "Restrict",
                    _ => "Protect",
                }
            ),
            "Aging Time                 : 0 mins".to_string(),
            "Aging Type                 : Absolute".to_string(),
            "SecureStatic Address Aging : Disabled".to_string(),
            format!("Maximum MAC Addresses      : {}", ps.max),
            format!("Total MAC Addresses        : {}", learned.len().max(ps.macs.len())),
            "Configured MAC Addresses   : 0".to_string(),
            format!("Sticky MAC Addresses       : {}", if ps.sticky { ps.macs.len() } else { 0 }),
            format!(
                "Last Source Address:Vlan   : {}",
                match learned.last() {
                    Some(m) => format!("{}:{}", U::mac_dots(m), i.access_vlan),
                    None => "0000.0000.0000:0".to_string(),
                }
            ),
            format!("Security Violation Count   : {}", d.rt.sec_violations),
        ]
        .join("\n");
    }
    let mut o = vec![
        "Secure Port  MaxSecureAddr  CurrentAddr  SecurityViolation  Security Action".to_string(),
        "                (Count)       (Count)          (Count)".to_string(),
        "---------------------------------------------------------------------------".to_string(),
    ];
    for n in phys_ports(d) {
        let i = d.ifc(&n).unwrap();
        let ps = match &i.port_sec {
            Some(p) => p,
            None => continue,
        };
        let learned = d.rt.sec_learned.get(&n).map(|v| v.len()).unwrap_or(0);
        o.push(format!(
            "{}{}{}{}         {}",
            pad_l(U::short_if(&n), 11),
            pad_l(ps.max, 15),
            pad_l((learned as u32).min(ps.max), 13),
            pad_l(d.rt.sec_violations, 19),
            if ps.violation == "shutdown" { "Shutdown" } else { "Restrict" }
        ));
    }
    o.push("---------------------------------------------------------------------------".into());
    o.join("\n")
}

pub fn cdp_neighbors(st: &mut State, dev: &str) -> String {
    let dd = get_d(st);
    let d = st.dev(dev);
    let mut o = vec![
        "Capability Codes: R - Router, T - Trans Bridge, B - Source Route Bridge".to_string(),
        "                  S - Switch, H - Host, I - IGMP, r - Repeater, P - Phone,".to_string(),
        "                  D - Remote, C - CVTA, M - Two-port Mac Relay".to_string(),
        String::new(),
        "Device ID        Local Intrfce     Holdtme    Capability  Platform  Port ID".to_string(),
    ];
    let mut count = 0;
    for n in d.cfg().ifaces.keys() {
        let p = match dd.ports.get(&sim::key(&d.id, n)) {
            Some(p) if p.up => p,
            _ => continue,
        };
        let peer = match st.devices.get(&p.peer.dev) {
            Some(p) => p,
            None => continue,
        };
        let (name, cap, plat, pid) = if peer.os == Os::Ios {
            (
                format!("{}.nordvik.example", peer.cfg().hostname),
                if peer.kind == Kind::Router { "R B S I" } else { "S I" },
                peer.model.replacen("CISCO", "", 1).replacen("/K9", "", 1),
                U::short_if(&p.peer.port),
            )
        } else if peer.kind == Kind::Ap {
            (peer.id.clone(), "T B I", "AIR-CAP37".to_string(), "Gig 0".to_string())
        } else {
            continue;
        };
        o.push(format!(
            "{}{}{}{}{}{}",
            pad(slice_chars(&name, 0, 16), 17),
            pad(U::short_if(n).replacen("Gi", "Gig ", 1), 18),
            pad("165", 11),
            pad(cap, 12),
            pad(slice_chars(&plat, 0, 9), 10),
            pid.replacen("Gi", "Gig ", 1)
        ));
        count += 1;
    }
    o.push(String::new());
    o.push(format!("Total cdp entries displayed : {}", count));
    o.join("\n")
}

// ---------------------------------------------------------------- Routing

fn dyn_route<'a>(dd: &'a Derived, dev: &str, net: &str, mask: &str) -> Option<&'a dynroute::DynRoute> {
    dd.dynr.routes.get(dev).and_then(|v| v.iter().find(|r| r.net == net && r.mask == mask))
}

/// Hur länge en OSPF- eller BGP-rutt har funnits (sedan grannskapet eller sessionen kom upp)
fn route_age_i(st: &State, dd: &Derived, dev: &str, kind: &str, nh: &str) -> String {
    let t0 = if kind == "B" {
        dynroute::since(st, &format!("b|{}|{}", dev, nh))
    } else {
        let nb = dd.dynr.ospf.get(dev).and_then(|r| r.nbrs.iter().find(|n| n.ip == nh));
        match nb {
            Some(nb) => dynroute::since(st, &format!("{}|{}@{}", dev, nb.rid, nb.iface)),
            None => 0.0,
        }
    };
    let sec = (st.time - t0).floor().max(0.0) as i64;
    format!("{}:{}:{}", two(sec / 3600 % 24), two(sec / 60 % 60), two(sec % 60))
}

/// Hur länge sedan rutten lärdes, t.ex. "00:12:31" (routeAge).
/// `net` är rutten: antingen nätadressen (slås upp i dynamiska rutter) eller nästa hopp.
pub fn route_age(st: &mut State, dev: &str, kind: &str, net: &str) -> String {
    let dd = get_d(st);
    let st = &*st;
    let nh = dd
        .dynr
        .routes
        .get(dev)
        .and_then(|v| v.iter().find(|r| r.net == net && (kind.is_empty() || r.kind == kind || (kind == "O" && r.kind.starts_with('O')))))
        .map(|r| r.nh.clone())
        .unwrap_or_else(|| net.to_string());
    route_age_i(st, &dd, dev, kind, &nh)
}

pub fn ip_route(st: &mut State, dev: &str, filt: Option<&str>) -> String {
    let dd = get_d(st);
    let st = &*st;
    let d = st.dev(dev);
    let routes = sim::installed_routes(st, &dd, dev);
    if let Some(f) = filt.filter(|f| U::is_ip(f)) {
        let r = match sim::route_lookup(st, &dd, dev, f) {
            Some(r) => r,
            None => return "% Network not in table".into(),
        };
        let nh = r.nh.clone().unwrap_or_default();
        let mut lines = vec![format!("Routing entry for {}/{}{}", r.net, prefix(&r.mask), if r.net == "0.0.0.0" { ", supernet" } else { "" })];
        let dr = dyn_route(&dd, dev, &r.net, &r.mask);
        match r.kind.as_str() {
            "C" => {
                lines.push("  Known via \"connected\", distance 0, metric 0 (connected, via interface)".into());
                lines.push("  Routing Descriptor Blocks:".into());
                lines.push(format!("  * directly connected, via {}", r.iface));
            }
            "O" | "O*E2" => {
                let pid = d.cfg().ospf.as_ref().map(|o| o.pid).filter(|p| *p != 0).unwrap_or(1);
                let ty = if r.kind == "O" {
                    "intra area".to_string()
                } else {
                    format!("extern 2, forward metric {}", dr.and_then(|x| x.fwd).unwrap_or(0))
                };
                lines.push(format!(
                    "  Known via \"ospf {}\", distance 110, metric {}{}, type {}",
                    pid,
                    r.metric,
                    if r.kind == "O*E2" { ", candidate default path" } else { "" },
                    ty
                ));
                let age = route_age_i(st, &dd, dev, &r.kind, &nh);
                lines.push(format!("  Last update from {} on {}, {} ago", nh, r.iface, age));
                lines.push("  Routing Descriptor Blocks:".into());
                let from = dr
                    .and_then(|x| x.origin.as_ref())
                    .and_then(|o| dd.dynr.ospf.get(o))
                    .and_then(|o| o.rid.clone())
                    .unwrap_or_else(|| nh.clone());
                lines.push(format!("  * {}, from {}, {} ago, via {}", nh, from, age, r.iface));
                lines.push(format!("      Route metric is {}, traffic share count is 1", r.metric));
            }
            "B" => {
                let age = route_age_i(st, &dd, dev, "B", &nh);
                let path: Vec<i64> = dr.map(|x| x.path.clone()).unwrap_or_default();
                let p0 = path.first().map(|p| p.to_string()).unwrap_or_else(|| "0".into());
                lines.push(format!("  Known via \"bgp {}\", distance 20, metric 0", d.cfg().bgp.as_ref().map(|b| b.as_).unwrap_or(0)));
                lines.push(format!("  Tag {}, type external", p0));
                lines.push(format!("  Last update from {} {} ago", nh, age));
                lines.push("  Routing Descriptor Blocks:".into());
                lines.push(format!("  * {}, from {}, {} ago", nh, nh, age));
                lines.push("      Route metric is 0, traffic share count is 1".into());
                lines.push(format!("      AS Hops {}", path.len()));
                lines.push(format!("      Route tag {}", p0));
            }
            _ => {
                lines.push(format!("  Known via \"static\", distance 1, metric 0{}", if r.net == "0.0.0.0" { ", candidate default path" } else { "" }));
                lines.push("  Routing Descriptor Blocks:".into());
                lines.push(format!("  * {}", r.nh.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("directly connected, via {}", r.iface))));
                lines.push("      Route metric is 0, traffic share count is 1".into());
            }
        }
        return lines.join("\n");
    }
    let all = routes.clone();
    let mut routes = routes;
    match filt {
        Some("static") => routes.retain(|r| r.kind.starts_with('S')),
        Some("connected") => routes.retain(|r| r.kind == "C" || r.kind == "L"),
        Some("ospf") => routes.retain(|r| r.kind.starts_with('O')),
        Some("bgp") => routes.retain(|r| r.kind == "B"),
        _ => {}
    }
    let def = all.iter().find(|r| r.net == "0.0.0.0" && r.mask == "0.0.0.0");
    let mut o: Vec<String> = [
        "Codes: L - local, C - connected, S - static, R - RIP, M - mobile, B - BGP",
        "       D - EIGRP, EX - EIGRP external, O - OSPF, IA - OSPF inter area",
        "       N1 - OSPF NSSA external type 1, N2 - OSPF NSSA external type 2",
        "       E1 - OSPF external type 1, E2 - OSPF external type 2",
        "       i - IS-IS, su - IS-IS summary, L1 - IS-IS level-1, L2 - IS-IS level-2",
        "       ia - IS-IS inter area, * - candidate default, U - per-user static route",
        "       o - ODR, P - periodic downloaded static route, H - NHRP, l - LISP",
        "       + - replicated route, % - next hop override",
        "",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    o.push(format!(
        "Gateway of last resort is {}",
        match def {
            Some(r) => format!("{} to network 0.0.0.0", r.nh.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "0.0.0.0".into())),
            None => "not set".into(),
        }
    ));
    o.push(String::new());
    // Gruppera per klassfullt nät för utskriftens rubriker
    let mut groups: Vec<(String, Vec<sim::InstalledRoute>)> = vec![];
    for r in routes {
        let parts: Vec<&str> = r.net.split('.').collect();
        let first: u32 = parts[0].parse().unwrap_or(0);
        let cls = if r.net == "0.0.0.0" {
            "0.0.0.0/0".to_string()
        } else if first < 128 {
            format!("{}.0.0.0/8", parts[0])
        } else if first < 192 {
            format!("{}.0.0/16", parts[..2].join("."))
        } else {
            format!("{}.0/24", parts[..3].join("."))
        };
        match groups.iter_mut().find(|g| g.0 == cls) {
            Some(g) => g.1.push(r),
            None => groups.push((cls, vec![r])),
        }
    }
    groups.sort_by_key(|g| U::ipn(g.0.split('/').next().unwrap_or("0.0.0.0")));
    for (g, mut rs) in groups {
        let age = |r: &sim::InstalledRoute| route_age_i(st, &dd, dev, &r.kind, r.nh.as_deref().unwrap_or(""));
        let iface = |r: &sim::InstalledRoute| r.iface.clone().unwrap_or_default();
        if g == "0.0.0.0/0" {
            for r in &rs {
                let nh = r.nh.clone().unwrap_or_default();
                if r.kind == "O*E2" {
                    o.push(format!("O*E2  0.0.0.0/0 [110/{}] via {}, {}, {}", r.metric, nh, age(r), iface(r)));
                } else if r.kind == "B" {
                    o.push(format!("B*    0.0.0.0/0 [20/0] via {}, {}", nh, age(r)));
                } else if !nh.is_empty() {
                    o.push(format!("S*    0.0.0.0/0 [{}/0] via {}", if r.ad != 0 { r.ad } else { 1 }, nh));
                } else {
                    o.push(format!("S*    0.0.0.0/0 is directly connected, {}", iface(r)));
                }
            }
            continue;
        }
        let mut masks: Vec<&str> = vec![];
        for r in &rs {
            if !masks.contains(&r.mask.as_str()) {
                masks.push(&r.mask);
            }
        }
        let gp: u32 = g.split('/').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        if rs.len() > 1 || prefix(&rs[0].mask) != gp {
            o.push(format!("      {} is variably subnetted, {} subnets, {} masks", g, rs.len(), masks.len()));
        }
        rs.sort_by(|a, b| {
            U::ipn(&a.net).cmp(&U::ipn(&b.net)).then_with(|| match (a.kind == "C", b.kind == "C") {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            })
        });
        for r in &rs {
            let pfx = format!("{}/{}", r.net, prefix(&r.mask));
            let nh = r.nh.clone().unwrap_or_default();
            match r.kind.as_str() {
                "C" => o.push(format!("C        {} is directly connected, {}", pfx, iface(r))),
                "L" => o.push(format!("L        {} is directly connected, {}", pfx, iface(r))),
                "O" => o.push(format!("O        {} [110/{}] via {}, {}, {}", pfx, r.metric, nh, age(r), iface(r))),
                "B" => o.push(format!("B        {} [20/0] via {}, {}", pfx, nh, age(r))),
                _ => {
                    if !nh.is_empty() {
                        o.push(format!("S        {} [{}/0] via {}", pfx, if r.ad != 0 { r.ad } else { 1 }, nh));
                    } else {
                        o.push(format!("S        {} is directly connected, {}", pfx, iface(r)));
                    }
                }
            }
        }
    }
    o.join("\n")
}

pub fn dhcp_binding(st: &mut State, dev: &str) -> String {
    let d = st.dev(dev);
    let mut o = vec![
        "Bindings from all pools not associated with VRF:".to_string(),
        "IP address          Client-ID/              Lease expiration        Type       State      Interface".to_string(),
        "                    Hardware address/".to_string(),
        "                    User name".to_string(),
    ];
    let mut ips: Vec<&String> = d.rt.dhcp_bindings.keys().collect();
    ips.sort_by_key(|ip| U::ipn(ip));
    for ip in ips {
        let e = &d.rt.dhcp_bindings[ip];
        if e.conflict {
            continue;
        }
        let mac = e.mac.as_ref().filter(|m| !m.is_empty()).map(|m| format!("01{}", m)).unwrap_or_default();
        let chars: Vec<char> = mac.chars().collect();
        let cid = chars.chunks(4).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join(".");
        o.push(format!("{}{}{}{}{}{}", pad(ip, 20), pad(cid, 24), pad("Sep 30 2026 08:00 AM", 24), pad("Automatic", 11), pad("Active", 11), sub_if_for(d, ip)));
    }
    o.join("\n")
}
fn sub_if_for(d: &Device, ip: &str) -> String {
    let n = d.cfg().ifaces.iter().find(|(_, i)| i.ip.as_ref().map(|a| U::same_subnet(ip, &a.addr, &a.mask)).unwrap_or(false));
    match n {
        Some((n, _)) => U::short_if(n),
        None => "Unknown".into(),
    }
}

pub fn dhcp_pool(st: &mut State, dev: &str, only: Option<&str>) -> String {
    let d = st.dev(dev);
    let c = d.cfg();
    let mut o: Vec<String> = vec![];
    for (name, p) in &c.dhcp_pools {
        if let Some(on) = only {
            if on != name {
                continue;
            }
        }
        let network = match p.network.as_ref().filter(|s| !s.is_empty()) {
            Some(n) => n,
            None => {
                for l in [
                    "",
                    &format!("Pool {} :", name),
                    " Utilization mark (high/low)    : 100 / 0",
                    " Subnet size (first/next)       : 0 / 0",
                    " Total addresses                : 0",
                    " Leased addresses               : 0",
                ] {
                    o.push(l.to_string());
                }
                continue;
            }
        };
        let mask = p.mask.clone().unwrap_or_default();
        let total: i64 = (1i64 << (32 - prefix(&mask).min(32))) - 2;
        let leased = d.rt.dhcp_bindings.values().filter(|b| &b.pool == name && !b.conflict).count() as i64;
        let mut ex: i64 = 0;
        let net = U::ipn(network) as i64;
        let ranges: Vec<(i64, i64)> = c
            .dhcp_excluded
            .iter()
            .map(|x| (U::ipn(&x.0) as i64, U::ipn(if x.1.is_empty() { &x.0 } else { &x.1 }) as i64))
            .collect();
        let mut n = net + 1;
        while n < net + 1 + total {
            if ranges.iter().any(|(a, b)| n >= *a && n <= *b) {
                ex += 1;
            }
            n += 1;
        }
        o.push(String::new());
        o.push(format!("Pool {} :", name));
        o.push(" Utilization mark (high/low)    : 100 / 0".into());
        o.push(" Subnet size (first/next)       : 0 / 0 ".into());
        o.push(format!(" Total addresses                : {}", total));
        o.push(format!(" Leased addresses               : {}", leased));
        o.push(format!(" Excluded addresses             : {}", ex));
        o.push(" Pending event                  : none".into());
        o.push(" 1 subnet is currently in the pool :".into());
        o.push(" Current index        IP address range                    Leased/Excluded/Total".into());
        o.push(format!(
            " {}{}{}    / {}     / {}",
            pad(U::int_to_ip((net + 1 + (total - 1).min(leased + ex)) as u32), 21),
            pad(format!("{}     - {}", U::int_to_ip((net + 1) as u32), U::int_to_ip((net + total) as u32)), 36),
            leased,
            ex,
            total
        ));
    }
    o.join("\n")
}

pub fn nat_translations(st: &mut State, dev: &str) -> String {
    let d = st.dev(dev);
    let mut o = vec!["Pro Inside global         Inside local          Outside local         Outside global".to_string()];
    for s in &d.cfg().nat.statics {
        o.push(format!("{}{}{}{}---", pad("---", 4), pad(&s.global, 22), pad(&s.local, 22), pad("---", 22)));
    }
    for t in &d.rt.nat_trans {
        if t.from_static {
            continue;
        }
        o.push(format!(
            "{}{}{}{}{}:{}",
            pad(&t.proto, 4),
            pad(format!("{}:{}", t.global, t.port), 22),
            pad(format!("{}:{}", t.local, t.port), 22),
            pad(format!("{}:{}", t.dst, t.dport), 22),
            t.dst,
            t.dport
        ));
    }
    if o.len() == 1 {
        return String::new();
    }
    o.join("\n")
}

pub fn nat_statistics(st: &mut State, dev: &str) -> String {
    let d = st.dev(dev);
    let c = d.cfg();
    let mut ins = vec![];
    let mut outs = vec![];
    for n in iface_names(d) {
        match c.ifaces.get(&n).and_then(|i| i.nat_dir) {
            Some(NatDir::Inside) => ins.push(n),
            Some(NatDir::Outside) => outs.push(n),
            None => {}
        }
    }
    let dyn_n = d.rt.nat_trans.iter().filter(|t| !t.from_static).count();
    let ns = c.nat.statics.len();
    let mut o = vec![
        format!("Total active translations: {} ({} static, {} dynamic; {} extended)", dyn_n + ns, ns, dyn_n, dyn_n),
        format!("Peak translations: {}", dyn_n + ns + 3),
        "Outside interfaces:".to_string(),
        format!("  {}", outs.join(", ")),
        "Inside interfaces: ".to_string(),
        format!("  {}", ins.join(", ")),
        format!("Hits: {}  Misses: 0", dyn_n * 12),
        format!("CEF Translated packets: {}, CEF Punted packets: 0", dyn_n * 12),
        "Expired translations: 0".to_string(),
        "Dynamic mappings:".to_string(),
    ];
    for n in &c.nat.dynamic {
        o.push("-- Inside Source".into());
        o.push(format!("[Id: 1] access-list {} interface {} refcount {}", n.acl, n.iface, dyn_n));
    }
    o.push("Total doors: 0".into());
    o.push("Appl doors: 0".into());
    o.push("Normal doors: 0".into());
    o.push("Queued Packets: 0".into());
    o.join("\n")
}

pub fn access_lists(st: &mut State, dev: &str, only: Option<&str>, ip_only: bool) -> String {
    let d = st.dev(dev);
    let mut o: Vec<String> = vec![];
    for (name, a) in &d.cfg().acls {
        if let Some(on) = only {
            if on != name {
                continue;
            }
        }
        o.push(format!("{} IP access list {}", if a.extended { "Extended" } else { "Standard" }, name));
        let empty = std::collections::BTreeMap::new();
        let c = d.rt.acl_counters.get(name).unwrap_or(&empty);
        for r in &a.rules {
            if r.remark.as_ref().map(|s| !s.is_empty()).unwrap_or(false) {
                continue;
            }
            let mut txt = collapse_ws(&ace_text(a.extended, r).replacen("deny  ", "deny", 1));
            if !a.extended {
                if let Some(s) = &r.src {
                    if !s.any && s.host.as_ref().map(|h| h.is_empty()).unwrap_or(true) {
                        if let Some(w) = s.wild.as_ref().filter(|w| !w.is_empty() && *w != "0.0.0.0") {
                            txt = format!("{} {}, wildcard bits {}", r.action, s.ip.clone().unwrap_or_default(), w);
                        }
                    }
                }
            }
            let m = match c.get(&r.seq).copied().filter(|v| *v != 0.0) {
                Some(v) => format!(" ({} match{})", num(v), if v == 1.0 { "" } else { "es" }),
                None => String::new(),
            };
            o.push(format!("    {} {}{}", r.seq, txt, m));
        }
    }
    o.join("\n")
}

pub fn ip_interface(st: &mut State, dev: &str, n: &str) -> String {
    let dd = get_d(st);
    let st = &*st;
    let d = st.dev(dev);
    let i = match d.ifc(n) {
        Some(i) => i,
        None => return String::new(),
    };
    let (s, pr) = brief_status(st, &dd, d, n);
    let mut o = vec![format!("{} is {}, line protocol is {}", n, s, pr)];
    if let Some(ip) = &i.ip {
        o.push(format!("  Internet address is {}", pfx_txt(ip)));
        for l in [
            "  Broadcast address is 255.255.255.255",
            "  Address determined by non-volatile memory",
        ] {
            o.push(l.to_string());
        }
        o.push(format!("  MTU is {} bytes", nz(i.mtu).unwrap_or(1500)));
        o.push("  Helper address is not set".into());
        o.push("  Directed broadcast forwarding is disabled".into());
        o.push(format!("  Outgoing access list is {}", i.acl_out.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "not set".into())));
        o.push(format!("  Inbound  access list is {}", i.acl_in.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "not set".into())));
        for l in [
            "  Proxy ARP is enabled",
            "  Local Proxy ARP is disabled",
            "  Security level is default",
            "  Split horizon is enabled",
            "  ICMP redirects are always sent",
            "  ICMP unreachables are always sent",
            "  ICMP mask replies are never sent",
            "  IP fast switching is enabled",
            "  IP CEF switching is enabled",
        ] {
            o.push(l.to_string());
        }
        o.push(format!(
            "  Network address translation is {}",
            match i.nat_dir {
                Some(NatDir::Inside) => "enabled, interface in domain inside",
                Some(NatDir::Outside) => "enabled, interface in domain outside",
                None => "disabled",
            }
        ));
        o.push(format!("  TCP Adjust MSS is {}", nz(i.adjust_mss).map(|v| v.to_string()).unwrap_or_else(|| "disabled".into())));
        if let Some(cm) = i.crypto_map.as_ref().filter(|s| !s.is_empty()) {
            o.push(format!("  Crypto map {} is applied (IPsec, plaintext mtu {})", cm, sim::TUNNEL_MTU));
        }
    } else {
        o.push("  Internet protocol processing disabled".into());
    }
    o.join("\n")
}

pub fn arp_table(st: &mut State, dev: &str) -> String {
    let dd = get_d(st);
    let st = &*st;
    let d = st.dev(dev);
    let mut o = vec!["Protocol  Address          Age (min)  Hardware Addr   Type   Interface".to_string()];
    struct Row {
        ip: String,
        age: String,
        mac: String,
        iface: String,
    }
    let mut rows: Vec<Row> = vec![];
    for e in sim::dev_eps(&dd, &d.id) {
        if !e.up {
            continue;
        }
        let eip = match &e.ip {
            Some(ip) => ip.clone(),
            None => continue,
        };
        rows.push(Row { ip: eip.clone(), age: "-".into(), mac: e.mac.clone(), iface: e.iface.clone() });
        let seg = match dd.comp.get(&e.key) {
            Some(s) => *s,
            None => continue,
        };
        for x in dd.seg_eps(seg) {
            if x.dev == d.id || !x.up {
                continue;
            }
            let xip = match &x.ip {
                Some(ip) if !ip.is_empty() => ip,
                _ => continue,
            };
            if !U::same_subnet(xip, &eip, e.mask.as_deref().unwrap_or("")) {
                continue;
            }
            if let Some(dv) = st.devices.get(&x.dev) {
                if dv.kind == Kind::Switch && d.kind != Kind::Router {
                    continue;
                }
            }
            rows.push(Row { ip: xip.clone(), age: (1 + U::hash(xip) % 40).to_string(), mac: x.mac.clone(), iface: e.iface.clone() });
        }
    }
    rows.sort_by_key(|r| U::ipn(&r.ip));
    for r in rows {
        o.push(format!("Internet  {}{}{}ARPA   {}", pad(&r.ip, 17), pad(&r.age, 11), pad(U::mac_dots(&r.mac), 16), r.iface));
    }
    o.join("\n")
}

pub fn ip_ssh(st: &mut State, dev: &str) -> String {
    let d = st.dev(dev);
    let v = match sim::ssh_enabled(d) {
        Some(v) if !v.is_empty() => v,
        _ => return "SSH Disabled - version 1.99\n%Please create RSA keys to enable SSH (and of atleast 768 bits for SSH v2).\nAuthentication methods:publickey,keyboard-interactive,password\nAuthentication timeout: 120 secs; Authentication retries: 3".into(),
    };
    let c = d.cfg();
    format!(
        "SSH Enabled - version {}\nAuthentication methods:publickey,keyboard-interactive,password\nAuthentication Publickey Algorithms:x509v3-ssh-rsa,ssh-rsa\nHostkey Algorithms:x509v3-ssh-rsa,ssh-rsa\nEncryption Algorithms:aes128-ctr,aes192-ctr,aes256-ctr\nMAC Algorithms:hmac-sha2-256,hmac-sha2-512,hmac-sha1\nAuthentication timeout: {} secs; Authentication retries: {}\nMinimum expected Diffie Hellman key size : 2048 bits\nIOS Keys in SECSH format(ssh-rsa, base64 encoded): {}\nssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQC{}k9Xp2vT...",
        if v == "2" || v == "2.0" { "2.0" } else { "1.99" },
        nz(c.ssh.timeout).unwrap_or(120),
        c.ssh.retries.unwrap_or(3),
        c.hostname,
        d.base_mac
    )
}

pub fn show_version(st: &mut State, dev: &str) -> String {
    let stamp = if st.dev(dev).rt.reloads != 0 { sim::stamp(st, dev) } else { String::new() };
    let d = st.dev(dev);
    let c = d.cfg();
    let up = (st.time - d.rt.boot_time).floor().max(0.0) as i64;
    let dd = up / 86400;
    let hh = up % 86400 / 3600;
    let mm = up % 3600 / 60;
    let up_txt = format!(
        "{}{}{} minute{}",
        if dd != 0 { format!("{} day{}, ", dd, if dd > 1 { "s" } else { "" }) } else { String::new() },
        if hh != 0 { format!("{} hour{}, ", hh, if hh > 1 { "s" } else { "" }) } else { String::new() },
        mm,
        if mm != 1 { "s" } else { "" }
    );
    let reg = |def: &str| -> String {
        let cur = c.conf_reg.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| def.to_string());
        let mut s = format!("Configuration register is {}", cur);
        if let Some(nx) = c.conf_reg_next.as_ref().filter(|n| !n.is_empty() && **n != cur) {
            s += &format!(" (will be {} at next reload)", nx);
        }
        s
    };
    if d.kind == Kind::Router {
        return vec![
            "Cisco IOS Software, C2951 Software (C2951-UNIVERSALK9-M), Version 15.2(4)M11, RELEASE SOFTWARE (fc2)".to_string(),
            "Technical Support: http://www.cisco.com/techsupport".to_string(),
            "Copyright (c) 1986-2016 by Cisco Systems, Inc.".to_string(),
            String::new(),
            "ROM: System Bootstrap, Version 15.0(1r)M15, RELEASE SOFTWARE (fc1)".to_string(),
            String::new(),
            format!("{} uptime is {}", c.hostname, up_txt),
            format!("System returned to ROM by {}", if d.rt.reloads != 0 { format!("reload at {}", stamp) } else { "power-on".to_string() }),
            "System image file is \"flash0:c2951-universalk9-mz.SPA.152-4.M11.bin\"".to_string(),
            "Last reload type: Normal Reload".to_string(),
            String::new(),
            "Cisco CISCO2951/K9 (revision 1.0) with 491520K/32768K bytes of memory.".to_string(),
            "Processor board ID FTX1629AHB2".to_string(),
            "3 Gigabit Ethernet interfaces".to_string(),
            "1 terminal line".to_string(),
            "DRAM configuration is 72 bits wide with parity enabled.".to_string(),
            "255K bytes of non-volatile configuration memory.".to_string(),
            "255744K bytes of ATA System CompactFlash 0 (Read/Write)".to_string(),
            String::new(),
            reg("0x2102"),
        ]
        .join("\n");
    }
    let img = if d.model.contains("3750") { "C3750-IPSERVICESK9-M" } else { "C3560-IPSERVICESK9-M" };
    let img_head = img.split('-').next().unwrap_or("");
    vec![
        format!("Cisco IOS Software, {} Software ({}), Version 12.2(55)SE12, RELEASE SOFTWARE (fc2)", img_head, img),
        "Technical Support: http://www.cisco.com/techsupport".to_string(),
        "Copyright (c) 1986-2017 by Cisco Systems, Inc.".to_string(),
        String::new(),
        format!("ROM: Bootstrap program is {} boot loader", img_head),
        String::new(),
        format!("{} uptime is {}", c.hostname, up_txt),
        format!("System returned to ROM by {}", if d.rt.reloads != 0 { "reload" } else { "power-on" }),
        format!("System image file is \"flash:/{}.122-55.SE12.bin\"", img.to_lowercase()),
        String::new(),
        format!("cisco {} (PowerPC405) processor (revision F0) with 131072K bytes of memory.", d.model),
        "Processor board ID FOC1133Z0R4".to_string(),
        "28 Gigabit Ethernet interfaces".to_string(),
        "The password-recovery mechanism is enabled.".to_string(),
        String::new(),
        "512K bytes of flash-simulated non-volatile configuration memory.".to_string(),
        format!("Base ethernet MAC Address       : {}", U::mac_colon(&d.base_mac).to_uppercase()),
        format!("Model number                    : {}", d.model),
        String::new(),
        reg("0xF"),
    ]
    .join("\n")
}

pub fn show_clock(st: &mut State, dev: &str) -> String {
    let c = sim::device_clock(st, dev);
    format!("{}{}.{} {} {} {} {} {}", if c.synced { "" } else { "*" }, c.hms, c.msec, if c.synced { "CEST" } else { "UTC" }, c.dow, c.mon, c.day, c.year)
}
pub fn show_clock_detail(st: &mut State, dev: &str) -> String {
    let first = show_clock(st, dev);
    let synced = sim::device_clock(st, dev).synced;
    let src = st.dev(dev).cfg().ntp_source.clone().filter(|s| !s.is_empty());
    format!(
        "{}\n{}{}",
        first,
        if synced { "Time source is NTP" } else { "No time source" },
        src.map(|s| format!(" (source interface {})", s)).unwrap_or_default()
    )
}
pub fn ntp_associations(st: &mut State, dev: &str) -> String {
    let ok = sim::ntp_synced(st, dev);
    let d = st.dev(dev);
    let mut o = vec![String::new(), "  address         ref clock       st   when   poll reach  delay  offset   disp".to_string()];
    for (i, n) in d.cfg().ntp_servers.iter().enumerate() {
        let good = ok && i == 0;
        o.push(format!(
            "{}{}{}{}{}{}{}{}{}{}",
            if good { "*~" } else { " ~" },
            pad(n, 16),
            pad(if good { ".GPS." } else { ".INIT." }, 16),
            pad_l(if good { "1" } else { "16" }, 2),
            pad_l(if good { "37" } else { "-" }, 7),
            pad_l(64, 7),
            pad_l(if good { "377" } else { "0" }, 6),
            pad_l(if good { "1.230" } else { "0.000" }, 7),
            pad_l(if good { "0.452" } else { "0.000" }, 8),
            pad_l(if good { "0.911" } else { "15937" }, 7)
        ));
    }
    o.push(" * sys.peer, # selected, + candidate, - outlyer, x falseticker, ~ configured".into());
    o.join("\n")
}
pub fn ip_cef(st: &mut State, dev: &str, ip: &str) -> String {
    let dd = get_d(st);
    let st = &*st;
    match sim::route_lookup(st, &dd, dev, ip) {
        None => "0.0.0.0/0\n  no route".into(),
        Some(rt) => {
            let pfx = prefix(&rt.mask);
            format!(
                "{}/{}\n  {}",
                rt.net,
                pfx,
                match rt.nh.as_ref().filter(|s| !s.is_empty()) {
                    Some(nh) => format!("nexthop {} {}", nh, rt.iface),
                    None => format!("attached to {}", rt.iface),
                }
            )
        }
    }
}
pub fn ntp_status(st: &mut State, dev: &str) -> String {
    if st.dev(dev).cfg().ntp_servers.is_empty() {
        return "%NTP is not enabled.".into();
    }
    let ok = sim::ntp_synced(st, dev);
    if ok {
        let first = st.dev(dev).cfg().ntp_servers[0].clone();
        return format!("Clock is synchronized, stratum 3, reference is {}\nnominal freq is 119.2092 Hz, actual freq is 119.2090 Hz, precision is 2**18\nreference time is E8A1F3C2.1D2F9A11 (08:00:01.114 CEST Tue Sep 29 2026)\nclock offset is 0.4521 msec, root delay is 1.23 msec", first);
    }
    "Clock is unsynchronized, stratum 16, no reference clock\nnominal freq is 119.2092 Hz, actual freq is 119.2092 Hz, precision is 2**18\nreference time is 00000000.00000000 (00:00:00.000 UTC Mon Jan 1 1900)".into()
}
pub fn show_logging(st: &mut State, dev: &str) -> String {
    let d = st.dev(dev);
    let c = &d.cfg().logging;
    let n = d.rt.logs.len();
    let mut o = vec![
        "Syslog logging: enabled (0 messages dropped, 0 messages rate-limited, 0 flushes, 0 overruns, xml disabled, filtering disabled)".to_string(),
        String::new(),
        "No Active Message Discriminator.".to_string(),
        String::new(),
        "No Inactive Message Discriminator.".to_string(),
        String::new(),
        format!("    Console logging: level debugging, {} messages logged, xml disabled,", n + 40),
        "                     filtering disabled".to_string(),
        "    Monitor logging: level debugging, 0 messages logged, xml disabled,".to_string(),
        "                     filtering disabled".to_string(),
        format!("    Buffer logging:  {}", if c.buffered { format!("level debugging, {} messages logged, xml disabled,", n) } else { "disabled".to_string() }),
        "                    filtering disabled".to_string(),
        "    Exception Logging: size (8192 bytes)".to_string(),
        "    Count and timestamp logging messages: disabled".to_string(),
        "    Persistent logging: disabled".to_string(),
        String::new(),
        "No active filter modules.".to_string(),
        String::new(),
        format!("    Trap logging: level informational, {} message lines logged", n + 12),
    ];
    for h in &c.hosts {
        o.push(format!("        Logging to {}  (udp port 514, audit disabled,", h));
        o.push("              link up),".into());
        o.push(format!("              {} message lines logged,", n));
        o.push("              0 message lines rate-limited,".into());
        o.push("              0 message lines dropped-by-MD,".into());
        o.push("              xml disabled, sequence number disabled".into());
        o.push("              filtering disabled".into());
    }
    if c.buffered {
        o.push(String::new());
        o.push(format!("Log Buffer ({} bytes):", c.size));
        o.push(String::new());
        for l in &d.rt.logs {
            o.push(l.clone());
        }
    }
    o.join("\n")
}

pub fn interfaces_description(st: &mut State, dev: &str) -> String {
    let dd = get_d(st);
    let st = &*st;
    let d = st.dev(dev);
    let mut o = vec!["Interface                      Status         Protocol Description".to_string()];
    for n in iface_names(d) {
        let i = d.ifc(&n).unwrap();
        if i.internal {
            continue;
        }
        let line = brief_line(st, &dd, d, &n);
        // JS skär på kolumn 60/82 (ger tom Protocol-kolumn); samma här för identisk utdata
        let mut s = slice_chars(&line, 60, 82).trim().to_string();
        let pr = line.chars().skip(82).collect::<String>().trim().to_string();
        if s == "administratively down" {
            s = "admin down".into();
        }
        o.push(format!("{}{}{}{}", pad(U::short_if(&n), 31), pad(s, 15), pad(pr, 9), i.description));
    }
    o.join("\n")
}

pub fn vlan_id(st: &mut State, dev: &str, v: u16) -> String {
    let dd = get_d(st);
    let full_s = vlan_brief(st, dev);
    let full: Vec<&str> = full_s.split('\n').collect();
    let vs = v.to_string();
    let idx = full.iter().position(|l| {
        l.strip_prefix(vs.as_str()).and_then(|r| r.chars().next()).map(|c| c.is_whitespace()).unwrap_or(false)
    });
    let idx = match idx {
        Some(i) => i,
        None => return format!("VLAN id {} not found in current VLAN database", v),
    };
    let mut out: Vec<String> = vec![
        String::new(),
        "VLAN Name                             Status    Ports".to_string(),
        "---- -------------------------------- --------- -------------------------------".to_string(),
        full[idx].to_string(),
    ];
    for l in &full[idx + 1..] {
        if l.chars().take(40).filter(|c| c.is_whitespace()).count() == 40 {
            out.push(l.to_string());
        } else {
            break;
        }
    }
    let st = &*st;
    let d = st.dev(dev);
    let trunks: Vec<String> = phys_ports(d)
        .into_iter()
        .filter(|n| sim::op_mode(st, &d.id, n) == "trunk" && sim::trunk_vlans(d, d.ifc(n).unwrap()).contains(&v) && port_state_i(st, &dd, d, n) == "connected")
        .map(|n| U::short_if(&n))
        .collect();
    out.push(String::new());
    out.push("VLAN Type  SAID       MTU   Parent RingNo BridgeNo Stp  BrdgMode Trans1 Trans2".into());
    out.push("---- ----- ---------- ----- ------ ------ -------- ---- -------- ------ ------".into());
    out.push(format!("{}enet  {}1500  -      -      -        -    -        0      0", pad(v, 5), pad(100000 + v as u32, 11)));
    if !trunks.is_empty() {
        out.push(String::new());
        out.push(format!("Trunkar som bär VLAN {}: {}", v, trunks.join(", ")));
    }
    out.join("\n")
}

pub fn processes_cpu(st: &mut State, dev: &str) -> String {
    let dd = get_d(st);
    let d = st.dev(dev);
    let storm = dd.storm.values().any(|l| l.contains(&d.id));
    let c5 = if storm { 99 } else { 3 + (U::hash(&d.id) % 5) };
    let c1 = if storm { 98 } else { c5 + 1 };
    let c5m = if storm { 96 } else { c5 };
    let mut o = vec![
        format!("CPU utilization for five seconds: {}%/{}%; one minute: {}%; five minutes: {}%", c5, if storm { 91 } else { 0 }, c1, c5m),
        " PID Runtime(ms)     Invoked      uSecs   5Sec   1Min   5Min TTY Process".to_string(),
    ];
    let procs: [[&str; 8]; 3] = if storm {
        [
            ["96", "98765432", "1234567", "80", "71.43%", "70.12%", "69.80%", "Hulc LED Process"],
            ["4", "5432100", "99887", "54", "19.81%", "18.90%", "18.02%", "ARP Input"],
            ["18", "120000", "5000", "24", "4.51%", "4.40%", "4.33%", "Spanning Tree"],
        ]
    } else {
        [
            ["96", "12345", "8765", "1408", "1.12%", "1.05%", "1.01%", "Hulc LED Process"],
            ["4", "2345", "1998", "1173", "0.15%", "0.12%", "0.11%", "ARP Input"],
            ["18", "980", "877", "1117", "0.08%", "0.07%", "0.07%", "Spanning Tree"],
        ]
    };
    for p in procs {
        o.push(format!("{}{}{}{}{}{}{}   0 {}", pad_l(p[0], 4), pad_l(p[1], 12), pad_l(p[2], 13), pad_l(p[3], 11), pad_l(p[4], 7), pad_l(p[5], 7), pad_l(p[6], 7), p[7]));
    }
    o.join("\n")
}

pub fn cdp_detail(st: &mut State, dev: &str) -> String {
    let dd = get_d(st);
    let st = &*st;
    let d = st.dev(dev);
    let mut o: Vec<String> = vec![];
    let mut count = 0;
    for n in d.cfg().ifaces.keys() {
        let p = match dd.ports.get(&sim::key(&d.id, n)) {
            Some(p) if p.up => p,
            _ => continue,
        };
        let peer = match st.devices.get(&p.peer.dev) {
            Some(p) => p,
            None => continue,
        };
        let is_ios = peer.os == Os::Ios;
        if !is_ios && peer.kind != Kind::Ap {
            continue;
        }
        let ip: Option<String> = if is_ios {
            sim::dev_eps(&dd, &peer.id).into_iter().find(|e| e.up && e.ip.as_ref().map(|i| !i.is_empty()).unwrap_or(false)).and_then(|e| e.ip.clone())
        } else {
            peer.nic.as_ref().and_then(|n| n.static_.as_ref()).map(|s| s.ip.clone())
        };
        o.push("-------------------------".into());
        o.push(format!("Device ID: {}", if is_ios { format!("{}.nordvik.example", peer.cfg().hostname) } else { peer.id.clone() }));
        o.push("Entry address(es): ".into());
        if let Some(ip) = &ip {
            o.push(format!("  IP address: {}", ip));
        }
        o.push(format!(
            "Platform: cisco {},  Capabilities: {}",
            if peer.model.is_empty() { "AIR-CAP3702I-E-K9" } else { peer.model.as_str() },
            if peer.kind == Kind::Router { "Router Switch IGMP" } else if peer.kind == Kind::Ap { "Trans-Bridge Source-Route-Bridge IGMP" } else { "Switch IGMP" }
        ));
        o.push(format!("Interface: {},  Port ID (outgoing port): {}", n, if is_ios { p.peer.port.clone() } else { "GigabitEthernet0".to_string() }));
        o.push("Holdtime : 164 sec".into());
        o.push(String::new());
        o.push("Version :".into());
        o.push(
            if peer.kind == Kind::Router {
                "Cisco IOS Software, C2951 Software (C2951-UNIVERSALK9-M), Version 15.2(4)M11"
            } else if peer.kind == Kind::Ap {
                "Cisco AP Software, ap3g2-k9w8 Version: 15.3(3)JA"
            } else {
                "Cisco IOS Software, C3560 Software (C3560-IPSERVICESK9-M), Version 12.2(55)SE12"
            }
            .into(),
        );
        o.push(String::new());
        if is_ios {
            if let Some(nv) = peer.ifc(&p.peer.port).map(|i| i.native).filter(|v| *v != 0) {
                o.push(format!("Native VLAN: {}", nv));
            }
        }
        o.push(format!("Duplex: {}", p.neg.as_ref().map(|n| n.duplex.clone()).unwrap_or_else(|| "full".into())));
        if let Some(ip) = &ip {
            o.push("Management address(es): ".into());
            o.push(format!("  IP address: {}", ip));
        }
        o.push(String::new());
        count += 1;
    }
    o.push(format!("Total cdp entries displayed : {}", count));
    o.join("\n")
}

pub fn stp_summary(st: &mut State, dev: &str) -> String {
    let dd = get_d(st);
    let st = &*st;
    let d = st.dev(dev);
    let c = d.cfg();
    let vl: Vec<u16> = c.vlans.keys().copied().collect();
    let root: Vec<String> = vl
        .iter()
        .filter(|v| dd.stp.get(&d.id).and_then(|m| m.get(v)).map(|i| i.is_root).unwrap_or(false))
        .map(|v| vlan4(v))
        .collect();
    let mut o: Vec<String> = vec![
        "Switch is in pvst mode".to_string(),
        format!("Root bridge for: {}", if root.is_empty() { "none".to_string() } else { root.join(", ") }),
        "Extended system ID                      is enabled".to_string(),
        "Portfast Default                        is disabled".to_string(),
        "PortFast BPDU Guard Default             is disabled".to_string(),
        "Loopguard Default                       is disabled".to_string(),
        "EtherChannel misconfig guard            is enabled".to_string(),
        "UplinkFast                              is disabled".to_string(),
        "BackboneFast                            is disabled".to_string(),
        String::new(),
        "Name                   Blocking Listening Learning Forwarding STP Active".to_string(),
        "---------------------- -------- --------- -------- ---------- ----------".to_string(),
    ];
    let (mut tb, mut tf, mut nv) = (0usize, 0usize, 0usize);
    let phys = phys_ports(d);
    for v in vl {
        if c.stp_off.contains(&v) {
            continue;
        }
        let ports: Vec<&String> = phys
            .iter()
            .filter(|n| {
                match dd.ports.get(&sim::key(&d.id, n)) {
                    Some(p) if p.up => {}
                    _ => return false,
                }
                let i = d.ifc(n).unwrap();
                if sim::op_mode(st, &d.id, n) == "trunk" { sim::trunk_vlans(d, i).contains(&v) } else { i.access_vlan == v }
            })
            .collect();
        if ports.is_empty() {
            continue;
        }
        nv += 1;
        let b = ports.iter().filter(|n| dd.blocked.contains_key(&format!("{}#{}", sim::key(&d.id, n), v))).count();
        tb += b;
        tf += ports.len() - b;
        o.push(format!("{}{}{}{}{}{}", pad(vlan4(v), 23), pad_l(b, 8), pad_l(0, 10), pad_l(0, 9), pad_l(ports.len() - b, 11), pad_l(ports.len(), 11)));
    }
    o.push("---------------------- -------- --------- -------- ---------- ----------".into());
    o.push(format!("{}{}{}{}{}{}", pad(format!("{} vlans", nv), 23), pad_l(tb, 8), pad_l(0, 10), pad_l(0, 9), pad_l(tf, 11), pad_l(tb + tf, 11)));
    if !c.stp_off.is_empty() {
        let mut so = c.stp_off.clone();
        so.sort();
        o.push(String::new());
        o.push(format!("Spanning tree is DISABLED for VLAN {}", U::vlan_list_str(&so)));
    }
    o.join("\n")
}

pub fn stp_blocked(st: &mut State, dev: &str) -> String {
    let dd = get_d(st);
    let d = st.dev(dev);
    let mut o = vec![
        String::new(),
        "Name                 Blocked Interfaces List".to_string(),
        "-------------------- ------------------------------------".to_string(),
    ];
    // (vlan, portnummer, port, markering) – HashMap saknar ordning, så sortera
    let pre = format!("{}|", d.id);
    let mut items: Vec<(u32, u32, String, String)> = vec![];
    for (k, val) in &dd.blocked {
        if let Some(rest) = k.strip_prefix(&pre) {
            let mut it = rest.splitn(2, '#');
            let port = it.next().unwrap_or("").to_string();
            let vlan = it.next().unwrap_or("0").to_string();
            items.push((vlan.parse().unwrap_or(0), sim::port_num(&port), port, val.clone()));
        }
    }
    items.sort();
    let n = items.len();
    for (v, _, port, val) in items {
        o.push(format!("{}{}{}", pad(vlan4(v), 21), U::short_if(&port), if val == "BKN*" { " (*PVID_Inc)" } else { "" }));
    }
    o.push(String::new());
    o.push(format!("Number of blocked ports (segments) in the system : {}", n));
    o.join("\n")
}

pub fn inventory(st: &mut State, dev: &str) -> String {
    let dd = get_d(st);
    let d = st.dev(dev);
    let sn = format!("FOC{}X{}", U::hash(&d.id) % 900000 + 100000, U::hash(&format!("{}x", d.id)) % 90 + 10);
    if d.kind == Kind::Router {
        return format!(
            "NAME: \"CISCO2951/K9 chassis\", DESCR: \"CISCO2951/K9 chassis\"\nPID: CISCO2951/K9      , VID: V05 , SN: {}\n\nNAME: \"PVDM3-32 on Motherboard\", DESCR: \"PVDMIII DSP SIMM with two DSPs\"\nPID: PVDM3-32          , VID: V01 , SN: {}",
            sn,
            sn.replacen("FOC", "FOX", 1)
        );
    }
    let mut o = vec![format!("NAME: \"1\", DESCR: \"{}\"", d.model), format!("PID: {}    , VID: V02  , SN: {}", d.model, sn)];
    for n in phys_ports(d) {
        if d.ifc(&n).unwrap().sfp && dd.ports.contains_key(&sim::key(&d.id, &n)) {
            o.push(String::new());
            o.push(format!("NAME: \"{}\", DESCR: \"1000BaseSX SFP\"", n));
            o.push(format!("PID: GLC-SX-MMD          , VID: V01  , SN: AGM{}", U::hash(&format!("{}{}", n, d.id)) % 9000000));
        }
    }
    o.join("\n")
}

pub fn counters_errors(st: &mut State, dev: &str) -> String {
    let d = st.dev(dev);
    let ports = phys_ports(d);
    let get = |n: &String| d.rt.counters.get(n).cloned().unwrap_or_default();
    let mut o = vec![String::new(), "Port        Align-Err     FCS-Err    Xmit-Err     Rcv-Err  UnderSize  OutDiscards".to_string()];
    for n in &ports {
        let c = get(n);
        o.push(format!("{}{}{}{}{}{}{}", pad(U::short_if(n), 10), pad_l(0, 12), pad_l(fl(c.crc), 12), pad_l(0, 12), pad_l(fl(c.crc + c.runts), 12), pad_l(fl(c.runts), 11), pad_l(0, 13)));
    }
    o.push(String::new());
    o.push("Port      Single-Col  Multi-Col   Late-Col  Excess-Col  Carri-Sen      Runts".into());
    for n in &ports {
        let c = get(n);
        o.push(format!("{}{}{}{}{}{}{}", pad(U::short_if(n), 10), pad_l(fl(c.coll * 0.6), 10), pad_l(fl(c.coll * 0.4), 11), pad_l(fl(c.late), 11), pad_l(0, 12), pad_l(0, 11), pad_l(fl(c.runts), 11)));
    }
    o.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::model::{self, AddrSpec};

    fn state() -> State {
        State::new(model::build_golden(0))
    }

    #[test]
    fn running_config_sw1() {
        let mut st = state();
        let t = running_config(&mut st, "SW1");
        assert!(t.starts_with("Building configuration...\n\nCurrent configuration : "), "{}", t);
        assert!(t.contains("hostname SW-Nordvik-1"));
        assert!(t.contains("switchport trunk native vlan 999"));
        assert!(t.contains("\nend"));
        assert!(t.contains("spanning-tree mode pvst"));
    }

    #[test]
    fn running_config_router() {
        let mut st = state();
        let t = running_config(&mut st, "R1");
        assert!(t.contains("hostname R-Nordvik-1"));
        assert!(t.contains("encapsulation dot1Q 10"));
        assert!(t.contains("ip nat inside"));
        assert!(t.contains("line aux 0"));
    }

    #[test]
    fn startup_saknas_och_finns() {
        let mut st = state();
        st.dev_mut("SW1").startup = None;
        assert_eq!(startup_config(&mut st, "SW1"), "startup-config is not present");
        let c = st.dev("SW1").cfg().clone();
        st.dev_mut("SW1").startup = Some(c);
        let t = startup_config(&mut st, "SW1");
        assert!(t.starts_with("Using "));
        assert!(t.contains("out of 524288 bytes\n!"));
    }

    #[test]
    fn ace_text_rader() {
        let r = Ace::ext(10, "deny", AddrSpec::net("192.168.1.128", "0.0.0.63"), AddrSpec::net("192.168.1.0", "0.0.0.255"));
        assert_eq!(ace_text(true, &r), "deny   ip 192.168.1.128 0.0.0.63 192.168.1.0 0.0.0.255");
        let mut r2 = Ace::ext(20, "permit", AddrSpec::any(), AddrSpec::host("10.0.0.1"));
        r2.proto = Some("tcp".into());
        r2.dport = Some(80);
        assert_eq!(ace_text(true, &r2), "permit tcp any host 10.0.0.1 eq www");
        let s = Ace::std(10, "permit", AddrSpec::net("10.0.0.0", "0.0.0.255"));
        assert_eq!(ace_text(false, &s), "permit 10.0.0.0 0.0.0.255");
    }

    #[test]
    fn hash_secret_form() {
        let h = hash_secret("nordvik");
        assert!(h.starts_with("$9$"));
        assert_eq!(h.chars().count(), 3 + 14 + 1 + 43);
        assert_eq!(h, hash_secret("nordvik"));
        assert_ne!(h, hash_secret("annat"));
    }

    #[test]
    fn iface_ordning() {
        let st = state();
        let n = iface_names(st.dev("R1"));
        assert_eq!(n[0], "Embedded-Service-Engine0/0");
        let i0 = n.iter().position(|x| x == "GigabitEthernet0/0").unwrap();
        assert_eq!(n[i0 + 1], "GigabitEthernet0/0.10");
        let sw = phys_ports(st.dev("SW1"));
        assert_eq!(sw[0], "GigabitEthernet0/1");
        assert_eq!(sw.len(), 28);
    }
}
