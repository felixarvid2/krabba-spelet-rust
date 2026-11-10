//! Svårt läge: bevis för varje uppgift och tålig radmatchning.
use super::Verify;
use crate::net::util::norm_if;

fn mk(on: &[&'static str], cmds: &[&'static str], text: &'static str) -> Verify {
    Verify { on: on.to_vec(), cmds: cmds.to_vec(), text }
}

/// Beviset för en uppgift; None om uppgiften är okänd.
pub(super) fn table(id: &str) -> Option<Verify> {
    let fil: &[&'static str] = &["ping 192.168.1.10", "ping filserver"];
    Some(match id {
        // ---------------- Vecka 1
        "v1k1" => mk(&["SW1", "Tekniker"], &["show line", "show running-config | include speed", "show running-config | section line", "show startup-config | section line", "show startup-config | include speed", "screen /dev/ttyUSB0 9600"],
            "Bevisa: anslut från laptopen med screen /dev/ttyUSB0 9600, eller visa konsolinställningen på SW1 (show line, show running-config | section line)."),
        "v1k2" => mk(&["PC-Anna"], &["ping 192.168.1.10", "ping filserver", "ping 192.168.1.1", "ipconfig"],
            "Bevisa: pinga filservern (192.168.1.10) från Annas dator."),
        "v1n1" => mk(&["SW2"], &["show startup-config"],
            "Bevisa: visa den sparade konfigurationen på SW2 (show startup-config) så att namnet syns."),
        // ---------------- Vecka 2
        "v2k1" => mk(&["PC-Karim"], &["ping 192.168.1.10", "ping filserver", "ping 192.168.1.1"],
            "Bevisa: pinga filservern (192.168.1.10) från Karims dator."),
        "v2k2" => mk(&["SW1", "SW2"], &["show interfaces gi0/24", "show interfaces status"],
            "Bevisa: visa länkens hastighet och duplex på trunkporten (show interfaces gi0/24) på SW1 eller SW2."),
        "v2n1" => mk(&["PC-Bo"], &["ping 192.168.1.70", "ping ekonomi", "ping 192.168.1.65", "ipconfig"],
            "Bevisa: pinga ekonomisystemet (192.168.1.70) från Bos dator."),
        // ---------------- Vecka 3
        "v3k1" => mk(&["PC-Anna"], &["ping 192.168.1.70", "ping ekonomi"],
            "Bevisa: pinga ekonomisystemet (192.168.1.70) från Annas dator."),
        "v3k2" => mk(&["PC-Karim"], &["ping 192.168.1.10", "ping filserver", "ping 192.168.1.1", "ipconfig"],
            "Bevisa: pinga filservern (192.168.1.10) från Karims dator (som ska ha fått en riktig adress)."),
        "v3n1" => mk(&["PC-Maja"], &["nslookup ekonomi", "ping ekonomi"],
            "Bevisa: slå upp namnet ekonomi på Majas dator (nslookup ekonomi eller ping ekonomi)."),
        // ---------------- Vecka 4
        "v4k1" => mk(&["PC-Bo"], &["ping 192.168.1.70", "ping ekonomi"],
            "Bevisa: pinga ekonomisystemet (192.168.1.70) från Bos dator."),
        "v4k2" => mk(&["PC-Lisa"], &["ping 192.168.1.11", "ping 192.168.1.1", "ping 203.0.113.1"],
            "Bevisa: pinga skrivaren (192.168.1.11) från Lisas dator."),
        "v4n1" => mk(&["SW2", "SW1"], &["show spanning-tree"],
            "Bevisa: titta i spanning-tree (show spanning-tree) på SW2 och se varför Gi0/25 är orange."),
        // ---------------- Vecka 5
        "v5k1" => mk(&["PC-Lager", "RB"], &["ping 192.168.1.10", "ping 192.168.1.1", "ping 203.0.113.1"],
            "Bevisa: pinga filservern i Göteborg (192.168.1.10) från lagerdatorn."),
        "v5k2" => mk(&["PC-Anna", "PC-Karim", "R1"], &["ping 203.0.113.1", "ping 9.9.9.9"],
            "Bevisa: pinga internet (203.0.113.1) från en dator i Göteborg eller från R1."),
        "v5n1" => mk(&["PC-Maja", "PC-Bo"], &["ping 192.168.1.65", "ping 192.168.1.10", "ping filserver"],
            "Bevisa: pinga gatewayen (192.168.1.65) eller filservern (192.168.1.10) från Majas dator."),
        // ---------------- Vecka 6
        "v6k1" => mk(&["PC-Anna", "PC-Karim"], &["ping 203.0.113.1", "ping 9.9.9.9"],
            "Bevisa: pinga internet (203.0.113.1) från Annas dator."),
        "v6k2" => mk(&["Tekniker", "R1"], &["ssh drift@192.168.1.193", "ssh -l drift 192.168.1.193", "show ip ssh", "show crypto key mypubkey rsa"],
            "Bevisa: logga in med ssh drift@192.168.1.193 från laptopen, eller visa show ip ssh på R1."),
        "v6n1" => mk(&["R1", "Tekniker"], &["show running-config | section line vty", "show running-config | include transport", "telnet 192.168.1.193"],
            "Bevisa: visa vty-linjerna på R1 (show running-config | section line vty), eller försök med telnet från laptopen."),
        // ---------------- Vecka 7
        "v7k1" => mk(&["SW1", "SW2"], &["show spanning-tree"],
            "Bevisa: visa spanning-tree (show spanning-tree) på SW1 eller SW2 så att det finns en root bridge."),
        "v7k2" => mk(&["PC-Karim", "SW1"], &["ping 192.168.1.10", "ping filserver", "show interfaces gi0/6", "show interfaces status"],
            "Bevisa: pinga filservern (192.168.1.10) från Karims dator, eller visa Gi0/6 (show interfaces gi0/6) på SW1."),
        "v7n1" => mk(&["SW2"], &["show clock", "show ntp status", "show ntp associations", "show logging"],
            "Bevisa: visa klockan och NTP-status på SW2 (show clock eller show ntp status)."),
        // ---------------- Vecka 8
        "v8k1" => mk(&["SWB", "WLC"], &["show power inline", "show ap summary", "show interfaces status"],
            "Bevisa: visa PoE på SWB (show power inline) eller accesspunkterna på WLC (show ap summary)."),
        "v8k2" => mk(&["WLC"], &["show wlan summary", "show wlan 2", "show client summary"],
            "Bevisa: visa gäst-WLAN:et på WLC (show wlan summary eller show wlan 2) och se vilket gränssnitt det använder."),
        "v8n1" => mk(&["WLC"], &["show ap summary", "show ap channel", "show advanced 802.11b summary"],
            "Bevisa: visa accesspunkternas kanaler på WLC (show ap channel eller show ap summary)."),
        // ---------------- Vecka 9
        "v9k1" => mk(&["PC-Anna", "R1"], &["ping 192.168.1.70", "show access-lists", "show ip interface gi0/0.10"],
            "Bevisa: pinga ekonomisystemet (192.168.1.70) från Annas dator – det ska nu stoppas – eller visa listan med show access-lists på R1."),
        "v9k2" => mk(&["Gast-laptop"], &["ping 203.0.113.1", "ping 9.9.9.9"],
            "Bevisa: pinga internet (203.0.113.1) från gästlaptopen."),
        "v9n1" => mk(&["Gast-laptop", "SW1"], &["ping 203.0.113.1", "ping 192.168.1.129", "ipconfig", "show interfaces gi0/9", "show port-security interface gi0/9"],
            "Bevisa: pinga gatewayen (192.168.1.129) från gästlaptopen, eller visa Gi0/9 (show interfaces gi0/9) på SW1."),
        // ---------------- Vecka 10
        "v10k1" => mk(&["PC-Lager", "R1", "RB"], &["ping 192.168.1.10", "ping filserver", "show crypto ipsec sa"],
            "Bevisa: pinga filservern (192.168.1.10) från lagerdatorn, och titta i show crypto ipsec sa på R1 eller RB."),
        "v10k2" => mk(&["RB", "R1"], &["show running-config interface gi0/1", "show running-config | include mss", "show ip interface gi0/1"],
            "Bevisa: visa MSS-justeringen på tunnelns utgång (show running-config interface gi0/1) på RB."),
        "v10n1" => mk(&["Tekniker", "LB"], &["curl http://tid/", "curl http://192.168.1.13/", "show pool", "show monitor"],
            "Bevisa: hämta sidan flera gånger med curl http://tid/ från laptopen, eller visa poolen med show pool på LB."),
        // ---------------- Bygge 1
        "b1a" => mk(&["SW1"], &["show startup-config"],
            "Bevisa: visa den sparade konfigurationen på SW1 (show startup-config) med rätt namn."),
        "b1b" => mk(&["SW1"], &["show vlan brief"],
            "Bevisa: visa VLAN:en på SW1 (show vlan brief)."),
        "b1c" => mk(&["SW1"], &["show vlan brief", "show interfaces status"],
            "Bevisa: se att portarna ligger i rätt VLAN (show vlan brief eller show interfaces status) på SW1."),
        "b1d" => mk(&["SW1", "SW2"], &["show interfaces trunk"],
            "Bevisa: visa trunkarna på SW1 (show interfaces trunk)."),
        "b1e" => mk(&["Tekniker", "SW1"], &["ping 192.168.1.194", "ssh drift@192.168.1.194", "ssh -l drift 192.168.1.194", "show ip ssh", "show ip interface brief"],
            "Bevisa: pinga switchens driftadress (192.168.1.194) från laptopen eller logga in med ssh drift@192.168.1.194."),
        // ---------------- Bygge 2
        "b2a" => mk(&["Tekniker", "R1"], &["ping 192.168.1.193", "ping 192.168.1.1", "show ip interface brief"],
            "Bevisa: pinga routerns driftadress (192.168.1.193) från laptopen, eller visa show ip interface brief på R1."),
        "b2b" => mk(&["PC-Anna", "R1"], &["ping 192.168.1.1", "ping 192.168.1.10", "ipconfig", "show ip dhcp binding"],
            "Bevisa: visa Annas adress (ipconfig) och pinga gatewayen (192.168.1.1) från hennes dator."),
        "b2c" => mk(&["PC-Anna"], &["nslookup filserver", "ping filserver"],
            "Bevisa: slå upp filserver från Annas dator (nslookup filserver eller ping filserver)."),
        "b2d" => mk(&["PC-Anna", "Tekniker", "R1"], &["ping 203.0.113.1", "ping 9.9.9.9", "show ip nat translations"],
            "Bevisa: pinga internet (203.0.113.1) från Annas dator och titta i show ip nat translations på R1."),
        "b2e" => mk(&["PC-Lager", "R1"], &["ping 192.168.1.10", "ping 192.168.1.1", "ping 10.0.0.2", "show ip route"],
            "Bevisa: pinga filservern (192.168.1.10) från lagerdatorn eller 10.0.0.2 från R1."),
        "b2f" => mk(&["Tekniker", "R1"], &["ssh drift@192.168.1.193", "ssh -l drift 192.168.1.193", "telnet 192.168.1.193", "show ip ssh", "show running-config | include hostname"],
            "Bevisa: logga in med ssh drift@192.168.1.193 från laptopen (och se att telnet nekas), eller visa show ip ssh på R1."),
        // ---------------- Bygge 3
        "b3a" => mk(&["SWB"], &["show vlan brief"],
            "Bevisa: visa VLAN:en på SWB (show vlan brief)."),
        "b3b" => mk(&["SWB"], &["show interfaces trunk"],
            "Bevisa: visa trunkarna på SWB (show interfaces trunk)."),
        "b3c" => mk(&["RB"], &["show ip interface brief", "ping 10.0.0.1", "ping 192.168.1.1"],
            "Bevisa: visa adresserna på RB (show ip interface brief) och pinga Göteborg (10.0.0.1)."),
        "b3d" => mk(&["PC-Lager", "RB"], &["ipconfig", "ping 192.168.2.1", "ping 192.168.1.10", "show ip dhcp binding"],
            "Bevisa: visa lagerdatorns adress (ipconfig) och pinga gatewayen (192.168.2.1)."),
        "b3e" => mk(&["WLC"], &["show wlan summary", "show client summary", "show interface summary", "show ap summary"],
            "Bevisa: visa WLAN:en på WLC (show wlan summary) och att de pekar på rätt gränssnitt."),
        "b3f" => mk(&["RB", "PC-Lager"], &["show access-lists", "show ip interface gi0/0.50", "ping 192.168.1.10"],
            "Bevisa: visa gästlistan på RB (show access-lists), eller pinga filservern från lagerdatorn och se att lagret fortfarande når fram."),
        // ---------------- Bygge 4
        "b4a" => mk(&["R1", "Gast-laptop"], &["show access-lists", "ping 192.168.1.10", "ping 203.0.113.1"],
            "Bevisa: pinga filservern (192.168.1.10) från gästlaptopen – det ska stoppas – eller visa listan med show access-lists på R1."),
        "b4b" => mk(&["PC-Anna", "R1"], &["ping 192.168.1.70", "show access-lists"],
            "Bevisa: pinga ekonomisystemet (192.168.1.70) från Annas dator – det ska stoppas – eller visa listan med show access-lists på R1."),
        "b4c" => mk(&["SW1"], &["show port-security interface gi0/9", "show port-security"],
            "Bevisa: visa port security på Gi0/9 (show port-security interface gi0/9) på SW1."),
        "b4d" => mk(&["R1", "SW1", "SW2"], &["show clock", "show ntp status", "show ntp associations"],
            "Bevisa: visa klockan och NTP-status (show clock, show ntp status) på en av enheterna."),
        "b4e" => mk(&["R1", "SW1", "SW2"], &["show logging", "show running-config | include logging"],
            "Bevisa: visa loggningen (show logging) och att loggservern 192.168.1.17 finns med."),
        // ---------------- Bygge 5
        "b5a" => mk(&["R1", "RB"], &["show crypto isakmp policy", "show crypto isakmp sa", "show running-config | include isakmp"],
            "Bevisa: visa policyn på routern (show crypto isakmp policy)."),
        "b5b" => mk(&["R1", "RB"], &["show crypto ipsec transform-set", "show access-lists"],
            "Bevisa: visa transform-setet (show crypto ipsec transform-set) och listorna (show access-lists)."),
        "b5c" => mk(&["PC-Lager", "R1", "RB"], &["ping 192.168.1.10", "ping filserver", "show crypto isakmp sa", "show crypto ipsec sa"],
            "Bevisa: pinga filservern (192.168.1.10) från lagerdatorn och titta i show crypto ipsec sa på R1 eller RB."),
        "b5d" => mk(&["Tekniker", "WLC", "R1", "RB"], &["ping 192.168.2.193", "ssh drift@192.168.2.193", "ssh -l drift 192.168.2.193", "show ap summary", "show crypto ipsec sa"],
            "Bevisa: pinga Boråsroutern (192.168.2.193) från laptopen eller visa show ap summary på WLC."),
        "b5e" => mk(&["RB", "R1"], &["show running-config interface gi0/1", "show running-config | include mss", "show ip interface gi0/1"],
            "Bevisa: visa MSS-justeringen på tunnelns utgång (show running-config interface gi0/1) på RB."),
        // ---------------- Kurs 2
        "v11k1" => mk(&["PC-Molndal", "RM"], &["ping 192.168.1.10", "ping 192.168.1.1", "show ip route"],
            "Bevisa: pinga filservern (192.168.1.10) från Elins dator i Mölndal."),
        "v11k2" => mk(&["PC-Lager", "RB", "R1"], &["ping 192.168.3.10", "tracert 192.168.3.10", "traceroute 192.168.3.10"],
            "Bevisa: pinga Elins dator (192.168.3.10) från lagerdatorn eller kör traceroute 192.168.3.10."),
        "v11n1" => mk(&["RB", "RM", "PC-Lager"], &["show ip route static", "show ip route 192.168.3.0", "show ip route 192.168.2.0", "ping 192.168.3.10", "traceroute 192.168.3.10", "tracert 192.168.3.10"],
            "Bevisa: visa de statiska rutterna (show ip route static) på RB eller RM, och pinga mellan Borås och Mölndal."),
        "v12k1" => mk(&["PC-Lager", "R1", "RB"], &["ping 192.168.1.10", "ping filserver", "show crypto isakmp sa"],
            "Bevisa: visa fas 1 (show crypto isakmp sa, ska vara QM_IDLE) eller pinga filservern från lagret."),
        "v12k2" => mk(&["R1", "RB", "PC-Lager"], &["show crypto ipsec sa", "ping 192.168.1.10", "show access-lists NAT-UT"],
            "Bevisa: titta i show crypto ipsec sa på R1 eller RB efter ping (paketen ska krypteras och avkrypteras)."),
        "v12n1" => mk(&["PC-Molndal", "R1"], &["ping 192.168.1.70", "ping 192.168.1.10", "show access-lists MOLNDAL-IN", "show ip interface se0/0/0"],
            "Bevisa: pinga ekonomisystemet (192.168.1.70) från Elins dator – det ska stoppas – och sedan filservern (192.168.1.10), som ska svara."),
        "v13k1" => mk(&["RB", "RM", "PC-Lager"], &["show ip ospf neighbor", "ping 192.168.1.10"],
            "Bevisa: visa OSPF-grannarna på RB (show ip ospf neighbor) och pinga filservern från lagerdatorn."),
        "v13k2" => mk(&["R1", "PC-Molndal"], &["show ip route ospf", "show ip route 192.168.3.0", "show ip ospf interface brief", "traceroute 192.168.3.10", "tracert 192.168.1.10"],
            "Bevisa: visa OSPF-rutten till Mölndal (show ip route ospf) på R1 eller kör traceroute."),
        "v13n1" => mk(&["R1", "RB", "RM", "PC-Anna"], &["show ip route ospf", "ping 10.255.0.3", "show ip protocols"],
            "Bevisa: pinga loopbacken (10.255.0.3) från R1, RB eller en dator, eller visa show ip route ospf."),
        "v14k1" => mk(&["R1"], &["show ip bgp summary", "show ip bgp"],
            "Bevisa: visa BGP-grannen (show ip bgp summary) på R1."),
        "v14k2" => mk(&["PC-Lager", "PC-Molndal", "R1"], &["ping 203.0.113.1", "ping 9.9.9.9", "show ip nat translations"],
            "Bevisa: pinga internet (203.0.113.1) från lagerdatorn eller visa show ip nat translations på R1."),
        "v14n1" => mk(&["R1"], &["show ip bgp", "show ip bgp summary", "show ip nat statistics"],
            "Bevisa: visa båda operatörernas rutter (show ip bgp) och BGP-grannarna (show ip bgp summary) på R1."),
        _ => return None,
    })
}

// ---------------------------------------------------------------- Matchning

/// Delar upp en rad i ord (gemener, "|" som eget ord, utan sudo/do).
fn tokens(line: &str) -> Vec<String> {
    let l = line.to_lowercase().replace('|', " | ");
    let mut t: Vec<String> = l.split_whitespace().map(|s| s.to_string()).collect();
    while t.first().is_some_and(|w| w == "sudo" || w == "do") && t.len() > 1 {
        t.remove(0);
    }
    t
}

/// Tar bort flaggor (-n 4, -c 3, -s) ur värdkommandon så att "ping -n 4 x" blir "ping x".
fn strip_flags(t: Vec<String>) -> Vec<String> {
    let host_cmd = t.first().is_some_and(|w| matches!(w.as_str(), "ping" | "curl" | "wget" | "nslookup" | "dig" | "tracert" | "traceroute"));
    if !host_cmd {
        return t;
    }
    let mut out = vec![];
    let mut i = 0;
    while i < t.len() {
        let w = &t[i];
        if i > 0 && w.starts_with('-') && w.len() > 1 {
            // flaggans värde: ett tal eller do/dont/want
            if let Some(n) = t.get(i + 1) {
                if n.chars().all(|c| c.is_ascii_digit()) || matches!(n.as_str(), "do" | "dont" | "want") {
                    i += 1;
                }
            }
        } else {
            out.push(w.clone());
        }
        i += 1;
    }
    out
}

fn word_ok(lw: &str, pw: &str) -> bool {
    if lw == pw {
        return true;
    }
    // interfacenamn: gi0/24 = GigabitEthernet0/24
    if lw.chars().any(|c| c.is_ascii_digit()) {
        if let (Some(a), Some(b)) = (norm_if(lw), norm_if(pw)) {
            return a == b;
        }
        return false;
    }
    // IOS-förkortning: bara rena bokstavsord
    pw.chars().all(|c| c.is_ascii_alphabetic() || c == '-') && lw.chars().all(|c| c.is_ascii_alphabetic() || c == '-') && pw.starts_with(lw)
}

fn line_matches(pattern: &str, line: &str) -> bool {
    let p = tokens(pattern);
    let l = strip_flags(tokens(line));
    if p.is_empty() || l.len() < p.len() {
        return false;
    }
    p.iter().zip(l.iter()).all(|(pw, lw)| word_ok(lw, pw))
}

pub(super) fn matches(v: &Verify, dev: &str, line: &str) -> bool {
    v.on.iter().any(|d| *d == dev) && v.cmds.iter().any(|c| line_matches(c, line))
}
