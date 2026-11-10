//! Veckorna, byggena och kurs 2: varje fel ska synas från start (check false) och försvinna
//! när CLI-lösningen körs (check true). Port av test/levels.js, test/build.js och test/kurs2.js.
use krabba_spelet::cli::ios::IosSession;
use krabba_spelet::cli::lb::LbSession;
use krabba_spelet::cli::wlc::WlcSession;
use krabba_spelet::cli::Cli;
use krabba_spelet::game::levels::{self, Week};
use krabba_spelet::net::model::StaticIp;
use krabba_spelet::net::sim::{self as S, State};

struct Rep {
    fails: Vec<String>,
}
impl Rep {
    fn fail(&mut self, m: impl Into<String>) {
        let m = m.into();
        println!("   FEL: {}", m);
        self.fails.push(m);
    }
    fn done(self, what: &str) {
        assert!(self.fails.is_empty(), "{} fel i {}:\n{}", self.fails.len(), what, self.fails.join("\n"));
    }
}

/// Kör kommandon i en IOS-session (privilegierat läge) och ger all utskrift.
fn ios(rep: &mut Rep, st: &mut State, dev: &str, cmds: &[&str], bad: &[&str]) -> String {
    let mut s = IosSession::new(st, dev, "console", true, None);
    let mut all = String::new();
    for c in cmds {
        let mut r = s.handle_line(st, c);
        let mut out = r.out.clone();
        for ch in &r.stream {
            out.push_str(&ch.text);
        }
        while s.pending.is_some() {
            r = s.handle_line(st, "");
            out.push_str(&r.out);
        }
        if bad.iter().any(|b| out.contains(b)) {
            rep.fail(format!("CLI [{}] {}\n{}", dev, c, out));
        }
        all.push_str(&out);
        all.push('\n');
    }
    S::refresh(st);
    all
}
const BAD1: &[&str] = &["Invalid", "Incomplete", "Ambiguous", "rejected", "Bad mask", "overlaps"];
const BAD_B: &[&str] = &["Invalid", "Incomplete", "Ambiguous", "rejected", "Bad mask", "overlaps", "ERROR"];
const BAD_K: &[&str] = &["Invalid", "Incomplete", "Ambiguous", "already", "not active"];

fn tick(st: &mut State, n: usize) {
    for _ in 0..n {
        S::tick(st, 1.0);
    }
}
fn renew(st: &mut State, h: &str) {
    S::dhcp(st, h);
    S::refresh(st);
}
fn karim_link_ok(st: &mut State) {
    let l = st.links.iter_mut().find(|l| l.b.dev == "PC-Karim").unwrap();
    l.state = krabba_spelet::net::model::LinkState::Ok;
    S::touch(st);
}

fn lines(rep: &mut Rep, st: &mut State, dev: &str, cmds: &[&str]) {
    ios(rep, st, dev, cmds, BAD1);
}

fn solve1(rep: &mut Rep, st: &mut State, id: &str, w: &Week) {
    match id {
        "v1k1" => lines(rep, st, "SW1", &["conf t", "line con 0", "speed 9600", "end", "write memory"]),
        "v1k2" => {
            lines(rep, st, "SW1", &["conf t", "int gi0/5", "no shut", "end", "wr"]);
            renew(st, "PC-Anna");
        }
        "v1n1" => lines(rep, st, "SW2", &["conf t", "hostname SW-Nordvik-2", "end", "copy running-config startup-config"]),
        "v2k1" => {
            karim_link_ok(st);
            renew(st, "PC-Karim");
        }
        "v2k2" => lines(rep, st, "SW2", &["conf t", "int gi0/24", "speed auto", "duplex auto", "end", "wr"]),
        "v2n1" => {
            lines(rep, st, "SW1", &["conf t", "int gi0/7", "sw acc vlan 20", "end", "wr"]);
            renew(st, "PC-Bo");
        }
        "v3k1" => {
            let a = st.devices.get_mut("PC-Anna").unwrap();
            let nic = a.nic.as_mut().unwrap();
            nic.dhcp = false;
            nic.static_ = Some(StaticIp { ip: "192.168.1.19".into(), mask: "255.255.255.192".into(), gw: Some("192.168.1.1".into()), dns: vec!["192.168.1.1".into()] });
            a.lease = None;
            S::touch(st);
            S::refresh(st);
        }
        "v3k2" => {
            lines(rep, st, "R1", &["conf t", "ip dhcp pool KONTOR", "network 192.168.1.0 255.255.255.192", "end", "wr"]);
            renew(st, "PC-Karim");
        }
        "v3n1" => {
            lines(rep, st, "R1", &["conf t", "ip dhcp pool EKONOMI", "dns-server 192.168.1.65", "end"]);
            renew(st, "PC-Maja");
        }
        "v4k1" => lines(rep, st, "SW1", &["conf t", "interface range gi0/24 - 25", "switchport trunk allowed vlan add 20", "end"]),
        "v4k2" => lines(rep, st, "SW2", &["conf t", "int range gi0/24-25", "sw trunk native vlan 999", "end"]),
        "v4n1" => {
            st.flags.misc.insert("stpLooked".into(), true);
        }
        "v5k1" => lines(rep, st, "RB", &["conf t", "ip route 0.0.0.0 0.0.0.0 10.0.0.1", "end"]),
        "v5k2" => lines(rep, st, "R1", &["conf t", "no ip route 0.0.0.0 0.0.0.0 203.0.113.9", "ip route 0.0.0.0 0.0.0.0 203.0.113.1", "end"]),
        "v5n1" => lines(rep, st, "R1", &["conf t", "int gi0/0.20", "encapsulation dot1Q 20", "end"]),
        "v6k1" => lines(rep, st, "R1", &["conf t", "int g0/0.10", "ip nat inside", "end"]),
        "v6k2" => lines(rep, st, "R1", &["conf t", "ip domain-name nordvik.example", "crypto key generate rsa modulus 2048", "end"]),
        "v6n1" => lines(rep, st, "R1", &["conf t", "line vty 5 15", "transport input ssh", "end"]),
        "v7k1" => {
            lines(rep, st, "SW1", &["conf t", "spanning-tree vlan 10", "end"]);
            lines(rep, st, "SW2", &["conf t", "spanning-tree vlan 10", "end"]);
        }
        "v7k2" => karim_link_ok(st),
        "v7n1" => lines(rep, st, "SW2", &["conf t", "ntp server 192.168.1.16", "end"]),
        "v8k1" => lines(rep, st, "SWB", &["conf t", "int gi0/1", "power inline auto", "end"]),
        "v8k2" => {
            let mut s = WlcSession::new(st, "WLC");
            s.handle(st, "admin");
            s.handle(st, "x");
            for c in ["config wlan disable 2", "config wlan interface 2 gast", "config wlan enable 2"] {
                let r = s.handle(st, c);
                if !r.out.trim().is_empty() {
                    println!("   WLC: {} {}", c, r.out);
                }
            }
            S::touch(st);
            renew(st, "Gast-mobil");
        }
        "v8n1" => {
            let mut s = WlcSession::new(st, "WLC");
            s.handle(st, "admin");
            s.handle(st, "x");
            s.handle(st, "config 802.11b channel ap AP-Lager-3 11");
            S::touch(st);
        }
        "v9k1" => lines(rep, st, "R1", &["conf t", "int gi0/0.10", "no ip access-group KONTOR-UT out", "ip access-group KONTOR-UT in", "end"]),
        "v9k2" => lines(rep, st, "R1", &["conf t", "ip access-list extended GAST", "permit ip any any", "end"]),
        "v9n1" => {
            levels::remove_mini_switch(st);
            S::refresh(st);
            lines(rep, st, "SW1", &["conf t", "int gi0/9", "shutdown", "no shutdown", "end"]);
            renew(st, "Gast-laptop");
        }
        "v10k1" => {
            lines(rep, st, "RB", &["conf t", "ip access-list extended VPN-TRAFIK", "no 10", "10 permit ip 192.168.2.0 0.0.0.63 192.168.1.0 0.0.0.63", "end"]);
            // Bara det ena felet rättat: fortfarande trasigt
            if (w.tasks[0].check)(st) {
                rep.fail("v10k1 löst med bara ett av två fel rättat");
            }
            lines(rep, st, "R1", &["conf t", "ip access-list extended NAT-UT", "5 deny ip 192.168.1.0 0.0.0.63 192.168.2.0 0.0.0.63", "end"]);
        }
        "v10k2" => lines(rep, st, "RB", &["conf t", "int gi0/1", "ip tcp adjust-mss 1360", "end", "wr"]),
        "v10n1" => {
            let mut s = LbSession::new(st, "LB");
            s.handle(st, "admin");
            s.handle(st, "x");
            let r = s.handle(st, "config pool TID-POOL monitor http");
            if r.out.contains('%') {
                rep.fail(format!("LB: {}", r.out));
            }
        }
        _ => rep.fail(format!("ingen lösning för {}", id)),
    }
}

#[test]
fn data_shape() {
    let all = levels::all();
    assert_eq!(all.len(), 19);
    let nums: Vec<u32> = all.iter().map(|w| w.week).collect();
    assert_eq!(nums, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 101, 102, 103, 104, 105]);
    for w in &all {
        for t in &w.tasks {
            assert_eq!(t.hints.len(), 3, "{} ska ha tre ledtrådar", t.id);
            if w.build.is_none() {
                let r = t.report.as_ref().unwrap_or_else(|| panic!("{} saknar rapport", t.id));
                assert_eq!((r.cause.len(), r.fix.len()), (4, 4), "{}", t.id);
            }
        }
        if w.build.is_none() {
            assert!(!w.quiz.is_empty(), "vecka {} saknar quiz", w.week);
            assert!(w.challenge.is_some());
        }
    }
    assert_eq!(levels::name(levels::def(101).as_ref()), "Bygge 1");
    assert_eq!(levels::name(levels::def(3).as_ref()), "Vecka 3");
    assert_eq!(levels::name(None), "Fri träning");
}

#[test]
fn veckor_1_till_10() {
    let mut rep = Rep { fails: vec![] };
    for w in levels::all().into_iter().filter(|w| w.kurs == 1 && w.build.is_none()) {
        let (mut st, def) = levels::start(w.week);
        let def = def.unwrap();
        tick(&mut st, 3);
        println!("Vecka {}: {}", w.week, w.title);
        for t in &def.tasks {
            if (t.check)(&mut st) {
                rep.fail(format!("{} är redan löst från start", t.id));
            }
        }
        for t in &def.tasks {
            solve1(&mut rep, &mut st, t.id, &def);
            tick(&mut st, 2);
            let after = (t.check)(&mut st);
            println!("  {} {} {}", if after { "OK  " } else { "FEL " }, t.id, t.title);
            if !after {
                rep.fail(format!("{} är inte löst efter lösningen", t.id));
            }
        }
    }
    rep.done("veckorna 1–10");
}

fn ssh_cmds() -> Vec<&'static str> {
    vec!["ip domain-name nordvik.example", "crypto key generate rsa modulus 2048", "ip ssh version 2", "username drift privilege 15 secret nordvik", "line vty 0 15", "transport input ssh", "login local", "exit"]
}
fn trunk(r: &str, extra: &[&'static str]) -> Vec<String> {
    let mut v = vec![format!("interface {}", r), "switchport trunk encapsulation dot1q".into(), "switchport mode trunk".into()];
    v.extend(extra.iter().map(|s| s.to_string()));
    v.push("exit".into());
    v
}
fn run(rep: &mut Rep, st: &mut State, dev: &str, cmds: Vec<String>) {
    let refs: Vec<&str> = cmds.iter().map(|s| s.as_str()).collect();
    ios(rep, st, dev, &refs, BAD_B);
}
fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn solve_b(rep: &mut Rep, st: &mut State, id: &str) {
    let ssh: Vec<String> = ssh_cmds().iter().map(|x| x.to_string()).collect();
    match id {
        "b1a" => run(rep, st, "SW1", s(&["conf t", "hostname SW-Nordvik-1", "line con 0", "logging synchronous", "end", "write memory"])),
        "b1b" => run(rep, st, "SW1", s(&["conf t", "vlan 10", "name KONTOR", "vlan 20", "name EKONOMI", "vlan 30", "name GAST", "vlan 99", "name DRIFT", "end"])),
        "b1c" => run(rep, st, "SW1", s(&["conf t", "interface range gi0/5 - 6", "switchport mode access", "switchport access vlan 10", "spanning-tree portfast", "interface range gi0/11 - 12", "switchport mode access", "switchport access vlan 10",
            "interface range gi0/7 - 8", "switchport mode access", "switchport access vlan 20", "interface gi0/9", "switchport mode access", "switchport access vlan 30", "interface gi0/10", "switchport mode access", "switchport access vlan 99", "interface gi0/20", "switchport mode access", "switchport access vlan 99", "end"])),
        "b1d" => {
            let tr = ["switchport trunk native vlan 999", "switchport trunk allowed vlan 10,20,30,99", "switchport nonegotiate"];
            let mut v = s(&["conf t"]);
            v.extend(trunk("gi0/1", &tr));
            v.extend(trunk("range gi0/24 - 25", &tr));
            v.push("end".into());
            run(rep, st, "SW1", v);
        }
        "b1e" => {
            let mut v = s(&["conf t", "interface vlan 99", "ip address 192.168.1.194 255.255.255.192", "no shutdown", "exit", "ip default-gateway 192.168.1.193"]);
            v.extend(ssh);
            v.extend(s(&["end", "wr"]));
            run(rep, st, "SW1", v);
        }
        "b2a" => run(rep, st, "R1", s(&["conf t", "interface gi0/0", "no shutdown", "interface gi0/0.10", "encapsulation dot1Q 10", "ip address 192.168.1.1 255.255.255.192", "interface gi0/0.20", "encapsulation dot1Q 20", "ip address 192.168.1.65 255.255.255.192",
            "interface gi0/0.30", "encapsulation dot1Q 30", "ip address 192.168.1.129 255.255.255.192", "interface gi0/0.99", "encapsulation dot1Q 99", "ip address 192.168.1.193 255.255.255.192", "end"])),
        "b2b" => run(rep, st, "R1", s(&["conf t", "ip dhcp excluded-address 192.168.1.1 192.168.1.19", "ip dhcp excluded-address 192.168.1.65 192.168.1.79", "ip dhcp excluded-address 192.168.1.129 192.168.1.139",
            "ip dhcp pool KONTOR", "network 192.168.1.0 255.255.255.192", "default-router 192.168.1.1", "dns-server 192.168.1.1", "ip dhcp pool EKONOMI", "network 192.168.1.64 255.255.255.192", "default-router 192.168.1.65", "dns-server 192.168.1.65",
            "ip dhcp pool GAST", "network 192.168.1.128 255.255.255.192", "default-router 192.168.1.129", "dns-server 9.9.9.9", "end"])),
        "b2c" => run(rep, st, "R1", s(&["conf t", "ip dns server", "ip host filserver 192.168.1.10", "ip name-server 9.9.9.9", "end"])),
        "b2d" => run(rep, st, "R1", s(&["conf t", "interface gi0/1", "ip address 203.0.113.10 255.255.255.0", "ip nat outside", "no shutdown", "interface gi0/0.10", "ip nat inside", "interface gi0/0.20", "ip nat inside", "interface gi0/0.30", "ip nat inside", "interface gi0/0.99", "ip nat inside", "exit",
            "ip route 0.0.0.0 0.0.0.0 203.0.113.1", "access-list 1 permit 192.168.1.0 0.0.0.255", "ip nat inside source list 1 interface gi0/1 overload", "ip nat inside source static 192.168.1.10 203.0.113.11", "end"])),
        "b2e" => run(rep, st, "R1", s(&["conf t", "interface gi0/2", "ip address 10.0.0.1 255.255.255.252", "ip nat inside", "no shutdown", "exit", "ip route 192.168.2.0 255.255.255.0 10.0.0.2", "access-list 1 permit 192.168.2.0 0.0.0.255", "end"])),
        "b2f" => {
            let mut v = s(&["conf t", "hostname R-Nordvik-1"]);
            v.extend(ssh);
            v.extend(s(&["end", "wr"]));
            run(rep, st, "R1", v);
        }
        "b3a" => run(rep, st, "SWB", s(&["conf t", "hostname SW-Boras-1", "vlan 40", "name LAGER", "vlan 50", "name TRADLOST-GAST", "vlan 99", "name DRIFT", "exit", "interface gi0/5", "switchport mode access", "switchport access vlan 40", "spanning-tree portfast", "end"])),
        "b3b" => {
            let mut v = s(&["conf t"]);
            v.extend(trunk("range gi0/1 - 3", &["switchport trunk native vlan 99", "switchport trunk allowed vlan 40,50,99"]));
            v.extend(trunk("gi0/24", &["switchport trunk native vlan 999", "switchport trunk allowed vlan 40,50,99"]));
            v.push("end".into());
            run(rep, st, "SWB", v);
        }
        "b3c" => run(rep, st, "RB", s(&["conf t", "interface gi0/0", "no shutdown", "interface gi0/0.40", "encapsulation dot1Q 40", "ip address 192.168.2.1 255.255.255.192", "interface gi0/0.50", "encapsulation dot1Q 50", "ip address 192.168.2.65 255.255.255.192",
            "interface gi0/0.99", "encapsulation dot1Q 99", "ip address 192.168.2.193 255.255.255.192", "interface gi0/2", "ip address 10.0.0.2 255.255.255.252", "no shutdown", "exit", "ip route 0.0.0.0 0.0.0.0 10.0.0.1", "end"])),
        "b3d" => run(rep, st, "RB", s(&["conf t", "ip dhcp excluded-address 192.168.2.1 192.168.2.19", "ip dhcp excluded-address 192.168.2.65 192.168.2.69", "ip dhcp pool LAGER", "network 192.168.2.0 255.255.255.192", "default-router 192.168.2.1", "dns-server 9.9.9.9",
            "ip dhcp pool TRADLOST-GAST", "network 192.168.2.64 255.255.255.192", "default-router 192.168.2.65", "dns-server 9.9.9.9", "end"])),
        "b3e" => {
            S::touch(st);
            S::refresh(st);
        }
        "b3f" => run(rep, st, "RB", s(&["conf t", "ip access-list extended GAST-B", "deny ip 192.168.2.64 0.0.0.63 192.168.0.0 0.0.255.255", "permit ip any any", "exit", "interface gi0/0.50", "ip access-group GAST-B in", "end"])),
        "b4a" => run(rep, st, "R1", s(&["conf t", "ip access-list extended GAST", "deny ip 192.168.1.128 0.0.0.63 192.168.1.0 0.0.0.255", "deny ip 192.168.1.128 0.0.0.63 192.168.2.0 0.0.0.255", "permit ip any any", "exit", "interface gi0/0.30", "ip access-group GAST in", "end"])),
        "b4b" => run(rep, st, "R1", s(&["conf t", "ip access-list extended KONTOR-UT", "deny ip 192.168.1.0 0.0.0.63 192.168.1.64 0.0.0.63", "permit ip any any", "exit", "interface gi0/0.10", "ip access-group KONTOR-UT in", "end"])),
        "b4c" => run(rep, st, "SW1", s(&["conf t", "interface gi0/9", "switchport port-security", "switchport port-security maximum 1", "switchport port-security mac-address sticky", "switchport port-security violation shutdown", "end"])),
        "b4d" => {
            for d in ["R1", "SW1", "SW2"] {
                run(rep, st, d, s(&["conf t", "ntp server 192.168.1.16", "end"]));
            }
        }
        "b4e" => {
            for d in ["R1", "SW1", "SW2"] {
                run(rep, st, d, s(&["conf t", "logging host 192.168.1.17", "service timestamps log datetime msec", "end"]));
            }
        }
        "b5a" => {
            let pol = ["crypto isakmp policy 10", "encryption aes 256", "hash sha256", "authentication pre-share", "group 14", "exit"];
            let mut a = s(&["conf t"]);
            a.extend(s(&pol));
            a.extend(s(&["crypto isakmp key Nordvik-VPN-2026 address 203.0.113.20", "end"]));
            run(rep, st, "R1", a);
            let mut b = s(&["conf t"]);
            b.extend(s(&pol));
            b.extend(s(&["crypto isakmp key Nordvik-VPN-2026 address 203.0.113.10", "end"]));
            run(rep, st, "RB", b);
        }
        "b5b" => {
            run(rep, st, "R1", s(&["conf t", "crypto ipsec transform-set NORDVIK-TS esp-aes 256 esp-sha256-hmac", "mode tunnel", "exit", "ip access-list extended VPN-TRAFIK", "permit ip 192.168.1.0 0.0.0.63 192.168.2.0 0.0.0.63", "permit ip 192.168.1.192 0.0.0.63 192.168.2.192 0.0.0.63", "end"]));
            run(rep, st, "RB", s(&["conf t", "crypto ipsec transform-set NORDVIK-TS esp-aes 256 esp-sha256-hmac", "mode tunnel", "exit", "ip access-list extended VPN-TRAFIK", "permit ip 192.168.2.0 0.0.0.63 192.168.1.0 0.0.0.63", "permit ip 192.168.2.192 0.0.0.63 192.168.1.192 0.0.0.63", "end"]));
        }
        "b5c" => {
            run(rep, st, "R1", s(&["conf t", "crypto map VPN-MAP 10 ipsec-isakmp", "set peer 203.0.113.20", "set transform-set NORDVIK-TS", "match address VPN-TRAFIK", "exit", "interface gi0/1", "crypto map VPN-MAP", "exit", "ip access-list extended NAT-UT", "5 deny ip 192.168.1.0 0.0.0.63 192.168.2.0 0.0.0.63", "end"]));
            run(rep, st, "RB", s(&["conf t", "crypto map VPN-MAP 10 ipsec-isakmp", "set peer 203.0.113.10", "set transform-set NORDVIK-TS", "match address VPN-TRAFIK", "exit", "interface gi0/1", "crypto map VPN-MAP", "exit", "ip access-list extended NAT-UT", "5 deny ip 192.168.2.0 0.0.0.63 192.168.1.0 0.0.0.63", "end"]));
        }
        "b5d" => {
            run(rep, st, "R1", s(&["conf t", "ip access-list extended NAT-UT", "6 deny ip 192.168.1.192 0.0.0.63 192.168.2.192 0.0.0.63", "end"]));
            run(rep, st, "RB", s(&["conf t", "ip access-list extended NAT-UT", "6 deny ip 192.168.2.192 0.0.0.63 192.168.1.192 0.0.0.63", "end"]));
        }
        "b5e" => run(rep, st, "RB", s(&["conf t", "interface gi0/1", "ip tcp adjust-mss 1360", "end"])),
        _ => rep.fail(format!("ingen lösning för {}", id)),
    }
}

#[test]
fn byggen() {
    let mut rep = Rep { fails: vec![] };
    for b in levels::all().into_iter().filter(|w| w.build.is_some()) {
        let (mut st, def) = levels::start(b.week);
        let def = def.unwrap();
        println!("Bygge {}: {}", b.build.unwrap(), b.title);
        for t in &def.tasks {
            if (t.check)(&mut st) {
                rep.fail(format!("{} är redan löst från start", t.id));
            }
        }
        for t in &def.tasks {
            solve_b(&mut rep, &mut st, t.id);
            tick(&mut st, 2);
            let ok = (t.check)(&mut st);
            println!("  {} {} {}", if ok { "OK  " } else { "FEL " }, t.id, t.title);
            if !ok {
                rep.fail(format!("{} är inte löst efter lösningen", t.id));
            }
        }
    }
    rep.done("byggen");
}

fn k(rep: &mut Rep, st: &mut State, dev: &str, cmds: &[&str]) -> String {
    ios(rep, st, dev, cmds, BAD_K)
}

fn solve_k(rep: &mut Rep, st: &mut State, id: &str) {
    match id {
        "v11k1" => { k(rep, st, "RM", &["conf t", "no ip route 0.0.0.0 0.0.0.0 10.0.0.1", "ip route 0.0.0.0 0.0.0.0 10.0.0.5", "end", "wr"]); }
        "v11k2" => { k(rep, st, "R1", &["conf t", "no ip route 192.168.3.0 255.255.255.192 10.0.0.2", "ip route 192.168.3.0 255.255.255.192 10.0.0.6", "end", "wr"]); }
        "v11n1" => {
            k(rep, st, "RB", &["conf t", "ip route 192.168.3.0 255.255.255.192 10.0.0.1", "ip route 192.168.3.0 255.255.255.192 10.0.0.10 200", "end", "wr"]);
            k(rep, st, "RM", &["conf t", "ip route 192.168.2.0 255.255.255.0 10.0.0.5", "ip route 192.168.2.0 255.255.255.0 10.0.0.9 200", "end", "wr"]);
        }
        "v12k1" => { k(rep, st, "RB", &["conf t", "crypto isakmp policy 10", "group 5", "end", "wr"]); }
        "v12k2" => { k(rep, st, "R1", &["conf t", "ip access-list extended NAT-UT", "no 10", "10 deny ip 192.168.1.0 0.0.0.63 192.168.2.0 0.0.0.63", "end", "clear ip nat translation *", "wr"]); }
        "v12n1" => { k(rep, st, "R1", &["conf t", "ip access-list extended MOLNDAL-IN", "10 deny ip 192.168.3.0 0.0.0.63 host 192.168.1.70", "20 permit ip any any", "exit", "interface se0/0/0", "ip access-group MOLNDAL-IN in", "end", "wr"]); }
        "v13k1" => {
            k(rep, st, "RB", &["conf t", "router ospf 1", "no network 10.0.0.0 0.0.0.3 area 1", "network 10.0.0.0 0.0.0.3 area 0", "no network 10.0.0.8 0.0.0.3 area 1", "network 10.0.0.8 0.0.0.3 area 0", "end", "wr"]);
            k(rep, st, "RM", &["conf t", "interface gi0/1", "no ip ospf hello-interval", "no ip ospf dead-interval", "end", "wr"]);
        }
        "v13k2" => { k(rep, st, "R1", &["conf t", "interface se0/0/0", "no bandwidth", "end", "wr"]); }
        "v13n1" => { k(rep, st, "RM", &["conf t", "interface loopback0", "ip address 10.255.0.3 255.255.255.255", "router ospf 1", "network 10.255.0.3 0.0.0.0 area 0", "end", "wr"]); }
        "v14k1" => { k(rep, st, "R1", &["conf t", "router bgp 65010", "no neighbor 203.0.113.1 remote-as 64050", "neighbor 203.0.113.1 remote-as 64500", "end", "wr"]); }
        "v14k2" => { k(rep, st, "R1", &["conf t", "interface gi0/2", "ip nat inside", "end", "wr"]); }
        "v14n1" => { k(rep, st, "R1", &["conf t", "router bgp 65010", "neighbor 198.51.100.1 remote-as 64501", "exit", "interface se0/0/1", "ip nat outside", "exit",
            "access-list 2 permit 192.168.1.0 0.0.0.255", "access-list 2 permit 192.168.2.0 0.0.0.255", "access-list 2 permit 192.168.3.0 0.0.0.63",
            "ip nat inside source list 2 interface serial0/0/1 overload", "ip route 0.0.0.0 0.0.0.0 198.51.100.1 250", "end", "wr"]); }
        _ => rep.fail(format!("ingen lösning för {}", id)),
    }
}

fn matches(rep: &mut Rep, out: &str, needles: &[&str], what: &str) {
    for n in needles {
        if !out.contains(n) {
            rep.fail(format!("{}: saknar \"{}\" i\n{}", what, n, out));
        }
    }
}

#[test]
fn kurs2_veckor() {
    let mut rep = Rep { fails: vec![] };
    let weeks: Vec<Week> = levels::all().into_iter().filter(|w| w.kurs == 2).collect();
    assert_eq!(weeks.iter().map(|w| w.week).collect::<Vec<_>>(), vec![11, 12, 13, 14]);
    for w in &weeks {
        let (mut st, def) = levels::start(w.week);
        let def = def.unwrap();
        println!("Vecka {} – {}", w.week, def.title);
        for t in &def.tasks {
            if (t.check)(&mut st) {
                rep.fail(format!("{} är löst från start", t.id));
            }
        }
        for t in &def.tasks {
            solve_k(&mut rep, &mut st, t.id);
            if !(t.check)(&mut st) {
                rep.fail(format!("{} är inte löst efter lösningen", t.id));
            } else {
                println!("  OK   {} {}", t.id, t.title);
            }
        }
        for h in ["PC-Anna", "PC-Lager", "PC-Molndal"] {
            if !S::ping(&mut st, h, "192.168.1.10", Default::default()).ok {
                rep.fail(format!("vecka {}: {} når inte filservern efter lösningarna", w.week, h));
            }
        }
    }
    rep.done("kurs 2");
}

/// Utdata mot kursrepots facit (labb 03A och 03B)
#[test]
fn kurs2_utdata() {
    let mut rep = Rep { fails: vec![] };
    let mut st = levels::base_state(13);
    let nb = k(&mut rep, &mut st, "RB", &["show ip ospf neighbor"]);
    if !(has_line(&nb, &["1.1.1.1", "FULL"]) && has_line(&nb, &["3.3.3.3", "FULL"])) {
        rep.fail(format!("show ip ospf neighbor på R-Boras-1:\n{}", nb));
    }
    let rt = k(&mut rep, &mut st, "R1", &["show ip route ospf"]);
    matches(&mut rep, &rt, &["192.168.3.0/26 [110/3] via 10.0.0.2"], "show ip route ospf på R-Nordvik-1");
    let br = k(&mut rep, &mut st, "R1", &["show ip ospf interface brief"]);
    if !has_line(&br, &["Se0/0/0", "10.0.0.5/30", "64", "P2P"]) {
        rep.fail(format!("serielänken ska ha kostnad 64:\n{}", br));
    }
    k(&mut rep, &mut st, "R1", &["conf t", "interface gi0/2", "shutdown", "end"]);
    let rt2 = k(&mut rep, &mut st, "R1", &["show ip route ospf"]);
    matches(&mut rep, &rt2, &["192.168.3.0/26 [110/65] via 10.0.0.6,", "Serial0/0/0"], "efter shutdown på Gi0/2");
    k(&mut rep, &mut st, "R1", &["conf t", "interface gi0/2", "no shutdown", "end"]);
    let def = k(&mut rep, &mut st, "RB", &["show ip route"]);
    matches(&mut rep, &def, &["O*E2", "0.0.0.0/0 [110/1] via 10.0.0.1"], "R-Boras-1 default");
    let mut st14 = levels::base_state(14);
    let bgp = k(&mut rep, &mut st14, "R1", &["show ip bgp"]);
    if !has_line(&bgp, &["198.51.100.64/27", "203.0.113.1", "0 64500 64501 i"]) {
        rep.fail(format!("show ip bgp ska visa 198.51.100.64/27 med vägen 64500 64501:\n{}", bgp));
    }
    k(&mut rep, &mut st14, "R1", &["conf t", "router bgp 65010", "neighbor 198.51.100.1 remote-as 64501", "end"]);
    let bgp2 = k(&mut rep, &mut st14, "R1", &["show ip bgp"]);
    if !has_line(&bgp2, &["*>", "198.51.100.64/27", "198.51.100.1", "64501 i"]) {
        rep.fail(format!("med ISP-B ska vägen 64501 vara bäst:\n{}", bgp2));
    }
    let sum = k(&mut rep, &mut st14, "R1", &["show ip bgp summary"]);
    if !has_line(&sum, &["203.0.113.1", "4", "64500"]) {
        rep.fail(format!("show ip bgp summary:\n{}", sum));
    }
    let run = k(&mut rep, &mut st14, "R1", &["show running-config"]);
    matches(&mut rep, &run, &["router ospf 1\n router-id 1.1.1.1", "router bgp 65010\n bgp router-id 1.1.1.1", "clock rate 128000"], "show running-config");
    // Fel AS ger en loggrad som i labbens felfil
    let (mut st14b, _) = levels::start(14);
    let lg = k(&mut rep, &mut st14b, "R1", &["show logging"]);
    matches(&mut rep, &lg, &["%BGP-3-NOTIFICATION: sent to neighbor 203.0.113.1"], "vecka 14-loggen");
    let (mut st13, _) = levels::start(13);
    let lg13 = k(&mut rep, &mut st13, "R1", &["show logging"]);
    matches(&mut rep, &lg13, &["%OSPF-4-ERRRCV: Received invalid packet: mismatched area ID"], "vecka 13-loggen");
    rep.done("kurs 2-utdata");
}

/// Någon rad innehåller alla orden i ordning (ersätter JS-regexarna)
fn has_line(out: &str, parts: &[&str]) -> bool {
    out.lines().any(|l| {
        let mut pos = 0;
        parts.iter().all(|p| match l[pos..].find(p) {
            Some(i) => {
                pos += i + p.len();
                true
            }
            None => false,
        })
    })
}

#[test]
fn utmaningar() {
    use levels::{challenge_done, ChallengeCtx};
    let log = vec![("R1".to_string(), "traceroute 1.2.3.4".to_string()), ("RB".to_string(), "tracert x".to_string()), ("R1".to_string(), "sh ip bgp".to_string()), ("R1".to_string(), "show ip bgp summary".to_string())];
    let g = ChallengeCtx { cmd_log: &log, commands: 4, any_hints: false, elapsed_ms: 1000.0, monitor_first: false, all_reports_perfect: false };
    assert!(challenge_done(5, &g));
    assert!(challenge_done(11, &g));
    assert!(challenge_done(14, &g));
    assert!(!challenge_done(13, &g));
    assert!(challenge_done(1, &g));
    assert!(!challenge_done(7, &g));
}
