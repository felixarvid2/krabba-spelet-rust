//! CCNA-läget (modul 1–8): varje uppgift ska vara olöst från start och lösas av
//! kommandona i ledtrådarna. Visar också att utskrifterna spelaren felsöker med stämmer.
use krabba_spelet::cli::ios::IosSession;
use krabba_spelet::cli::wlc::WlcSession;
use krabba_spelet::cli::Cli;
use krabba_spelet::game::levels::{self, Week};
use krabba_spelet::net::model::StaticIp;
use krabba_spelet::net::sim::{self as S, State};

const BAD: &[&str] = &["Invalid", "Incomplete", "Ambiguous", "rejected", "Bad mask", "overlaps", "Unrecognized"];

struct Rep {
    fails: Vec<String>,
}
impl Rep {
    fn fail(&mut self, m: impl Into<String>) {
        let m = m.into();
        println!("   FEL: {}", m);
        self.fails.push(m);
    }
}

/// Kör IOS-kommandon (privilegierat läge); frågor (t.ex. TFTP) besvaras med `answers`.
fn ios_ans(rep: &mut Rep, st: &mut State, dev: &str, cmds: &[&str], answers: &[&str]) -> String {
    let mut s = IosSession::new(st, dev, "console", true, None);
    let mut all = String::new();
    let mut ans = answers.iter();
    for c in cmds {
        let mut r = s.handle_line(st, c);
        let mut out = r.out.clone();
        for ch in &r.stream {
            out.push_str(&ch.text);
        }
        while s.pending.is_some() {
            r = s.handle_line(st, ans.next().copied().unwrap_or(""));
            out.push_str(&r.out);
        }
        if BAD.iter().any(|b| out.contains(b)) {
            rep.fail(format!("CLI [{}] {}\n{}", dev, c, out));
        }
        all.push_str(&out);
        all.push('\n');
    }
    S::refresh(st);
    all
}
fn ios(rep: &mut Rep, st: &mut State, dev: &str, cmds: &[&str]) -> String {
    ios_ans(rep, st, dev, cmds, &[])
}
fn wlc(st: &mut State, cmds: &[&str]) -> String {
    let mut s = WlcSession::new(st, "WLC");
    s.handle(st, "admin");
    s.handle(st, "x");
    let mut all = String::new();
    for c in cmds {
        let r = s.handle(st, c);
        all.push_str(&r.out);
        all.push('\n');
    }
    S::touch(st);
    S::refresh(st);
    all
}
fn renew(st: &mut State, h: &str) {
    S::dhcp(st, h);
    S::refresh(st);
}
fn tick(st: &mut State, n: usize) {
    for _ in 0..n {
        S::tick(st, 1.0);
    }
}

fn solve(rep: &mut Rep, st: &mut State, id: &str) {
    match id {
        "c1k1" => {
            ios(rep, st, "SW1", &["conf t", "interface gi0/12", "speed auto", "duplex auto", "end", "wr"]);
        }
        "c1k2" => {
            let l = st.devices.get_mut("PC-Lisa").unwrap();
            let nic = l.nic.as_mut().unwrap();
            nic.static_ = Some(StaticIp { ip: "192.168.1.21".into(), mask: "255.255.255.192".into(), gw: Some("192.168.1.1".into()), dns: vec!["192.168.1.1".into()] });
            S::touch(st);
            S::refresh(st);
        }
        "c1n1" => {
            ios(rep, st, "R1", &["conf t", "ipv6 unicast-routing", "interface gi0/0.10", "ipv6 address 2001:DB8:ACAD:10::1/64", "end", "wr"]);
        }
        "c2k1" => {
            ios(rep, st, "SW2", &["conf t", "vlan 20", "name EKONOMI", "end", "wr"]);
        }
        "c2k2" => {
            ios(rep, st, "SW2", &["conf t", "interface range gi0/24 - 25", "channel-group 1 mode active", "end", "wr"]);
        }
        "c2n1" => {
            ios(rep, st, "SW1", &["conf t", "lldp run", "end"]);
            ios(rep, st, "SW2", &["conf t", "lldp run", "end"]);
        }
        "c3k1" => {
            ios(rep, st, "SW2", &["conf t", "no spanning-tree vlan 10 priority 4096", "end", "wr"]);
        }
        "c3k2" => {
            ios(rep, st, "SW1", &["conf t", "interface gi0/6", "shutdown", "no shutdown", "end"]);
            renew(st, "PC-Karim");
        }
        "c3n1" => {
            ios(rep, st, "SW1", &["conf t", "spanning-tree mode rapid-pvst", "end"]);
            ios(rep, st, "SW2", &["conf t", "spanning-tree mode rapid-pvst", "end"]);
        }
        "c3n2" => {
            ios(rep, st, "SW1", &["conf t", "spanning-tree portfast bpduguard default", "end"]);
        }
        "c4k1" => {
            ios(rep, st, "R1", &["conf t", "no ip route 192.168.2.0 255.255.255.192 203.0.113.1", "end"]);
        }
        "c4k2" => {
            ios(rep, st, "R1", &["conf t", "no ip route 0.0.0.0 0.0.0.0 10.0.0.2", "ip route 0.0.0.0 0.0.0.0 203.0.113.1", "end"]);
        }
        "c4n1" => {
            for f in levels::look_flags("R1", "sh ip ro") {
                st.flags.misc.insert(f.into(), true);
            }
        }
        "c5k1" => {
            ios(rep, st, "R1", &["conf t", "router ospf 1", "no passive-interface Serial0/0/0", "end"]);
        }
        "c5k2" => {
            ios(rep, st, "RB", &["conf t", "router ospf 1", "router-id 2.2.2.2", "end"]);
        }
        "c5n1" => {
            ios(rep, st, "R1", &["conf t", "interface gi0/2", "ip ospf network point-to-point", "end"]);
            ios(rep, st, "RB", &["conf t", "interface gi0/2", "ip ospf network point-to-point", "end"]);
        }
        "c6k1" => {
            ios(rep, st, "R1", &["conf t", "no ip nat inside source static 192.168.1.11 203.0.113.11", "ip nat inside source static 192.168.1.10 203.0.113.11", "end"]);
        }
        "c6k2" => {
            ios(rep, st, "SW1", &["conf t", "logging host 192.168.1.17", "logging trap informational", "end"]);
        }
        "c6n1" => {
            let out = ios_ans(rep, st, "R1", &["copy running-config tftp:"], &["192.168.1.10", ""]);
            if !out.contains("bytes copied") {
                rep.fail(format!("TFTP-kopian misslyckades:\n{}", out));
            }
        }
        "c7k1" => {
            ios(rep, st, "SW1", &["conf t", "interface gi0/6", "no switchport port-security mac-address sticky 001a.2b3c.4d5e", "shutdown", "no shutdown", "end"]);
            renew(st, "PC-Karim");
        }
        "c7k2" => {
            ios(rep, st, "R1", &["conf t", "ip access-list standard VTY-SKYDD", "no 10", "10 permit 192.168.1.192 0.0.0.63", "end"]);
        }
        "c7k3" => {
            ios(rep, st, "R1", &["conf t", "no snmp-server community public", "snmp-server community nordvik-ro RO", "end"]);
        }
        "c7n1" => {
            ios(rep, st, "SW1", &["conf t", "ip dhcp snooping", "ip dhcp snooping vlan 10,20,30", "interface gi0/1", "ip dhcp snooping trust", "end"]);
        }
        "c8k1" => {
            let out = wlc(st, &["config wlan disable 2", "config wlan security wpa2-psk 2", "config wlan enable 2", "save config", "y"]);
            println!("   WLC: {}", out.trim());
        }
        "c8k2" => {
            wlc(st, &["config 802.11b channel ap AP-Lager-2 6"]);
        }
        "c8n1" => {
            for f in levels::look_flags("Tekniker", "curl http://192.168.1.196/api/v1/aps") {
                st.flags.misc.insert(f.into(), true);
            }
        }
        _ => rep.fail(format!("ingen lösning för {}", id)),
    }
}

#[test]
fn ccna_moduler() {
    let mut rep = Rep { fails: vec![] };
    let weeks: Vec<Week> = levels::all().into_iter().filter(|w| w.kurs == 3).collect();
    assert_eq!(weeks.iter().map(|w| w.week).collect::<Vec<_>>(), (201..=208).collect::<Vec<_>>());
    for w in &weeks {
        assert!(levels::is_ccna(w.week) && !levels::is_build(w.week));
        assert_eq!(levels::name(Some(w)), format!("CCNA {}", w.week - 200));
        assert!(w.quiz.len() >= 10, "{}: för få frågor", w.week);
        let (mut st, def) = levels::start(w.week);
        let def = def.unwrap();
        println!("CCNA {} – {}", w.week - 200, def.title);
        for t in &def.tasks {
            if (t.check)(&mut st) {
                rep.fail(format!("{} är löst från start", t.id));
            }
            if !t.build && t.report.is_none() {
                rep.fail(format!("{} saknar felrapport", t.id));
            }
            let v = levels::verify(t.id);
            if v.text.starts_with("Bevisa att felet är löst med ett ping från laptopen") {
                rep.fail(format!("{} saknar bevis för svårt läge", t.id));
            }
        }
        for t in &def.tasks {
            solve(&mut rep, &mut st, t.id);
            tick(&mut st, 2);
            if (t.check)(&mut st) {
                println!("  OK   {} {}", t.id, t.title);
            } else {
                rep.fail(format!("{} är inte löst efter lösningen", t.id));
            }
        }
        // Lösningarna får inte förstöra resten av nätet
        for h in ["PC-Anna", "PC-Lager"] {
            if !S::ping(&mut st, h, "192.168.1.10", Default::default()).ok {
                rep.fail(format!("CCNA {}: {} når inte filservern efter lösningarna", w.week - 200, h));
            }
        }
    }
    assert!(rep.fails.is_empty(), "{} fel i CCNA-läget:\n{}", rep.fails.len(), rep.fails.join("\n"));
}

/// Symptomen syns i de kommandon som ledtrådarna pekar på
#[test]
fn ccna_symptom() {
    let mut rep = Rep { fails: vec![] };
    let mut chk = |out: &str, needle: &str, what: &str, rep: &mut Rep| {
        if !out.contains(needle) {
            rep.fail(format!("{}: saknar \"{}\" i\n{}", what, needle, out));
        }
    };
    let (mut st, _) = levels::start(202);
    let o = ios(&mut rep, &mut st, "SW1", &["show etherchannel summary"]);
    chk(&o, "Gi0/24(I)", "etherchannel SW1", &mut rep);
    let (mut st, _) = levels::start(203);
    let o = ios(&mut rep, &mut st, "SW2", &["show spanning-tree vlan 10"]);
    chk(&o, "This bridge is the root", "rot på SW2", &mut rep);
    let o = ios(&mut rep, &mut st, "SW1", &["show interfaces status"]);
    chk(&o, "err-disabled", "bpduguard", &mut rep);
    let o = ios(&mut rep, &mut st, "SW1", &["conf t", "spanning-tree mode rapid-pvst", "end", "show spanning-tree vlan 10"]);
    chk(&o, "rstp", "rapid-pvst", &mut rep);
    let (mut st, _) = levels::start(205);
    let o = ios(&mut rep, &mut st, "RB", &["show logging"]);
    chk(&o, "DUP_RTRID", "dubblerat router-ID", &mut rep);
    let o = ios(&mut rep, &mut st, "R1", &["conf t", "interface gi0/2", "ip ospf network point-to-point", "end", "show ip ospf interface gi0/2"]);
    chk(&o, "POINT_TO_POINT", "ospf p2p", &mut rep);
    let (mut st, _) = levels::start(206);
    let o = ios(&mut rep, &mut st, "SW1", &["show logging"]);
    chk(&o, "level critical", "logging trap", &mut rep);
    let (mut st, _) = levels::start(207);
    let o = ios(&mut rep, &mut st, "SW1", &["show interfaces status"]);
    chk(&o, "err-disabled", "port security", &mut rep);
    let (mut st, _) = levels::start(208);
    let o = wlc(&mut st, &["show wlan summary"]);
    chk(&o, "Open", "wlan security", &mut rep);
    assert!(rep.fails.is_empty(), "{} fel:\n{}", rep.fails.len(), rep.fails.join("\n"));
}
