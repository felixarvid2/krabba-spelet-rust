//! Svårt läge: bevis för alla uppgifter, matchning och att beviskommandona ger meningsfull utdata.
use krabba_spelet::cli::host::HostSession;
use krabba_spelet::cli::ios::IosSession;
use krabba_spelet::cli::lb::LbSession;
use krabba_spelet::cli::wlc::WlcSession;
use krabba_spelet::cli::Cli;
use krabba_spelet::game::levels::{self, verify, verify_matches, Verify};
use krabba_spelet::net::model::{LinkState, StaticIp};
use krabba_spelet::net::sim::{self as S, State};

fn full(r: krabba_spelet::cli::CmdResult) -> String {
    let mut o = r.out;
    for c in r.stream {
        o.push_str(&c.text);
    }
    o
}

/// Kör en rad på en enhet med rätt sorts session.
fn run_on(st: &mut State, dev: &str, line: &str) -> String {
    if st.dev(dev).is_ios() {
        let mut s = IosSession::new(st, dev, "console", true, None);
        let mut r = s.handle_line(st, line);
        let mut o = full(r.clone());
        while s.pending.is_some() {
            r = s.handle_line(st, "");
            o.push_str(&full(r.clone()));
        }
        o
    } else if dev == "WLC" {
        let mut s = WlcSession::new(st, dev);
        s.handle(st, "admin");
        s.handle(st, "x");
        full(s.handle(st, line))
    } else if dev == "LB" {
        let mut s = LbSession::new(st, dev);
        s.handle(st, "admin");
        s.handle(st, "x");
        full(s.handle(st, line))
    } else {
        let mut s = HostSession::new(st, dev);
        full(s.handle(st, line))
    }
}

fn conf(st: &mut State, dev: &str, cmds: &[&str]) {
    let mut s = IosSession::new(st, dev, "console", true, None);
    for c in cmds {
        let mut r = s.handle_line(st, c);
        let mut o = full(r.clone());
        while s.pending.is_some() {
            r = s.handle_line(st, "");
            o.push_str(&full(r.clone()));
        }
        assert!(!o.contains("Invalid") && !o.contains("Incomplete") && !o.contains("Ambiguous") && !o.contains("rejected"), "[{}] {} -> {}", dev, c, o);
    }
    S::refresh(st);
}
fn tick(st: &mut State, n: usize) {
    for _ in 0..n {
        S::tick(st, 1.0);
    }
}
fn renew(st: &mut State, h: &str) {
    S::dhcp(st, h);
    S::refresh(st);
}
fn ping_ok(o: &str) -> bool {
    o.contains("TTL=")
}

fn check_proof(st: &mut State, id: &str, dev: &str, line: &str) -> String {
    let v = verify(id);
    assert!(verify_matches(&v, dev, line), "{} ska godkänna {} på {}", id, line, dev);
    let o = run_on(st, dev, line);
    println!("--- {} [{}] {}\n{}", id, dev, line, o);
    o
}

#[test]
fn alla_uppgifter_har_bevis() {
    for w in levels::all() {
        let (st, _) = levels::start(w.week);
        for t in &w.tasks {
            let v = verify(t.id);
            assert!(!v.on.is_empty() && !v.cmds.is_empty(), "{} saknar bevis", t.id);
            assert!(v.text.starts_with("Bevisa"), "{}: {}", t.id, v.text);
            for d in &v.on {
                assert!(st.devices.contains_key(*d), "{}: enheten {} finns inte i vecka {}", t.id, d, w.week);
            }
            // varje kommando måste vara giltigt och godkännas av sin egen matchning
            for c in &v.cmds {
                for d in &v.on {
                    assert!(verify_matches(&v, d, c), "{}: {} ska matcha sig själv på {}", t.id, c, d);
                }
            }
        }
    }
}

#[test]
fn beviskommandon_ger_ingen_felutskrift() {
    let bad = ["Invalid input", "Unrecognized", "command not found", "Incorrect usage", "Okänt", "Ambiguous", "Incomplete command"];
    for w in levels::all() {
        let (mut st, _) = levels::start(w.week);
        tick(&mut st, 3);
        for t in &w.tasks {
            let v = verify(t.id);
            // varje kommando ska fungera på minst en av enheterna (t.ex. "show line" på SW1 men inte på laptopen)
            for c in &v.cmds {
                if ["ssh", "telnet", "screen"].iter().any(|p| c.starts_with(p)) {
                    continue;
                }
                let mut last = String::new();
                let ok = v.on.iter().any(|d| {
                    let o = run_on(&mut st, d, c);
                    let good = !bad.iter().any(|b| o.contains(b));
                    if !good {
                        last = format!("[{}] {}", d, o);
                    }
                    good
                });
                assert!(ok, "vecka {} {}: {} fungerar inte på någon enhet\n{}", w.week, t.id, c, last);
            }
        }
    }
}

fn m(v: &Verify, dev: &str, line: &str) -> bool {
    verify_matches(v, dev, line)
}

#[test]
fn matchning() {
    let v = Verify { on: vec!["SW2", "PC-Anna"], cmds: vec!["show interfaces gi0/24", "show running-config | include speed", "ping 192.168.1.10", "ping filserver"], text: "Bevisa: x" };
    assert!(m(&v, "SW2", "show interfaces gi0/24"));
    assert!(m(&v, "SW2", "sh int gi0/24"));
    assert!(m(&v, "SW2", "  SH   INT  Gi0/24 "));
    assert!(m(&v, "SW2", "sh int GigabitEthernet0/24"));
    assert!(m(&v, "SW2", "show interfaces g0/24"));
    assert!(!m(&v, "SW2", "show interfaces gi0/23"));
    assert!(!m(&v, "SW1", "show interfaces gi0/24"));
    assert!(m(&v, "SW2", "sh run | i speed"));
    assert!(m(&v, "SW2", "show run |include speed"));
    assert!(m(&v, "SW2", "do sh int gi0/24"));
    assert!(!m(&v, "SW2", "sh int status"));
    assert!(m(&v, "PC-Anna", "ping 192.168.1.10"));
    assert!(m(&v, "PC-Anna", "PING 192.168.1.10"));
    assert!(m(&v, "PC-Anna", "ping -n 4 192.168.1.10"));
    assert!(m(&v, "PC-Anna", "ping 192.168.1.10 -n 10"));
    assert!(m(&v, "PC-Anna", "ping -t 192.168.1.10"));
    assert!(m(&v, "PC-Anna", "ping -c 3 192.168.1.10"));
    assert!(m(&v, "PC-Anna", "ping filserver"));
    assert!(m(&v, "PC-Anna", "ping  FilServer"));
    assert!(!m(&v, "PC-Anna", "ping 192.168.1.1"));
    assert!(!m(&v, "PC-Anna", "ping 192.168.1.100"));
    assert!(!m(&v, "PC-Anna", "ping"));
    let t = Verify { on: vec!["Tekniker"], cmds: vec!["screen /dev/ttyUSB0 9600", "curl http://tid/"], text: "Bevisa: x" };
    assert!(m(&t, "Tekniker", "sudo screen /dev/ttyUSB0 9600"));
    assert!(!m(&t, "Tekniker", "sudo screen /dev/ttyUSB0 19200"));
    assert!(m(&t, "Tekniker", "curl -s http://tid/"));
    assert!(!m(&t, "PC-Anna", "curl http://tid/"));
}

fn karim_ok(st: &mut State) {
    let l = st.links.iter_mut().find(|l| l.b.dev == "PC-Karim").unwrap();
    l.state = LinkState::Ok;
    S::touch(st);
}

#[test]
fn bevis_efter_rattning_vecka_1_till_3() {
    // Vecka 1
    let (mut st, _) = levels::start(1);
    tick(&mut st, 3);
    conf(&mut st, "SW1", &["conf t", "line con 0", "speed 9600", "end", "write memory"]);
    let o = check_proof(&mut st, "v1k1", "SW1", "sh run | i speed");
    assert!(!o.contains("19200"));
    let o = check_proof(&mut st, "v1k1", "SW1", "show startup-config | section line con");
    assert!(o.contains("line con 0") && !o.contains("19200"), "{}", o);
    conf(&mut st, "SW1", &["conf t", "int gi0/5", "no shut", "end", "wr"]);
    renew(&mut st, "PC-Anna");
    tick(&mut st, 2);
    assert!(ping_ok(&check_proof(&mut st, "v1k2", "PC-Anna", "ping -n 4 192.168.1.10")));
    conf(&mut st, "SW2", &["conf t", "hostname SW-Nordvik-2", "end", "copy running-config startup-config"]);
    assert!(check_proof(&mut st, "v1n1", "SW2", "sh start").contains("hostname SW-Nordvik-2"));

    // Vecka 2
    let (mut st, _) = levels::start(2);
    tick(&mut st, 3);
    karim_ok(&mut st);
    renew(&mut st, "PC-Karim");
    assert!(ping_ok(&check_proof(&mut st, "v2k1", "PC-Karim", "ping 192.168.1.10")));
    conf(&mut st, "SW2", &["conf t", "int gi0/24", "speed auto", "duplex auto", "end", "wr"]);
    tick(&mut st, 2);
    let o = check_proof(&mut st, "v2k2", "SW2", "sh int gi0/24");
    assert!(o.contains("Full-duplex") || o.contains("full duplex") || o.contains("Full"), "{}", o);
    conf(&mut st, "SW1", &["conf t", "int gi0/7", "sw acc vlan 20", "end", "wr"]);
    renew(&mut st, "PC-Bo");
    assert!(ping_ok(&check_proof(&mut st, "v2n1", "PC-Bo", "ping 192.168.1.70")));

    // Vecka 3
    let (mut st, _) = levels::start(3);
    tick(&mut st, 3);
    {
        let a = st.devices.get_mut("PC-Anna").unwrap();
        let nic = a.nic.as_mut().unwrap();
        nic.dhcp = false;
        nic.static_ = Some(StaticIp { ip: "192.168.1.19".into(), mask: "255.255.255.192".into(), gw: Some("192.168.1.1".into()), dns: vec!["192.168.1.1".into()] });
        a.lease = None;
        S::touch(&mut st);
        S::refresh(&mut st);
    }
    assert!(ping_ok(&check_proof(&mut st, "v3k1", "PC-Anna", "ping 192.168.1.70")));
    conf(&mut st, "R1", &["conf t", "ip dhcp pool KONTOR", "network 192.168.1.0 255.255.255.192", "end", "wr"]);
    renew(&mut st, "PC-Karim");
    assert!(ping_ok(&check_proof(&mut st, "v3k2", "PC-Karim", "ping 192.168.1.10")));
    conf(&mut st, "R1", &["conf t", "ip dhcp pool EKONOMI", "dns-server 192.168.1.65", "end"]);
    renew(&mut st, "PC-Maja");
    let o = check_proof(&mut st, "v3n1", "PC-Maja", "nslookup ekonomi");
    assert!(o.contains("192.168.1.70"), "{}", o);
    assert!(ping_ok(&check_proof(&mut st, "v3n1", "PC-Maja", "ping ekonomi")));
}

#[test]
fn bevis_efter_rattning_vecka_5_och_10() {
    let (mut st, _) = levels::start(5);
    tick(&mut st, 3);
    conf(&mut st, "RB", &["conf t", "ip route 0.0.0.0 0.0.0.0 10.0.0.1", "end"]);
    tick(&mut st, 2);
    assert!(ping_ok(&check_proof(&mut st, "v5k1", "PC-Lager", "ping 192.168.1.10")));
    conf(&mut st, "R1", &["conf t", "no ip route 0.0.0.0 0.0.0.0 203.0.113.9", "ip route 0.0.0.0 0.0.0.0 203.0.113.1", "end"]);
    tick(&mut st, 2);
    assert!(ping_ok(&check_proof(&mut st, "v5k2", "PC-Anna", "ping 203.0.113.1")));

    let (mut st, _) = levels::start(10);
    tick(&mut st, 3);
    conf(&mut st, "RB", &["conf t", "ip access-list extended VPN-TRAFIK", "no 10", "10 permit ip 192.168.2.0 0.0.0.63 192.168.1.0 0.0.0.63", "end"]);
    conf(&mut st, "R1", &["conf t", "ip access-list extended NAT-UT", "5 deny ip 192.168.1.0 0.0.0.63 192.168.2.0 0.0.0.63", "end"]);
    tick(&mut st, 2);
    assert!(ping_ok(&check_proof(&mut st, "v10k1", "PC-Lager", "ping 192.168.1.10")));
    let o = check_proof(&mut st, "v10k1", "RB", "sh crypto ipsec sa");
    assert!(o.contains("encaps"), "{}", o);
    conf(&mut st, "RB", &["conf t", "int gi0/1", "ip tcp adjust-mss 1360", "end", "wr"]);
    assert!(check_proof(&mut st, "v10k2", "RB", "sh run int gi0/1").contains("adjust-mss 1360"));
    let mut s = LbSession::new(&st, "LB");
    s.handle(&mut st, "admin");
    s.handle(&mut st, "x");
    s.handle(&mut st, "config pool TID-POOL monitor http");
    let o = check_proof(&mut st, "v10n1", "LB", "show pool");
    assert!(o.contains("tid-1") && o.contains("tid-2"), "{}", o);
    let o = check_proof(&mut st, "v10n1", "Tekniker", "curl http://tid/");
    assert!(!o.is_empty());
}

#[test]
fn bevis_efter_bygge_2() {
    let (mut st, _) = levels::start(102);
    tick(&mut st, 3);
    conf(&mut st, "R1", &["conf t", "interface gi0/0", "no shutdown", "interface gi0/0.10", "encapsulation dot1Q 10", "ip address 192.168.1.1 255.255.255.192",
        "interface gi0/0.20", "encapsulation dot1Q 20", "ip address 192.168.1.65 255.255.255.192", "interface gi0/0.30", "encapsulation dot1Q 30", "ip address 192.168.1.129 255.255.255.192",
        "interface gi0/0.99", "encapsulation dot1Q 99", "ip address 192.168.1.193 255.255.255.192", "end"]);
    tick(&mut st, 2);
    assert!(check_proof(&mut st, "b2a", "Tekniker", "ping -c 3 192.168.1.193").contains("0% packet loss"));
}
