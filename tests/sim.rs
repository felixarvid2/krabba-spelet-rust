//! Tester för simulatorn (port av js/test/smoke.js och en del av levels.js baseState).
use krabba_spelet::net::model::*;
use krabba_spelet::net::sim::{self, PingOpts, State};

const DHCP_HOSTS: [&str; 10] = ["PC-Anna", "PC-Karim", "PC-Bo", "PC-Maja", "PC-Sara", "PC-Lisa", "Gast-laptop", "PC-Lager", "Gast-mobil", "Lager-skanner"];

fn base_state(week: u32) -> State {
    let mut st = State::new(build_golden(week));
    st.dev_mut("Gast-mobil").ssid = Some("Nordvik-Gast".into());
    st.dev_mut("Lager-skanner").ssid = Some("Nordvik-Lager".into());
    sim::refresh(&mut st);
    let mut hosts: Vec<&str> = DHCP_HOSTS.to_vec();
    if st.devices.contains_key("PC-Molndal") {
        hosts.push("PC-Molndal");
    }
    for h in hosts {
        sim::dhcp(&mut st, h);
    }
    sim::refresh(&mut st);
    st
}

fn ping_ttl(st: &mut State, a: &str, b: &str) -> Result<i64, String> {
    let r = sim::ping(st, a, b, PingOpts::default());
    if r.ok { Ok(r.ttl) } else { Err(format!("{} {}", r.reason.unwrap_or_default(), r.where_.unwrap_or_default())) }
}

#[test]
fn smoke_dhcp_ping_nat_dns_stp() {
    let mut s = State::new(build_golden(1));
    sim::refresh(&mut s);
    let expect = [
        ("PC-Anna", "192.168.1.20", "192.168.1.1"),
        ("PC-Karim", "192.168.1.21", "192.168.1.1"),
        ("PC-Bo", "192.168.1.80", "192.168.1.65"),
        ("PC-Maja", "192.168.1.81", "192.168.1.65"),
        ("PC-Sara", "192.168.1.22", "192.168.1.1"),
        ("PC-Lisa", "192.168.1.23", "192.168.1.1"),
        ("Gast-laptop", "192.168.1.140", "192.168.1.129"),
        ("PC-Lager", "192.168.2.20", "192.168.2.1"),
    ];
    for (h, ip, gw) in expect {
        let l = sim::dhcp(&mut s, h);
        assert_eq!(l.ip, ip, "{}", h);
        assert_eq!(l.gw.as_deref(), Some(gw), "{}", h);
        assert!(!l.apipa);
    }
    s.dev_mut("Gast-mobil").ssid = Some("Nordvik-Gast".into());
    s.dev_mut("Lager-skanner").ssid = Some("Nordvik-Lager".into());
    sim::touch(&mut s);
    let d = sim::get(&mut s);
    assert_eq!(d.ap_joined.len(), 3);
    assert!(d.ap_joined.values().all(|j| *j));
    assert_eq!(sim::dhcp(&mut s, "Gast-mobil").ip, "192.168.2.70");
    assert_eq!(sim::dhcp(&mut s, "Lager-skanner").ip, "192.168.2.21");

    assert_eq!(ping_ttl(&mut s, "PC-Anna", "192.168.1.10"), Ok(128));
    assert_eq!(ping_ttl(&mut s, "PC-Anna", "192.168.1.70"), Ok(127));
    assert_eq!(ping_ttl(&mut s, "PC-Anna", "198.51.100.80"), Ok(54));
    assert_eq!(ping_ttl(&mut s, "PC-Anna", "9.9.9.9"), Ok(54));
    assert_eq!(ping_ttl(&mut s, "PC-Lisa", "192.168.1.11"), Ok(128));
    assert_eq!(ping_ttl(&mut s, "PC-Lager", "192.168.1.10"), Ok(126));
    assert_eq!(ping_ttl(&mut s, "PC-Lager", "198.51.100.80"), Ok(53));
    assert_eq!(ping_ttl(&mut s, "Gast-laptop", "192.168.1.10"), Err("acl R1".into()));
    assert_eq!(ping_ttl(&mut s, "Gast-laptop", "198.51.100.80"), Ok(54));
    assert_eq!(ping_ttl(&mut s, "Gast-mobil", "198.51.100.80"), Ok(53));
    assert_eq!(ping_ttl(&mut s, "Gast-mobil", "192.168.2.20"), Err("acl RB".into()));
    assert_eq!(ping_ttl(&mut s, "Tekniker", "192.168.2.193"), Ok(254));
    assert_eq!(ping_ttl(&mut s, "R1", "10.0.0.2"), Ok(255));
    assert_eq!(ping_ttl(&mut s, "SW1", "192.168.1.193"), Ok(255));

    // DNS
    let r = sim::resolve(&mut s, "PC-Anna", "filserver");
    assert_eq!((r.ip.as_deref(), r.server.as_deref()), (Some("192.168.1.10"), Some("192.168.1.1")));
    let r = sim::resolve(&mut s, "PC-Anna", "www.example.com");
    assert_eq!((r.ip.as_deref(), r.server.as_deref()), (Some("198.51.100.80"), Some("192.168.1.1")));
    let r = sim::resolve(&mut s, "Gast-laptop", "www.example.com");
    assert_eq!((r.ip.as_deref(), r.server.as_deref()), (Some("198.51.100.80"), Some("9.9.9.9")));

    // NAT och NTP
    assert_eq!(s.dev("R1").rt.nat_trans.len(), 6);
    assert!(sim::ntp_synced(&mut s, "SW1"));

    // STP: SW2 har SW1 som rot, Gi0/25 blockeras
    let d = sim::get(&mut s);
    let sw2 = &d.stp["SW2"];
    for v in [10u16, 20, 30, 99] {
        assert_eq!(sw2[&v].root, "SW1");
        assert!(!sw2[&v].is_root);
        assert_eq!(sw2[&v].root_port.as_deref(), Some("GigabitEthernet0/24"));
        assert_eq!(d.blocked.get(&format!("SW2|GigabitEthernet0/25#{}", v)).map(|s| s.as_str()), Some("BLK"));
    }
    assert_eq!(d.blocked.len(), 4);
}

#[test]
fn golden_week0_baseline() {
    let mut st = base_state(0);
    assert!(sim::get(&mut st).ap_joined.values().all(|j| *j));
    assert!(ping_ttl(&mut st, "PC-Anna", "192.168.1.10").is_ok());
    assert!(ping_ttl(&mut st, "PC-Anna", "198.51.100.80").is_ok());
    assert!(ping_ttl(&mut st, "PC-Lager", "192.168.1.10").is_ok());
    let r = sim::resolve(&mut st, "PC-Maja", "ekonomi");
    assert_eq!(r.ip.as_deref(), Some("192.168.1.70"), "{:?}", r);
    // Trådlösa klienter fick adress via AP:erna
    assert_eq!(st.dev("Gast-mobil").lease.as_ref().unwrap().ip, "192.168.2.70");
    assert_eq!(st.dev("Lager-skanner").lease.as_ref().unwrap().ip, "192.168.2.21");
    // Loggar fylls på och blir högst 400
    for _ in 0..450 {
        sim::push_log(&mut st, "R1", "test");
    }
    assert_eq!(st.dev("R1").rt.logs.len(), 400);
    assert!(!st.log_events.is_empty());
}

#[test]
fn tick_and_counters() {
    let mut st = base_state(0);
    sim::tick(&mut st, 30.0);
    sim::tick(&mut st, 40.0);
    let c = &st.dev("SW1").rt.counters["GigabitEthernet0/5"];
    assert!(c.inp > 0.0);
    assert!(sim::link_load(&mut st, "SW1", "GigabitEthernet0/5") > 0.0);
}

#[test]
fn week13_ospf() {
    let mut st = base_state(13);
    let d = sim::get(&mut st);
    let o = &d.dynr.ospf;
    assert_eq!(o.len(), 3);
    let nb = |r: &str| o[r].nbrs.iter().map(|n| format!("{}/{}/{}/{}", n.dev, n.rid, n.iface, n.role)).collect::<Vec<_>>().join(",");
    assert_eq!(nb("R1"), "RB/2.2.2.2/GigabitEthernet0/2/DR,RM/3.3.3.3/Serial0/0/0/-");
    assert_eq!(nb("RB"), "RM/3.3.3.3/GigabitEthernet0/1/DR,R1/1.1.1.1/GigabitEthernet0/2/BDR");
    assert_eq!(nb("RM"), "RB/2.2.2.2/GigabitEthernet0/1/BDR,R1/1.1.1.1/Serial0/0/0/-");
    for r in ["R1", "RB", "RM"] {
        assert!(o[r].problems.is_empty());
    }
    let r1 = &d.dynr.routes["R1"];
    assert_eq!(r1.len(), 5);
    assert!(r1.iter().all(|r| r.kind == "O"));
    let m = r1.iter().find(|r| r.net == "192.168.3.0").unwrap();
    assert_eq!((m.nh.as_str(), m.metric), ("10.0.0.2", 3));
    assert!(d.dynr.routes["RB"].iter().any(|r| r.kind == "O*E2" && r.net == "0.0.0.0"));
    // Installerade rutter innehåller O
    let ir = sim::installed_routes(&st, &d, "R1");
    assert!(ir.iter().any(|r| r.kind == "O" && r.net == "192.168.2.0"));
    assert!(ir.iter().any(|r| r.kind == "C"));
    assert_eq!(ping_ttl(&mut st, "PC-Anna", "192.168.1.10"), Ok(128));
    assert_eq!(ping_ttl(&mut st, "PC-Anna", "198.51.100.80"), Ok(54));
    assert_eq!(ping_ttl(&mut st, "PC-Lager", "192.168.1.10"), Ok(126));
    assert_eq!(ping_ttl(&mut st, "PC-Molndal", "192.168.1.10"), Ok(125));
    let r = sim::resolve(&mut st, "PC-Maja", "ekonomi");
    assert_eq!(r.ip.as_deref(), Some("192.168.1.70"));
}

#[test]
fn week14_bgp() {
    let mut st = base_state(14);
    let d = sim::get(&mut st);
    let b = &d.dynr.bgp["R1"];
    assert_eq!(b.peers.len(), 1);
    assert_eq!(b.peers[0].state, "Established");
    assert!(d.dynr.routes["R1"].iter().any(|r| r.kind == "B" && r.net == "198.51.100.64"));
}

#[test]
fn week10_ipsec() {
    let mut st = base_state(10);
    assert!(ping_ttl(&mut st, "PC-Lager", "192.168.1.10").is_ok());
    let cs = sim::crypto_status(&mut st, "R1");
    assert_eq!(cs.isakmp.len(), 1);
    assert!(cs.sas.iter().any(|s| s.up));
}

#[test]
fn week10_lb() {
    let mut st = base_state(10);
    let h = sim::http_get(&mut st, "PC-Anna", "192.168.1.13", 80);
    assert!(h.ok && h.code == 200, "{:?}", h.res);
    assert!(h.member.is_some());
    let s = sim::lb_status(&mut st, "LB");
    assert_eq!(s[0].members.len(), 2);
    assert!(s[0].members.iter().all(|m| m.up));
}
