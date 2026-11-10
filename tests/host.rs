//! Tester för värdterminalerna mot den riktiga simulatorn.
use krabba_spelet::cli::host::HostSession;
use krabba_spelet::cli::{Cli, PushReq};
use krabba_spelet::net::model::build_golden;
use krabba_spelet::net::sim::{self, State};

fn mk() -> State {
    State::new(build_golden(10))
}

fn out(r: krabba_spelet::cli::CmdResult) -> String {
    let mut s = r.out;
    for c in r.stream {
        s.push_str(&c.text);
    }
    s
}

#[test]
fn ipconfig_hos_pc_anna() {
    let mut st = mk();
    let mut s = HostSession::new(&st, "PC-Anna");
    sim::dhcp(&mut st, "PC-Anna");
    let o = out(s.handle(&mut st, "ipconfig /all"));
    println!("{}", o);
    assert!(o.contains("Windows IP-konfiguration"));
    assert!(o.contains("IPv4-adress"));
    assert!(o.contains("DHCP-server"));
}

#[test]
fn ping_och_nslookup_och_tracert() {
    let mut st = mk();
    let mut s = HostSession::new(&st, "PC-Anna");
    sim::dhcp(&mut st, "PC-Anna");
    let o = out(s.handle(&mut st, "ping 192.168.1.10"));
    println!("{}", o);
    assert!(o.contains("Ping-statistik för 192.168.1.10"));
    let o = out(s.handle(&mut st, "nslookup ekonomi"));
    println!("{}", o);
    assert!(o.contains("Server:"));
    let o = out(s.handle(&mut st, "tracert 192.168.1.10"));
    println!("{}", o);
    let o = out(s.handle(&mut st, "arp -a"));
    println!("{}", o);
    let o = out(s.handle(&mut st, "ipconfig /displaydns"));
    println!("{}", o);
    let o = out(s.handle(&mut st, "route print | findstr 0.0.0.0"));
    println!("{}", o);
    let o = out(s.handle(&mut st, "curl http://tid/"));
    println!("{}", o);
    let o = out(s.handle(&mut st, "copy \\\\filserver\\delat\\x.pdf ."));
    println!("{}", o);
    let o = out(s.handle(&mut st, "pathping 192.168.1.10"));
    println!("{}", o);
}

#[test]
fn linux_kommandon() {
    let mut st = mk();
    let mut s = HostSession::new(&st, "Tekniker");
    for c in [
        "ip -4 addr show", "ip route show default", "ip -4 neigh show", "ping -c 3 192.168.1.1", "traceroute 192.168.1.10",
        "tracepath 192.168.1.10", "nslookup filserver", "dig +short filserver", "ping -s 1400 -M do 192.168.1.10",
        "curl http://tid/", "for i in 1 2 3 4; do curl -s http://tid/; done", "nmap -sn 192.168.1.192/28",
        "nmap -p 22,23,80 192.168.1.193", "mtr -r 192.168.1.10", "ethtool enp0s31f6", "nmcli device status", "iperf3 -c 192.168.1.10",
        "tcpdump -c 4", "arping -c 3 192.168.1.1", "ss -tuln", "ip -br a", "dig -x 192.168.1.10", "wget http://tid/", "cat index.html",
        "ls", "history", "nc -zv 192.168.1.10 445", "resolvectl status", "w", "ps",
    ] {
        let o = out(s.handle(&mut st, c));
        println!("$ {}\n{}\n", c, o);
        assert!(!o.contains("command not found"), "{}", c);
    }
}

#[test]
fn ssh_flodet() {
    let mut st = mk();
    let mut s = HostSession::new(&st, "Tekniker");
    let o = out(s.handle(&mut st, "ssh drift@192.168.1.193"));
    println!("{}|{}", o, s.prompt_text(&st));
    if s.prompt_text(&st).contains("authenticity") {
        let o = out(s.handle(&mut st, "yes"));
        println!("{}|{}", o, s.prompt_text(&st));
        assert!(s.secret_input());
        let r = s.handle(&mut st, "fel");
        println!("{}", r.out);
        let r = s.handle(&mut st, "fel");
        let r = s.handle(&mut st, "fel");
        println!("{}", r.out);
        assert!(matches!(r.push, None));
    }
    let _ = PushReq::Lb { dev: String::new() };
}
