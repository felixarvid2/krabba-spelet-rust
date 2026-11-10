//! Fler kommandon i Windows- och Linux-terminalerna (host_more.js).
//! Lägger sig framför grundkommandona: känns kommandot inte igen här går det vidare till win_base/lin_base.
use super::host::*;
use super::CmdResult;
use crate::net::model::*;
use crate::net::sim::{self as S, IpConf, PingOpts, State};
use crate::net::util as U;

const WIN_ALL: &[&str] = &[
    "ipconfig", "ping", "tracert", "nslookup", "arp", "netsh", "curl", "copy", "route", "netstat", "getmac", "hostname", "whoami",
    "systeminfo", "test-netconnection", "pathping", "test-connection", "resolve-dnsname", "get-netipaddress",
    "get-netipconfiguration", "get-netadapter", "net", "w32tm", "dir", "tasklist", "cls", "exit", "help", "echo", "ver", "date",
    "time", "nbtstat", "get-netneighbor", "clear-dnsclientcache", "get-nettcpconnection", "get-date", "get-command",
];
const LIN_ALL: &[&str] = &[
    "ssh", "screen", "ping", "traceroute", "tracepath", "curl", "wget", "nslookup", "dig", "host", "resolvectl", "ip", "telnet",
    "nc", "man", "history", "clear", "exit", "nmap", "mtr", "ss", "netstat", "route", "ethtool", "nmcli", "arping", "ssh-keygen",
    "hostnamectl", "uname", "help", "ls", "cat", "echo", "date", "whoami", "uptime", "dmesg", "hostname", "pwd", "free", "df", "ps",
    "lsof", "w", "last", "env", "ping6", "ssh-copy-id", "tcpdump", "iperf3", "scp",
];

fn man2(k: &str) -> Option<&'static str> {
    Some(match k {
        "wget" => "WGET(1)\n  wget http://<adress>/\n  Hämtar en webbsida och sparar den som index.html. Läs den med cat index.html. Ger samma fel som curl när servern inte svarar.",
        "ps" => "PS(1)\n  ps\n  Visar processerna i terminalen.",
        "df" => "DF(1)\n  df -h\n  Visar hur mycket plats som finns kvar på diskarna.",
        "free" => "FREE(1)\n  free -h\n  Visar hur mycket minne som används.",
        "lsof" => "LSOF(8)\n  lsof -i\n  Visar vilka program som lyssnar på eller använder nätverksportar.",
        "ping6" => "PING6(8)\n  ping6 ::1\n  Pingar en IPv6-adress. Nordviks nät kör bara IPv4.",
        "w" => "W(1)\n  w\n  Visar vem som är inloggad och vad de gör.",
        "nmap" => "NMAP(1)\n  nmap -sn 192.168.1.192/26     vilka adresser svarar i nätet (ping-svep)\n  nmap -p 22,23,80 <ip>         vilka TCP-portar är öppna\n  Använd bara mot nät du har lov att skanna – här är det ditt eget.",
        "mtr" => "MTR(8)\n  mtr -r -c 10 <mål>\n  Traceroute och ping i ett: förlust och svarstid per hopp.",
        "ss" => "SS(8)\n  ss -tuln    portar som lyssnar på den här datorn\n  ss -tn      öppna TCP-anslutningar",
        "ethtool" => "ETHTOOL(8)\n  ethtool enp0s31f6\n  Visar hastighet, duplex och om länken är uppe. Bra när en port har auto/half duplex-problem.",
        "nmcli" => "NMCLI(1)\n  nmcli device status       nätverkskortens status\n  nmcli device show         adress, gateway och DNS",
        "arping" => "ARPING(8)\n  arping -c 3 <ip>\n  Skickar ARP-frågor i stället för ping. Fungerar även om målet blockerar ICMP.",
        "tcpdump" => "TCPDUMP(8)\n  sudo tcpdump -i enp0s31f6 -c 10\n  Fångar paket på nätverkskortet: ARP, DNS, NTP, STP och annat som syns på porten.",
        "iperf3" => "IPERF3(1)\n  iperf3 -c <server>\n  Mäter hur snabbt det går att skicka data till en server. Låg fart och många omsändningar tyder ofta på duplex-fel.",
        "ssh-keygen" => "SSH-KEYGEN(1)\n  ssh-keygen -R <värd>\n  Tar bort en sparad värdnyckel ur ~/.ssh/known_hosts (när en enhet har bytts ut).",
        "netstat" => "NETSTAT(8)\n  netstat -rn    routingtabellen\n  netstat -tuln  portar som lyssnar",
        "dhclient" => "DHCLIENT(8)\n  sudo dhclient -r    släpp adressen\n  sudo dhclient       begär en ny adress",
        "traceroute" => "TRACEROUTE(8)\n  traceroute [-n] <mål>\n  Visar routrarna på vägen. * * * betyder att ett hopp inte svarar.",
        "dig" => "DIG(1)\n  dig <namn>        fullständigt DNS-svar\n  dig +short <namn> bara adressen",
        "nc" => "NC(1)\n  nc -zv värd port\n  Testar om en TCP-port svarar.",
        _ => return None,
    })
}

fn portname(p: i64) -> &'static str {
    match p {
        21 => "ftp",
        22 => "ssh",
        23 => "telnet",
        25 => "smtp",
        53 => "domain",
        67 => "dhcps",
        80 => "http",
        123 => "ntp",
        161 => "snmp",
        443 => "https",
        445 => "microsoft-ds",
        514 => "syslog",
        631 => "ipp",
        3389 => "ms-wbt-server",
        5246 => "capwap-control",
        8080 => "http-proxy",
        9100 => "jetdirect",
        _ => "unknown",
    }
}

fn to_base36(mut n: u32) -> String {
    if n == 0 {
        return "0".into();
    }
    let mut s = vec![];
    while n > 0 {
        s.push(std::char::from_digit(n % 36, 36).unwrap());
        n /= 36;
    }
    s.iter().rev().collect()
}

fn norm_ws(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Namnet på enheten som har adressen (deviceName i JS)
fn device_name(st: &mut State, ip: &str) -> Option<String> {
    let d = S::get(st);
    let mut hit = None;
    for e in d.eps.values() {
        if e.ip.as_deref() == Some(ip) && e.up {
            hit = Some(e.dev.clone());
        }
    }
    let hit = hit?;
    let dv = st.devices.get(&hit)?;
    Some(match &dv.config {
        Some(c) if !c.hostname.is_empty() => c.hostname.to_lowercase(),
        _ => (if !dv.label.is_empty() && dv.kind == Kind::Wlc { "wlc-nordvik".to_string() } else { dv.id.clone() }).to_lowercase(),
    })
}

/// Omvänt namnuppslag: adress → namn
fn reverse(st: &mut State, ip: &str) -> Option<String> {
    let mut hit = None;
    if let Some(r1) = st.devices.get("R1") {
        if let Some(c) = &r1.config {
            for (n, v) in &c.hosts {
                if v == ip {
                    hit = Some(format!("{}.nordvik.example", n));
                }
            }
        }
    }
    hit.or_else(|| device_name(st, ip).map(|n| format!("{}.nordvik.example", n)))
}

fn subnet_hosts(cidr: &str) -> Option<Option<Vec<String>>> {
    // None = ingen CIDR, Some(None) = för stort
    let (ip, pre) = cidr.split_once('/')?;
    let ok_ip = ip.split('.').count() == 4 && ip.split('.').all(|x| is_digits(x));
    if !ok_ip || !is_digits(pre) {
        return None;
    }
    let pre: u32 = pre.parse().ok()?;
    if pre < 24 || pre > 30 {
        return Some(None);
    }
    let mask = U::prefix_to_mask(pre);
    let net = U::ipn(&U::network(ip, &mask));
    let n = 1u64 << (32 - pre);
    let mut out = vec![];
    for i in 1..(n - 1) {
        out.push(U::int_to_ip(net.wrapping_add(i as u32)));
    }
    Some(Some(out))
}

impl HostSession {
    /// Namn som har slagits upp i sessionen sparas, som i en riktig DNS-cache
    fn remember(&mut self, st: &State, name: &str, ip: &str) {
        if name.is_empty() || U::is_ip(name) || ip.is_empty() {
            return;
        }
        self.dns_cache.insert(name.to_lowercase(), (ip.to_string(), st.time));
    }
    /// Ok(ip) eller Err(felkod)
    fn resolve_for(&mut self, st: &mut State, name: &str) -> Result<String, String> {
        if U::is_ip(name) {
            return Ok(name.to_string());
        }
        let rr = S::resolve(st, &self.dev, name);
        if let Some(e) = rr.error {
            return Err(e);
        }
        let ip = rr.ip.unwrap_or_default();
        self.remember(st, name, &ip);
        Ok(ip)
    }

    fn port_of(&self, st: &mut State) -> Option<S::PortInfo> {
        let d = S::get(st);
        d.ports.get(&S::key(&self.dev, "nic")).cloned()
    }

    // ================================================================ Windows
    pub(super) fn win_layer(&mut self, st: &mut State, line: &str) -> CmdResult {
        if let Some(pp) = split_pipe(line) {
            self.whist.push(line.trim().to_string());
            let left = self.win_layer(st, &pp.left);
            return pipe_filter(left, &pp);
        }
        let a = splitargs(line);
        if a.is_empty() {
            return self.win_base(st, line);
        }
        if line.trim() == "?" {
            return self.win_layer(st, "help");
        }
        let cmd = a[0].to_lowercase();
        let id = self.dev.clone();
        // Historik: doskey /history i cmd och Get-History (h) i PowerShell
        if (cmd == "doskey" && line.to_lowercase().contains("/history")) || cmd == "get-history" || (cmd == "h" && a.len() == 1) {
            let wl = self.whist.clone();
            self.whist.push(line.trim().to_string());
            if cmd == "doskey" {
                return r(wl.join("\n"));
            }
            return r(format!(
                "\n  Id CommandLine\n  -- -----------\n{}",
                wl.iter().enumerate().map(|(i, l)| format!("{} {}", U::pad_l(i + 1, 4), l)).collect::<Vec<_>>().join("\n")
            ));
        }
        if cmd == "doskey" {
            return r("💡 doskey /history visar kommandona du har skrivit.");
        }
        self.whist.push(line.trim().to_string());
        if self.whist.len() > 200 {
            self.whist.remove(0);
        }
        // Kom ihåg uppslagna namn för ipconfig /displaydns
        if ["ping", "tracert", "nslookup", "curl", "curl.exe", "pathping", "tnc", "test-netconnection"].contains(&cmd.as_str()) {
            let nm = a[1..]
                .iter()
                .find(|x| !x.starts_with('-') && !x.starts_with('/') && !is_digits(x) && !starts_ci(x, "http:") && !starts_ci(x, "https:"))
                .cloned();
            if let Some(nm) = nm {
                let _ = self.resolve_for(st, &nm);
            }
            let joined = a[1..].join(" ");
            let after = if starts_ci(&joined, "https://") {
                Some(joined[8..].to_string())
            } else if starts_ci(&joined, "http://") {
                Some(joined[7..].to_string())
            } else {
                None
            };
            if let Some(rest) = after {
                let h: String = rest.chars().take_while(|c| *c != '/' && *c != ':').collect();
                if !h.is_empty() {
                    let _ = self.resolve_for(st, &h);
                }
            }
        }
        let lower = line.to_lowercase();
        let nl = norm_ws(line);
        if cmd == "ipconfig" && lower.contains("/flushdns") {
            self.dns_cache.clear();
            return r("\nWindows IP-konfiguration\n\nRensade DNS-matchningscachen.");
        }
        if cmd == "ipconfig" && lower.contains("/displaydns") {
            if self.dns_cache.is_empty() {
                return r("\nWindows IP-konfiguration\n\nDNS-matchningscachen är tom. (Pinga ett namn först.)");
            }
            let t = st.time;
            let body: Vec<String> = self
                .dns_cache
                .iter()
                .map(|(k, (ip, at))| {
                    let left = (300 - jsr(t - at)).max(1);
                    format!(
                        "    {}{}\n    ----------------------------------------\n    Postnamn . . . . . . : {}\n    Posttyp  . . . . . . : 1\n    Livstid  . . . . . . : {}\n    Datalängd  . . . . . : 4\n    Avsnitt  . . . . . . : Svar\n    A-post (värd)  . . . : {}\n",
                        k,
                        if !k.contains('.') { ".nordvik.example" } else { "" },
                        k,
                        left,
                        ip
                    )
                })
                .collect();
            return r(format!("\nWindows IP-konfiguration\n\n{}", body.join("\n")));
        }
        if cmd == "arp" && a.get(1).map(|x| x == "-d").unwrap_or(false) {
            st.dev_mut(&id).arp.clear();
            return r("");
        }
        if cmd == "netsh" && nl.contains("interface show interface") {
            let up = nic_up(st, &id);
            return r(format!(
                "\nAdministratörstillstånd   Tillstånd     Typ              Gränssnittsnamn\n-------------------------------------------------------------------------\nAktiverad                {}Dedikerad        Ethernet",
                U::pad(if up { "Ansluten" } else { "Frånkopplad" }, 14)
            ));
        }
        if cmd == "netsh" && (nl.contains("interface ipv4 show config")) {
            let h = self.me(st);
            let cn = conf(h);
            let dhcp = h.nic.as_ref().map(|n| n.dhcp).unwrap_or(false);
            let (ip, sub, gw, dns) = match &cn {
                Some(c) => {
                    let ip = c.ip.clone().unwrap_or_default();
                    let mask = cmask(c);
                    (
                        ip.clone(),
                        format!("{}/{} (mask {})", U::network(&ip, &mask), U::mask_to_prefix(&mask).map(|p| p.to_string()).unwrap_or_default(), mask),
                        c.gw.clone().unwrap_or_default(),
                        c.dns.join(", "),
                    )
                }
                None => ("–".into(), "–".into(), String::new(), String::new()),
            };
            return r(format!(
                "\nKonfiguration för gränssnittet \"Ethernet\"\n    DHCP aktiverat:                         {}\n    IP-adress:                              {}\n    Undernätsprefix:                        {}\n    Standardgateway:                        {}\n    Gatewaymått:                            0\n    Gränssnittsmått:                        25\n    DNS-servrar som konfigurerats via DHCP: {}",
                if dhcp { "Ja" } else { "Nej" },
                ip,
                sub,
                gw,
                dns
            ));
        }
        if cmd == "ping" && a.get(1).map(|x| x == "-a").unwrap_or(false) && a.get(2).is_some() {
            let rn2 = reverse(st, &a[2]);
            let mut rp = self.win_base(st, &format!("ping {}", a[2..].join(" ")));
            if let (Some(rn2), Some(first)) = (rn2, rp.stream.first_mut()) {
                first.text = first.text.replacen(&format!("till {}", a[2]), &format!("till {} [{}]", rn2, a[2]), 1);
            }
            return rp;
        }
        if cmd == "getmac" && lower.contains("/v") {
            return r(format!(
                "\nAnslutningsnamn  Nätverkskort     Fysisk adress       Transportnamn\n================ ================ =================== ==========================================================\nEthernet         Intel(R) Etherne {}   \\Device\\Tcpip_{{4E2B9A1C-7D11-4F2E-9B3A-1C5E7D9F0A21}}",
                U::mac_dash(&self.me(st).mac())
            ));
        }
        if cmd == "ping" && a.iter().any(|x| x == "-t") {
            let rest: Vec<String> = a[1..].iter().filter(|x| *x != "-t").cloned().collect();
            let mut rr = self.win_base(st, &format!("ping -n 8 {}", rest.join(" ")));
            if !rr.stream.is_empty() {
                rr.stream.push(ck("\nControl-C\n^C (ping -t pingar tills du trycker Ctrl+C – här stannar den efter 8 svar)", 100));
            }
            return rr;
        }
        if cmd == "netstat" && line.contains("-a") {
            return r("\nAktiva anslutningar\n\n  Proto  Lokal adress           Utländsk adress        Tillstånd\n  TCP    0.0.0.0:135            0.0.0.0:0              LYSSNAR\n  TCP    0.0.0.0:445            0.0.0.0:0              LYSSNAR\n  TCP    0.0.0.0:3389           0.0.0.0:0              LYSSNAR\n  UDP    0.0.0.0:5353           *:*\n  UDP    127.0.0.1:1900         *:*");
        }
        if let Some(rr) = self.win_extra(st, &a, &cmd, line) {
            return rr;
        }
        if cmd == "help" || cmd == "/?" {
            let mut r6 = self.win_base(st, line);
            r6.out.push_str("\n\nFler kommandon:\n  ipconfig /displaydns, ipconfig /flushdns   DNS-cachen\n  ping -t <adress>                    pinga tills du avbryter\n  pathping <adress>                   väg och förlust per hopp\n  arp -d                              töm ARP-cachen\n  netstat -an                         portar som lyssnar\n  net use Z: \\\\filserver\\delat          koppla en nätverksenhet\n  net view \\\\filserver                  visa delade mappar\n  w32tm /query /status                varifrån datorn får tiden\n  ping -a <ip>                        visa namnet för en adress\n  netsh interface show interface      är nätverkskortet anslutet?\n  netsh interface ipv4 show config    adress, gateway och DNS\n  getmac /v                           MAC-adress per nätverkskort\n  PowerShell: Test-Connection, Resolve-DnsName, Get-NetIPAddress,\n              Get-NetIPConfiguration, Get-NetAdapter, Get-NetRoute,\n              Get-DnsClientServerAddress");
            return r6;
        }
        let mut rw = self.win_base(st, line);
        if rw.out.contains("känns inte igen som ett internt") {
            if let Some(sg) = suggest(&cmd, WIN_ALL) {
                rw.out.push_str(&format!("\n💡 Menade du {}?", sg));
            }
        }
        rw
    }

    fn win_extra(&mut self, st: &mut State, a: &[String], cmd: &str, line: &str) -> Option<CmdResult> {
        let id = self.dev.clone();
        match cmd {
            "pathping" => {
                let Some(tgt) = a[1..].iter().find(|x| !x.starts_with('-')).cloned() else { return Some(r("Användning: pathping <målnamn>")) };
                let ip = match self.resolve_for(st, &tgt) {
                    Ok(ip) => ip,
                    Err(_) => return Some(r(format!("Det gick inte att matcha målsystemnamnet {}.", tgt))),
                };
                let (fwd, full) = S::traceroute(st, &id, &ip, None);
                let mut hops: Vec<String> = fwd.hops.iter().map(|x| x.ip.clone()).collect();
                if full.ok && hops.last() != Some(&ip) {
                    hops.push(ip.clone());
                }
                let me = conf(self.me(st)).and_then(|c| c.ip).unwrap_or_else(|| "0.0.0.0".into());
                let mut stream = vec![ck(format!("\nSpårar väg till {} [{}]\növer högst 30 hopp:\n  0  {} [{}]\n", tgt, ip, id, me), 200)];
                for (i, h) in hops.iter().enumerate() {
                    stream.push(ck(format!("{}  {}\n", U::pad_l(i + 1, 3), h), 350));
                }
                if !full.ok {
                    stream.push(ck(format!("{}     *        *        *\n", U::pad_l(hops.len() + 1, 3)), 900));
                }
                stream.push(ck(format!("\nBeräknar statistik för {} sekunder...\n", (hops.len() + 1) * 25), 1200));
                stream.push(ck(
                    format!(
                        "            Källa till hit   Den här noden/länken\nHopp  RTT    Förlust/Skick = Pct  Förlust/Skick = Pct  Adress\n  0                                           {}\n",
                        me
                    ),
                    300,
                ));
                for (i, h) in hops.iter().enumerate() {
                    let lost = if !full.ok && i == hops.len() - 1 { 100 } else { 0 };
                    stream.push(ck(
                        format!(
                            "                                0/ 100 =  0%   |\n{}{}ms{}/ 100 = {}%{}/ 100 = {}%  {}\n",
                            U::pad_l(i + 1, 3),
                            U::pad_l((i + 1) * 2, 6),
                            U::pad_l(lost, 8),
                            U::pad_l(lost, 2),
                            U::pad_l(lost, 8),
                            U::pad_l(lost, 2),
                            h
                        ),
                        120,
                    ));
                }
                stream.push(ck("\nSpårningen är slutförd.", 100));
                Some(rs(stream))
            }
            "test-connection" => {
                let Some(t2) = a[1..].iter().find(|x| !x.starts_with('-')).cloned() else { return Some(r("Test-Connection: Parametern -TargetName saknas.")) };
                let ip = match self.resolve_for(st, &t2) {
                    Ok(ip) => ip,
                    Err(_) => return Some(r(format!("Test-Connection: Det gick inte att matcha värdnamnet {}.", t2))),
                };
                let p2 = S::ping(st, &id, &ip, PingOpts::default());
                let mut rows = vec![
                    String::new(),
                    format!("   Destination: {}", t2),
                    String::new(),
                    "Ping Source           Address                   Latency BufferSize Status".into(),
                    "                                                   (ms)        (B)".into(),
                    "---- ------           -------                   ------- ---------- ------".into(),
                ];
                for i in 1..=4usize {
                    rows.push(format!(
                        "{} {}{}{}{} {}",
                        U::pad_l(i, 4),
                        U::pad(id.to_uppercase(), 21),
                        U::pad(&ip, 26),
                        U::pad_l(if p2.ok { 1 + i % 2 } else { 0 }, 7),
                        U::pad_l(32, 11),
                        if p2.ok { "Success" } else { "TimedOut" }
                    ));
                }
                Some(rd(rows.join("\n"), if p2.ok { 600 } else { 3000 }))
            }
            "resolve-dnsname" => {
                let Some(n3) = a.get(1).cloned() else { return Some(r("Resolve-DnsName: Parametern -Name saknas.")) };
                match self.resolve_for(st, &n3) {
                    Err(_) => Some(r(format!("Resolve-DnsName : {} : DNS-namnet finns inte", n3))),
                    Ok(ip) => Some(r(format!(
                        "\nName                                           Type   TTL   Section    IPAddress\n----                                           ----   ---   -------    ---------\n{}A      300   Answer     {}",
                        U::pad(format!("{}{}", n3, if !n3.contains('.') { ".nordvik.example" } else { "" }), 47),
                        ip
                    ))),
                }
            }
            "get-netipaddress" | "get-netipconfiguration" => {
                let up = nic_up(st, &id);
                let h = self.me(st);
                let c = conf(h);
                let dhcp = h.nic.as_ref().map(|n| n.dhcp).unwrap_or(false);
                if cmd == "get-netipaddress" {
                    let ipv = match &c {
                        Some(c) => c.ip.clone().unwrap_or_default(),
                        None => format!("169.254.{}.{}", U::hash(&id) % 250 + 1, U::hash(&format!("{}x", id)) % 250 + 1),
                    };
                    let pl = c.as_ref().and_then(|c| U::mask_to_prefix(&cmask(c))).unwrap_or(16);
                    return Some(r(format!(
                        "\nIPAddress         : {}\nInterfaceIndex    : 12\nInterfaceAlias    : Ethernet\nAddressFamily     : IPv4\nType              : Unicast\nPrefixLength      : {}\nPrefixOrigin      : {}\nAddressState      : {}",
                        ipv,
                        pl,
                        if dhcp { "Dhcp" } else { "Manual" },
                        if up { "Preferred" } else { "Tentative" }
                    )));
                }
                Some(r(format!(
                    "\nInterfaceAlias       : Ethernet\nInterfaceIndex       : 12\nInterfaceDescription : Intel(R) Ethernet Connection I219-LM\nNetProfile.Name      : nordvik.local\nIPv4Address          : {}\nIPv4DefaultGateway   : {}\nDNSServer            : {}\nNetAdapter.Status    : {}",
                    c.as_ref().map(|c| c.ip.clone().unwrap_or_default()).unwrap_or_else(|| "–".into()),
                    c.as_ref().and_then(|c| c.gw.clone()).unwrap_or_default(),
                    c.as_ref().map(|c| c.dns.join(", ")).unwrap_or_default(),
                    if up { "Up" } else { "Disconnected" }
                )))
            }
            "get-netadapter" => {
                let pt = self.port_of(st);
                let up2 = nic_up(st, &id);
                let mac = U::mac_dash(&self.me(st).mac());
                let sp = pt.as_ref().and_then(|p| p.neg.as_ref()).filter(|n| n.speed != 0).map(|n| format!("{} Mbps", n.speed)).unwrap_or_else(|| "0 bps".into());
                let speed = if up2 { if sp == "0 bps" { "1 Gbps".to_string() } else { sp.replace("1000 Mbps", "1 Gbps") } } else { "0 bps".to_string() };
                Some(r(format!(
                    "\nName      InterfaceDescription                 ifIndex Status       MacAddress         LinkSpeed\n----      --------------------                 ------- ------       ----------         ---------\nEthernet  Intel(R) Ethernet Connection I219-LM      12 {}{}  {}",
                    U::pad(if up2 { "Up" } else { "Disconnected" }, 13),
                    mac,
                    speed
                )))
            }
            "net" => {
                let sub = a.get(1).map(|x| x.to_lowercase()).unwrap_or_default();
                if sub == "use" {
                    let Some(drv) = a.get(2) else { return Some(r("Nya anslutningar kommer att sparas.\n\nDet finns inga poster i listan.")) };
                    let _ = drv;
                    let share = a.get(3).cloned().unwrap_or_default();
                    let host = share.strip_prefix("\\\\").and_then(|s| s.split_once('\\')).filter(|(h, p)| !h.is_empty() && !p.is_empty()).map(|(h, _)| h.to_string());
                    let Some(host) = host else { return Some(r("Kommandots syntax är:\n\nNET USE [enhet:] \\\\dator\\resurs")) };
                    let ip = match self.resolve_for(st, &host) {
                        Ok(ip) => ip,
                        Err(_) => return Some(rd("Systemfel 53 har uppstått.\n\nNätverkssökvägen hittades inte.", 2000)),
                    };
                    let p4 = S::ping(st, &id, &ip, PingOpts::tcp(445));
                    if !p4.ok {
                        return Some(rd("Systemfel 53 har uppstått.\n\nNätverkssökvägen hittades inte.", 2500));
                    }
                    return Some(rd("Kommandot har slutförts.", 500));
                }
                if sub == "view" {
                    let host = a.get(2).and_then(|x| x.strip_prefix("\\\\")).filter(|h| !h.is_empty()).map(|h| h.to_string());
                    let Some(host) = host else { return Some(r("Systemfel 6118 har uppstått.\n\nListan med servrar för den här arbetsgruppen är inte tillgänglig.")) };
                    let ok = match self.resolve_for(st, &host) {
                        Ok(ip) => S::ping(st, &id, &ip, PingOpts::tcp(445)).ok,
                        Err(_) => false,
                    };
                    if !ok {
                        return Some(rd("Systemfel 53 har uppstått.\n\nNätverkssökvägen hittades inte.", 2500));
                    }
                    return Some(rd(format!("Delade resurser på \\\\{}\n\nResursnamn  Typ   Används som  Kommentar\n\n-------------------------------------------------------------------------------\ndelat       Disk               Gemensamma filer\nekonomi     Disk               Bara ekonomi\nscan        Disk               Skannade dokument\nKommandot har slutförts.", host), 600));
                }
                Some(r("Kommandots syntax är:\n\nNET\n    [ USE | VIEW ]"))
            }
            "w32tm" => {
                if !line.to_lowercase().contains("/query") {
                    return Some(r("w32tm /query /status"));
                }
                let gw = conf(self.me(st)).and_then(|c| c.gw).filter(|g| !g.is_empty());
                Some(r(format!(
                    "Skottindikator: 0(ingen varning)\nSkikt: 4 (sekundär referens - synkroniserad via (S)NTP)\nPrecision: -23 (119,209ns per tick)\nRotfördröjning: 0.0039 s\nKälla: {}\nSenaste lyckade synkroniseringstid: 2026-09-29 07:58:12\nAvsökningsintervall: 10 (1024s)",
                    if gw.is_some() { "192.168.1.16 (ntp.nordvik.example)" } else { "Local CMOS Clock" }
                )))
            }
            "get-netroute" => {
                let Some(cr) = conf(self.me(st)) else { return Some(r("")) };
                let ip = cr.ip.clone().unwrap_or_default();
                let mask = cmask(&cr);
                Some(r(format!(
                    "\nifIndex DestinationPrefix                              NextHop                                  RouteMetric ifMetric PolicyStore\n------- -----------------                              -------                                  ----------- -------- -----------\n{}12      {}{}256         25       ActiveStore",
                    cr.gw.as_ref().filter(|g| !g.is_empty()).map(|g| format!("12      0.0.0.0/0                                      {}0           25       ActiveStore\n", U::pad(g, 41))).unwrap_or_default(),
                    U::pad(format!("{}/{}", U::network(&ip, &mask), U::mask_to_prefix(&mask).map(|p| p.to_string()).unwrap_or_default()), 47),
                    U::pad("0.0.0.0", 41)
                )))
            }
            "get-dnsclientserveraddress" => {
                let cd = conf(self.me(st));
                Some(r(format!(
                    "\nInterfaceAlias               Interface Address ServerAddresses\n                             Index     Family\n--------------               --------- ------- ---------------\nEthernet                            12 IPv4    {{{}}}",
                    cd.map(|c| c.dns.join(", ")).unwrap_or_default()
                )))
            }
            "dir" => Some(r(format!(" Volymen i enhet C har ingen etikett.\n Volymens serienummer är 4A1F-9C2E\n\n Innehåll i katalogen C:\\Users\\{}\n\n2026-09-28  16:20    <KATALOG>          .\n2026-09-28  16:20    <KATALOG>          ..\n2026-09-21  09:12    <KATALOG>          Desktop\n2026-09-29  07:55    <KATALOG>          Documents\n2026-09-27  14:03    <KATALOG>          Downloads\n               0 fil(er)              0 byte\n               5 katalog(er)  182 431 744 000 byte ledigt", self.user))),
            "cd" => Some(r(if a.get(1).is_some() { String::new() } else { format!("C:\\Users\\{}", self.user) })),
            "type" => Some(r(if a.get(1).map(|x| x.to_lowercase().contains("hosts")).unwrap_or(false) {
                "# Copyright (c) 1993-2009 Microsoft Corp.\n#\n# This is a sample HOSTS file used by Microsoft TCP/IP for Windows.\n\n# localhost name resolution is handled within DNS itself.\n#\t127.0.0.1       localhost"
            } else {
                "Det går inte att hitta den angivna filen."
            })),
            "tasklist" => Some(r("\nAvbildningsnamn                PID Sessionsnamn       Session#    Minnesanv.\n========================= ======== ================ =========== ============\nSystem                           4 Services                   0        144 kB\nsvchost.exe                    988 Services                   0     24 312 kB\nexplorer.exe                  5120 Console                    1     98 004 kB\nOUTLOOK.EXE                   7344 Console                    1    212 880 kB\ncmd.exe                       9012 Console                    1      4 120 kB")),
            "nbtstat" => {
                if !a.get(1).map(|x| x.eq_ignore_ascii_case("-n")).unwrap_or(false) {
                    return Some(r("\nVisar protokollstatistik och aktuella TCP/IP-anslutningar med NBT.\n\nNBTSTAT [-n] [-c] [-a fjärrnamn] [-A IP-adress]\n💡 Prova nbtstat -n"));
                }
                let cn2 = cip(&conf(self.me(st)));
                let ip = if cn2.is_empty() { "0.0.0.0".to_string() } else { cn2 };
                let up = id.to_uppercase();
                Some(r(format!(
                    "\nEthernet:\nNodens IP-adress: [{}] Scope-ID: []\n\n                Lokal NetBIOS-namntabell\n\n       Namn               Typ          Status\n    ---------------------------------------------\n    {}<00>  UNIK         Registrerad\n    NORDVIK        <00>  GRUPP        Registrerad\n    {}<20>  UNIK         Registrerad",
                    ip,
                    U::pad(&up, 15),
                    U::pad(&up, 15)
                )))
            }
            "get-netneighbor" => {
                let h = self.me(st);
                let rows: Vec<String> = h.arp.iter().map(|(ip, e)| format!("{}{}{}Reachable   ActiveStore", U::pad("12", 8), U::pad(ip, 40), U::pad(U::mac_dash(&e.mac), 20))).collect();
                Some(r(format!(
                    "\nifIndex IPAddress                               LinkLayerAddress    State       PolicyStore\n------- ---------                               ----------------    -----       -----------\n{}",
                    if rows.is_empty() { "(ARP-tabellen är tom: pinga något först)".to_string() } else { rows.join("\n") }
                )))
            }
            "clear-dnsclientcache" => {
                self.dns_cache.clear();
                Some(r(""))
            }
            "get-nettcpconnection" => {
                let Some(cn3) = conf(self.me(st)) else { return Some(r("")) };
                let ip = cn3.ip.clone().unwrap_or_default();
                Some(r(format!(
                    "\nLocalAddress                        LocalPort RemoteAddress                       RemotePort State       AppliedSetting\n------------                        --------- -------------                       ---------- -----       --------------\n{}{}{}{}Established Internet\n{}{}{}{}Listen\n{}{}{}{}Listen",
                    U::pad(&ip, 36), U::pad("50122", 10), U::pad("192.168.1.10", 36), U::pad("445", 11),
                    U::pad("0.0.0.0", 36), U::pad("135", 10), U::pad("0.0.0.0", 36), U::pad("0", 11),
                    U::pad("0.0.0.0", 36), U::pad("445", 10), U::pad("0.0.0.0", 36), U::pad("0", 11)
                )))
            }
            "get-date" => {
                let sec = 8 * 3600 + st.time.floor() as i64;
                Some(r(format!("\nden 29 september 2026 {:02}:{:02}:{:02}\n", (sec / 3600) % 24, (sec / 60) % 60, sec % 60)))
            }
            "get-command" => {
                let names = [
                    "Clear-DnsClientCache", "Get-DnsClientServerAddress", "Get-NetAdapter", "Get-NetIPAddress", "Get-NetIPConfiguration",
                    "Get-NetNeighbor", "Get-NetRoute", "Get-NetTCPConnection", "Resolve-DnsName", "Test-Connection", "Test-NetConnection",
                ];
                let rows: Vec<String> = names
                    .iter()
                    .map(|w| {
                        let src = if w.contains("Dns") {
                            "DnsClient"
                        } else if w.contains("Adapter") {
                            "NetAdapter"
                        } else if w.ends_with("Connection") && *w != "Get-NetTCPConnection" {
                            if *w == "Test-Connection" { "Microsoft.PowerShell.Management" } else { "NetTCPIP" }
                        } else {
                            "NetTCPIP"
                        };
                        format!("{}{}{}", U::pad("Cmdlet", 16), U::pad(w, 36), src)
                    })
                    .collect();
                Some(r(format!("\nCommandType     Name                                Source\n-----------     ----                                ------\n{}", rows.join("\n"))))
            }
            "telnet" => Some(r("'telnet' känns inte igen som ett internt eller externt kommando,\nkörbart program eller kommandofil.\n💡 Telnet-klienten är inte installerad i Windows. Testa en port med: Test-NetConnection <adress> -Port <nummer>")),
            "set" => Some(r(format!("COMPUTERNAME={}\nUSERDOMAIN=NORDVIK\nUSERNAME={}\nOS=Windows_NT\nPROCESSOR_ARCHITECTURE=AMD64", id.to_uppercase(), self.user))),
            _ => None,
        }
    }

    // ================================================================ Linux
    pub(super) fn lin_layer(&mut self, st: &mut State, line: &str) -> CmdResult {
        // !<n> kör om kommando nummer n i historiken
        let t = line.trim();
        if let Some(num) = t.strip_prefix('!') {
            if is_digits(num) {
                let n: usize = num.parse().unwrap_or(0);
                let hc = if n >= 1 { self.hist.get(n - 1).cloned() } else { None };
                let Some(hc) = hc else { return r(format!("bash: !{}: event not found", num)) };
                let mut rb = self.lin_top(st, &hc);
                rb.out = format!("{}\n{}", hc, rb.out);
                return rb;
            }
        }
        let mut a = splitargs(line);
        if a.first().map(|x| x == "sudo").unwrap_or(false) {
            a.remove(0);
        }
        if line.trim() == "?" {
            return self.lin_top(st, "help");
        }
        if a.is_empty() || line.contains('|') || line.contains("!!") {
            return self.lin_base(st, line);
        }
        let cmd = a[0].clone();
        if let Some(rr) = self.lin_extra(st, &a, &cmd, line) {
            return rr;
        }
        if cmd == "help" || (cmd == "man" && a.get(1).is_none()) {
            let mut r7 = self.lin_base(st, line);
            if cmd == "help" {
                r7.out.push_str("\n\nFler kommandon:\n  nmap -sn <nät/prefix>          vilka adresser svarar (ping-svep)\n  sudo tcpdump -i enp0s31f6 -c 10  fånga paket på nätverkskortet\n  iperf3 -c <server>             mät överföringshastigheten\n  ip -br a, ip -s link, ip -6 a, ip neigh flush all, dig -x <ip>\n  scp drift@<ip>:running-config backup.cfg   säkerhetskopia (kräver ip scp server enable)\n  curl -v <url>, ping -q -c 10 <ip>, ssh -V\n  nmap -p 22,23,80 <ip>          vilka portar är öppna\n  mtr -r <mål>                   förlust och svarstid per hopp\n  arping -c 3 <ip>               ARP i stället för ping (samma nät)\n  ethtool enp0s31f6              hastighet, duplex och länk\n  nmcli device status | show     nätverkskortets status\n  ss -tuln, netstat -rn, route -n\n  ssh-keygen -R <ip>             glöm en gammal värdnyckel\n  hostnamectl, pwd, cat /etc/os-release, cat ~/.ssh/known_hosts");
            }
            return r7;
        }
        let mut rl = self.lin_base(st, line);
        if rl.out.ends_with(": command not found") {
            if let Some(sl) = suggest(&cmd, LIN_ALL) {
                rl.out.push_str(&format!("\n💡 Menade du {}?", sl));
            }
        }
        rl
    }

    fn lin_extra(&mut self, st: &mut State, a: &[String], cmd: &str, line: &str) -> Option<CmdResult> {
        let id = self.dev.clone();
        let c = if ["ip", "tcpdump", "iperf3", "nmap", "ss", "netstat", "route", "nmcli", "dhclient", "arping"].contains(&cmd) { conf(self.me(st)) } else { None };
        let mac = self.me(st).mac();
        let a1 = a.get(1).map(|x| x.as_str()).unwrap_or("");
        let a2 = a.get(2).map(|x| x.as_str()).unwrap_or("");
        let fe = format!("fe80::{}:{}:{}:1", &mac[0..4], &mac[4..8], &mac[8..12]);
        if cmd == "man" {
            if let Some(t) = man2(&a1.to_lowercase()) {
                return Some(r(t));
            }
        }
        if cmd == "ip" && (a1 == "-br" || a1 == "-brief") && (a2.is_empty() || a2.starts_with('a')) {
            let upb = nic_up(st, &id);
            return Some(r(format!(
                "{}{}127.0.0.1/8 ::1/128\n{}{}{}{}/64",
                U::pad("lo", 17),
                U::pad("UNKNOWN", 15),
                U::pad("enp0s31f6", 17),
                U::pad(if upb { "UP" } else { "DOWN" }, 15),
                c.as_ref().map(|c| format!("{}/{} ", cip(&Some(c.clone())), U::mask_to_prefix(&cmask(c)).map(|p| p.to_string()).unwrap_or_default())).unwrap_or_default(),
                fe
            )));
        }
        if cmd == "ip" && a1 == "-s" && a2.starts_with('l') {
            let up3 = nic_up(st, &id);
            let rxb: i64 = if up3 { 184203311 + (st.time * 2100.0).floor() as i64 } else { 0 };
            return Some(r(format!(
                "2: enp0s31f6: <BROADCAST,MULTICAST,UP{}> mtu 1500 qdisc fq_codel state {} mode DEFAULT group default qlen 1000\n    link/ether {} brd ff:ff:ff:ff:ff:ff\n    RX:     bytes  packets errors dropped  missed   mcast\n    {}{}      0       0       0    1204\n    TX:     bytes  packets errors dropped carrier collsns\n    {}{}      0       0       0       0",
                if up3 { ",LOWER_UP" } else { "" },
                if up3 { "UP" } else { "DOWN" },
                U::mac_colon(&mac),
                U::pad_l(rxb, 13),
                U::pad_l(rxb / 820, 9),
                U::pad_l(rxb / 3, 13),
                U::pad_l(rxb / 2400, 9)
            )));
        }
        if cmd == "ip" && a1.starts_with("ma") {
            return Some(r("1:\tlo\n\tinet  224.0.0.1\n\tinet6 ff02::1\n2:\tenp0s31f6\n\tlink  01:00:5e:00:00:01\n\tlink  33:33:00:00:00:01\n\tinet  224.0.0.251\n\tinet  224.0.0.1\n\tinet6 ff02::fb\n\tinet6 ff02::1"));
        }
        if cmd == "ip" && a1.starts_with('n') && a2 == "flush" {
            st.dev_mut(&id).arp.clear();
            return Some(r(""));
        }
        if cmd == "ip" && a1 == "-6" && a2.starts_with('a') {
            return Some(r(format!("1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536 state UNKNOWN qlen 1000\n    inet6 ::1/128 scope host\n2: enp0s31f6: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 state UP qlen 1000\n    inet6 {}/64 scope link\n       valid_lft forever preferred_lft forever", fe)));
        }
        if cmd == "dig" && a1 == "-x" && !a2.is_empty() {
            let rn = reverse(st, a2);
            let rev: String = a2.split('.').rev().collect::<Vec<_>>().join(".");
            return Some(r(format!(
                "; <<>> DiG 9.18.24 <<>> -x {}\n;; ->>HEADER<<- opcode: QUERY, status: {}\n\n;; QUESTION SECTION:\n;{}.in-addr.arpa.\tIN\tPTR\n{}\n;; SERVER: 127.0.0.53#53(127.0.0.53) (UDP)",
                a2,
                if rn.is_some() { "NOERROR" } else { "NXDOMAIN" },
                rev,
                match &rn {
                    Some(n) => format!("\n;; ANSWER SECTION:\n{}.in-addr.arpa. 300 IN\tPTR\t{}.\n", rev, n),
                    None => String::new(),
                }
            )));
        }
        let ci = cip(&c);
        match cmd {
            "tcpdump" => {
                let Some(cc) = c.clone().filter(|_| nic_up(st, &id)) else { return Some(r("tcpdump: enp0s31f6: That device is not up")) };
                let mut n_c: i64 = 6;
                if let Some(p) = a.iter().position(|x| x == "-c") {
                    n_c = pint_or(a.get(p + 1), 6).min(20);
                }
                let gw = cc.gw.clone().filter(|g| !g.is_empty()).unwrap_or_else(|| ci.clone());
                let now = 8.0 * 3600.0 + st.time;
                let mut lines = vec![
                    "tcpdump: verbose output suppressed, use -v[v]... for full protocol decode".to_string(),
                    "listening on enp0s31f6, link-type EN10MB (Ethernet), snapshot length 262144 bytes".to_string(),
                ];
                let dns0 = cc.dns.get(0).cloned().unwrap_or_else(|| gw.clone());
                let gwmac = st.dev(&id).arp.get(&gw).map(|e| U::mac_colon(&e.mac)).unwrap_or_else(|| "5c:50:15:45:e2:af".into());
                let hh = U::hash(&id) as usize;
                for ii in 0..n_c as usize {
                    let sec = now + ii as f64 * 0.41;
                    let ts = format!(
                        "{:02}:{:02}:{:02}.{:06}",
                        ((sec / 3600.0).floor() as i64) % 24,
                        ((sec / 60.0).floor() as i64) % 60,
                        (sec.floor() as i64) % 60,
                        ((sec % 1.0) * 1e6).floor() as i64
                    );
                    let l = match (ii + hh) % 8 {
                        0 => format!("{} ARP, Request who-has {} tell {}, length 28", ts, gw, ci),
                        1 => format!("{} ARP, Reply {} is-at {}, length 46", ts, gw, gwmac),
                        2 => format!("{} IP {} > 224.0.0.10: CDPv2/LLDP announce, length 40", ts, gw),
                        3 => format!("{} STP 802.1d, Config, Flags [none], bridge-id 8063.00:1a:a1:7d:d1:bd.8008, length 43", ts),
                        4 => format!("{} IP {}.41022 > {}.53: 2471+ A? filserver.nordvik.example. (43)", ts, ci, dns0),
                        5 => format!("{} IP {}.53 > {}.41022: 2471* 1/0/0 A 192.168.1.10 (59)", ts, dns0, ci),
                        6 => format!("{} IP 192.168.1.16.123 > {}.123: NTPv4, Server, length 48", ts, ci),
                        _ => format!("{} IP 192.168.1.17.514 > 255.255.255.255.514: SYSLOG local7.notice, length 92", ts),
                    };
                    lines.push(l);
                }
                lines.push(format!("{} packets captured", n_c));
                lines.push(format!("{} packets received by filter", n_c));
                lines.push("0 packets dropped by kernel".into());
                Some(rd(lines.join("\n"), 1200))
            }
            "iperf3" => {
                let srv = a.iter().position(|x| x == "-c").and_then(|i| a.get(i + 1)).cloned();
                let Some(srv) = srv else { return Some(r("iperf3: parameter error - must either be a client (-c) or server (-s)\nAnvänd: iperf3 -c <server>   (filservern 192.168.1.10 kör en iperf3-server)")) };
                let ip = match self.resolve_for(st, &srv) {
                    Ok(ip) => ip,
                    Err(_) => return Some(r("iperf3: error - unable to connect to server: Name or service not known")),
                };
                let pr = S::ping(st, &id, &ip, PingOpts::default());
                if !pr.ok {
                    return Some(rd("iperf3: error - unable to connect to server: Connection timed out", 3000));
                }
                let pt = self.port_of(st);
                let neg = pt.as_ref().and_then(|p| p.neg.clone());
                let mb = neg.as_ref().filter(|n| n.speed != 0).map(|n| n.speed as f64).unwrap_or(1000.0);
                let half = neg.as_ref().map(|n| n.duplex == "half").unwrap_or(false);
                let mut rate = mb * (if half { 0.35 } else { 0.94 }) * (1.0 - pr.loss * 3.0) * (if pr.tunnel { 0.6 } else { 1.0 });
                if ip.starts_with("192.168.2.") {
                    rate = rate.min(95.0);
                }
                let mut out2: Vec<String> = vec![
                    format!("Connecting to host {}, port 5201", ip),
                    format!("[  5] local {} port 48122 connected to {} port 5201", ci, ip),
                    "[ ID] Interval           Transfer     Bitrate         Retr".into(),
                ];
                for si in 0..5usize {
                    let rr2 = (rate * (0.96 + ((U::hash(&format!("{}{}", srv, si)) % 8) as f64 / 100.0))).max(0.5);
                    out2.push(format!(
                        "[  5]   {}.00-{}.00   sec  {} MBytes  {} Mbits/sec  {}",
                        si,
                        si + 1,
                        U::pad_l(format!("{:.1}", rr2 / 8.0), 5),
                        U::pad_l(format!("{:.0}", rr2), 4),
                        U::pad_l(if half { 40 + si * 7 } else { 0 }, 3)
                    ));
                }
                out2.push("- - - - - - - - - - - - - - - - - - - - - - - - -".into());
                out2.push(format!(
                    "[  5]   0.00-5.00   sec  {} MBytes  {} Mbits/sec                  sender",
                    U::pad_l(format!("{:.0}", rate * 5.0 / 8.0), 5),
                    U::pad_l(format!("{:.0}", rate), 4)
                ));
                out2.push(String::new());
                out2.push(format!("iperf Done.{}", if half { "\n💡 Många omsändningar (Retr) och låg fart: kontrollera duplex på switchporten." } else { "" }));
                Some(rs(out2.into_iter().enumerate().map(|(i, l)| ck(format!("{}\n", l), if i > 2 && i < 8 { 600 } else { 50 })).collect()))
            }
            "nmap" => Some(self.nmap(st, a, &c)),
            "mtr" => {
                let Some(t2) = a[1..].iter().find(|x| !x.starts_with('-') && !is_digits(x)).cloned() else { return Some(r("mtr: No hostname specified")) };
                let ip = match self.resolve_for(st, &t2) {
                    Ok(ip) => ip,
                    Err(_) => return Some(r(format!("mtr: Failed to resolve host: {}: Name or service not known", t2))),
                };
                let (fwd, full) = S::traceroute(st, &id, &ip, None);
                let mut hops: Vec<String> = fwd.hops.iter().map(|x| x.ip.clone()).collect();
                if full.ok && hops.last() != Some(&ip) {
                    hops.push(ip.clone());
                }
                let mut o = vec![
                    "Start: 2026-09-29T08:00:00+0200".to_string(),
                    "HOST: laptop                      Loss%   Snt   Last   Avg  Best  Wrst StDev".to_string(),
                ];
                for (i, h) in hops.iter().enumerate() {
                    let f = i as f64 * 0.7;
                    o.push(format!(
                        "{}.|-- {}{}{}{}{}{}{}{}",
                        U::pad_l(i + 1, 3),
                        U::pad(h, 28),
                        U::pad_l("0.0%", 6),
                        U::pad_l(10, 6),
                        U::pad_l(format!("{:.1}", 0.5 + f), 7),
                        U::pad_l(format!("{:.1}", 0.6 + f), 6),
                        U::pad_l(format!("{:.1}", 0.4 + f), 6),
                        U::pad_l(format!("{:.1}", 1.2 + f), 6),
                        U::pad_l("0.2", 6)
                    ));
                }
                if !full.ok {
                    o.push(format!("{}.|-- ???                          100.0    10    0.0   0.0   0.0   0.0   0.0", U::pad_l(hops.len() + 1, 3)));
                }
                Some(rd(o.join("\n"), 1500))
            }
            "ss" | "netstat" => {
                if line.contains("-r") && cmd == "netstat" {
                    let hdr = "Kernel IP routing table\nDestination     Gateway         Genmask         Flags   MSS Window  irtt Iface";
                    let Some(cc) = &c else { return Some(r(hdr)) };
                    let ip = cc.ip.clone().unwrap_or_default();
                    let mask = cmask(cc);
                    return Some(r(format!(
                        "{}\n{}{}{}{}U         0 0          0 enp0s31f6",
                        hdr,
                        cc.gw.as_ref().filter(|g| !g.is_empty()).map(|g| format!("{}{}{}UG        0 0          0 enp0s31f6\n", U::pad("0.0.0.0", 16), U::pad(g, 16), U::pad("0.0.0.0", 16))).unwrap_or_default(),
                        U::pad(U::network(&ip, &mask), 16),
                        U::pad("0.0.0.0", 16),
                        U::pad(&mask, 16)
                    )));
                }
                let fl: String = a[1..].iter().filter(|x| x.starts_with('-')).cloned().collect::<Vec<_>>().join("");
                if cmd == "ss" && fl.contains('t') && !fl.contains('l') {
                    return Some(r("State    Recv-Q   Send-Q     Local Address:Port      Peer Address:Port  Process"));
                }
                Some(r(if cmd == "ss" {
                    "Netid  State   Recv-Q  Send-Q   Local Address:Port    Peer Address:Port  Process\nudp    UNCONN  0       0        127.0.0.53%lo:53           0.0.0.0:*\ntcp    LISTEN  0       4096     127.0.0.53%lo:53           0.0.0.0:*\ntcp    LISTEN  0       128          127.0.0.1:631          0.0.0.0:*"
                } else {
                    "Active Internet connections (only servers)\nProto Recv-Q Send-Q Local Address           Foreign Address         State\ntcp        0      0 127.0.0.53:53           0.0.0.0:*               LISTEN\ntcp        0      0 127.0.0.1:631           0.0.0.0:*               LISTEN\nudp        0      0 127.0.0.53:53           0.0.0.0:*"
                }))
            }
            "route" => {
                let hdr = "Kernel IP routing table\nDestination     Gateway         Genmask         Flags Metric Ref    Use Iface";
                let Some(cc) = &c else { return Some(r(hdr)) };
                let ip = cc.ip.clone().unwrap_or_default();
                let mask = cmask(cc);
                Some(r(format!(
                    "{}\n{}{}{}{}U     100    0        0 enp0s31f6",
                    hdr,
                    cc.gw.as_ref().filter(|g| !g.is_empty()).map(|g| format!("{}{}{}UG    100    0        0 enp0s31f6\n", U::pad("0.0.0.0", 16), U::pad(g, 16), U::pad("0.0.0.0", 16))).unwrap_or_default(),
                    U::pad(U::network(&ip, &mask), 16),
                    U::pad("0.0.0.0", 16),
                    U::pad(&mask, 16)
                )))
            }
            "ethtool" => {
                let pt = self.port_of(st);
                let up = nic_up(st, &id);
                let neg = pt.and_then(|p| p.neg);
                let sp = neg.as_ref().map(|n| n.speed);
                let dp = neg.as_ref().map(|n| n.duplex.clone());
                Some(r(format!(
                    "Settings for {}:\n\tSupported ports: [ TP ]\n\tSupported link modes:   10baseT/Half 10baseT/Full\n\t                        100baseT/Half 100baseT/Full\n\t                        1000baseT/Full\n\tAuto-negotiation: on\n\tSpeed: {}\n\tDuplex: {}\n\tPort: Twisted Pair\n\tMDI-X: on (auto)\n\tLink detected: {}",
                    if a1.is_empty() { "enp0s31f6" } else { a1 },
                    match (up, sp) {
                        (true, Some(s)) if s != 0 => format!("{}Mb/s", s),
                        _ => "Unknown!".to_string(),
                    },
                    match (up, dp) {
                        (true, Some(d)) => if d == "half" { "Half".to_string() } else { "Full".to_string() },
                        _ => "Unknown! (255)".to_string(),
                    },
                    if up { "yes" } else { "no" }
                )))
            }
            "nmcli" => {
                let up2 = nic_up(st, &id);
                if line.contains("show") {
                    return Some(r(format!(
                        "GENERAL.DEVICE:                         enp0s31f6\nGENERAL.TYPE:                           ethernet\nGENERAL.HWADDR:                         {}\nGENERAL.STATE:                          {}\nWIRED-PROPERTIES.CARRIER:               {}\nIP4.ADDRESS[1]:                         {}\nIP4.GATEWAY:                            {}\nIP4.DNS[1]:                             {}",
                        U::mac_colon(&mac).to_uppercase(),
                        if up2 { "100 (connected)" } else { "20 (unavailable)" },
                        if up2 { "on" } else { "off" },
                        c.as_ref().map(|c| format!("{}/{}", cip(&Some(c.clone())), U::mask_to_prefix(&cmask(c)).map(|p| p.to_string()).unwrap_or_default())).unwrap_or_default(),
                        c.as_ref().and_then(|c| c.gw.clone()).filter(|g| !g.is_empty()).unwrap_or_else(|| "--".into()),
                        c.as_ref().and_then(|c| c.dns.get(0).cloned()).unwrap_or_default()
                    )));
                }
                Some(r(format!(
                    "DEVICE     TYPE      STATE         CONNECTION\nenp0s31f6  ethernet  {}{}\nlo         loopback  connected (externally)  lo",
                    U::pad(if up2 { "connected" } else { "unavailable" }, 14),
                    if up2 { "Nordvik drift" } else { "--" }
                )))
            }
            "dhclient" => {
                let dhcp = self.me(st).nic.as_ref().map(|n| n.dhcp).unwrap_or(false);
                if !dhcp {
                    return Some(r(format!("Laptopen har en statisk adress ({}) i drift-VLAN:et, så den frågar ingen DHCP-server.\nTips: datorerna på kontoret använder DHCP – där fungerar ipconfig /renew.", ci)));
                }
                if a.iter().any(|x| x == "-r") {
                    S::release(st, &id);
                    return Some(r(""));
                }
                S::dhcp(st, &id);
                Some(rd("", 1200))
            }
            "arping" => {
                let Some(t3) = a[1..].iter().find(|x| !x.starts_with('-') && !is_digits(x)).cloned().filter(|x| U::is_ip(x)) else { return Some(r("Usage: arping [-c count] <ip>")) };
                let same = c.as_ref().map(|c| U::same_subnet(&t3, &cip(&Some(c.clone())), &cmask(c))).unwrap_or(false);
                if !same {
                    return Some(r(format!("arping: {} finns inte i det lokala nätet – ARP fungerar bara inom samma subnät.", t3)));
                }
                let p3 = S::ping(st, &id, &t3, PingOpts::default());
                let ok = p3.ok || p3.reason.as_deref() == Some("acl") || p3.reason.as_deref() == Some("refused");
                let mac_t = if ok { st.dev(&id).arp.get(&t3).map(|e| U::mac_colon(&e.mac)) } else { None };
                let mut lines = vec![format!("ARPING {}", t3)];
                for k in 0..3 {
                    lines.push(match &mac_t {
                        Some(m) => format!("60 bytes from {} ({}): index={} time={:.3} msec", m, t3, k, 0.3 + k as f64 * 0.1),
                        None => "Timeout".into(),
                    });
                }
                lines.push(String::new());
                lines.push(format!("--- {} statistics ---", t3));
                lines.push(format!("3 packets transmitted, {} packets received, {}% unanswered (0 extra)", if mac_t.is_some() { 3 } else { 0 }, if mac_t.is_some() { "0" } else { "100" }));
                Some(rd(lines.join("\n"), if mac_t.is_some() { 900 } else { 3000 }))
            }
            "ssh-keygen" => {
                let Some(hi) = a.iter().position(|x| x == "-R") else { return Some(r("usage: ssh-keygen -R hostname")) };
                let Some(host) = a.get(hi + 1).cloned() else { return Some(r("usage: ssh-keygen -R hostname")) };
                let had = self.known.iter().any(|k| *k == host);
                self.known.retain(|k| *k != host);
                Some(r(if had {
                    format!("# Host {} found: line 1\n/home/tekniker/.ssh/known_hosts updated.\nOriginal contents retained as /home/tekniker/.ssh/known_hosts.old", host)
                } else {
                    format!("Host {} not found in /home/tekniker/.ssh/known_hosts", host)
                }))
            }
            "scp" => Some(self.scp(st, a)),
            "curl" => {
                if a.iter().any(|x| x == "-v") {
                    let uu = a[1..].iter().find(|x| !x.starts_with('-')).cloned().unwrap_or_default();
                    // ^(?:https?:\/\/)?([^\/:]+)(?::(\d+))?(\/\S*)?
                    let mut rest: &str = &uu;
                    if starts_ci(rest, "https://") {
                        rest = &rest[8..];
                    } else if starts_ci(rest, "http://") {
                        rest = &rest[7..];
                    }
                    let host: String = rest.chars().take_while(|c| *c != '/' && *c != ':').collect();
                    if host.is_empty() {
                        return None;
                    }
                    let mut after = &rest[host.len()..];
                    let mut mport: Option<String> = None;
                    if let Some(p) = after.strip_prefix(':') {
                        let d: String = p.chars().take_while(|c| c.is_ascii_digit()).collect();
                        if !d.is_empty() {
                            after = &p[d.len()..];
                            mport = Some(d);
                        }
                    }
                    let path = if after.starts_with('/') { after.split_whitespace().next().unwrap_or("/").to_string() } else { "/".to_string() };
                    let rv = self.resolve_for(st, &host);
                    let base = self.lin_base(st, &format!("curl {}", uu));
                    let ip = match rv {
                        Err(_) => return Some(rd(format!("* Could not resolve host: {}\n{}", host, base.out), base.delay)),
                        Ok(ip) => ip,
                    };
                    let portv = mport.unwrap_or_else(|| if starts_ci(&uu, "https") { "443".into() } else { "80".into() });
                    let okv = !base.out.contains("curl: (");
                    return Some(rd(
                        format!(
                            "*   Trying {}:{}...\n{}{}{}",
                            ip,
                            portv,
                            if okv {
                                format!("* Connected to {} ({}) port {}\n> GET {} HTTP/1.1\n> Host: {}\n> User-Agent: curl/8.5.0\n> Accept: */*\n>\n< HTTP/1.1 200 OK\n< Content-Type: text/html; charset=utf-8\n<\n", host, ip, portv, path, host)
                            } else {
                                String::new()
                            },
                            base.out,
                            if okv { format!("\n* Connection #0 to host {} left intact", host) } else { String::new() }
                        ),
                        base.delay,
                    ));
                }
                None
            }
            "ping" => {
                if a.iter().any(|x| x == "-q") {
                    let rest: Vec<String> = a.iter().filter(|x| *x != "-q").cloned().collect();
                    let rq = self.lin_base(st, &rest.join(" "));
                    if !rq.stream.is_empty() {
                        let txt: String = rq.stream.iter().map(|x| x.text.clone()).collect();
                        let lines2: Vec<&str> = txt.split('\n').collect();
                        let idx = lines2.iter().position(|l| l.contains("statistics")).map(|i| i as i64 - 1).unwrap_or(-2);
                        let start = if idx < 0 { (lines2.len() as i64 + idx).max(0) as usize } else { idx as usize };
                        return Some(rd(format!("{}\n{}", lines2[0], lines2[start.min(lines2.len())..].join("\n")), 1500));
                    }
                    return Some(rq);
                }
                None
            }
            "ssh" => {
                if a1 == "-V" {
                    return Some(r("OpenSSH_9.6p1 Ubuntu-3ubuntu13.5, OpenSSL 3.0.13 30 Jan 2024"));
                }
                None
            }
            "hostnamectl" => Some(r(" Static hostname: laptop\n       Icon name: computer-laptop\n         Chassis: laptop 💻\n      Machine ID: 4f1c2d9e8b7a4c3e9f0a1b2c3d4e5f60\n Operating System: Ubuntu 24.04 LTS\n          Kernel: Linux 6.8.0-45-generic\n    Architecture: x86-64")),
            "free" => Some(r("               total        used        free      shared  buff/cache   available\nMem:            15Gi       3,2Gi       8,9Gi       412Mi       3,4Gi        11Gi\nSwap:          2,0Gi          0B       2,0Gi")),
            "df" => Some(r("Filesystem      Size  Used Avail Use% Mounted on\ntmpfs           1,6G  2,1M  1,6G   1% /run\n/dev/nvme0n1p2  468G   61G  384G  14% /\n/dev/nvme0n1p1  1,1G  6,2M  1,1G   1% /boot/efi")),
            "ps" => Some(r("    PID TTY          TIME CMD\n   2231 pts/0    00:00:00 bash\n   2402 pts/0    00:00:00 ps")),
            "lsof" => Some(r("COMMAND    PID     USER   FD   TYPE DEVICE SIZE/OFF NODE NAME\nsystemd-r  611 systemd-resolve 13u IPv4  21345      0t0  UDP 127.0.0.53:53\nsshd       902     root    3u  IPv4  23011      0t0  TCP *:22 (LISTEN)\nchronyd    744  _chrony    5u  IPv4  22987      0t0  UDP 127.0.0.1:323")),
            "w" => {
                let sec2 = 8 * 3600 + st.time.floor() as i64;
                Some(r(format!(" {:02}:{:02}:00 up 3 days,  2:14,  1 user,  load average: 0,08, 0,12, 0,10\nUSER     TTY      FROM             LOGIN@   IDLE   JCPU   PCPU WHAT\ntekniker pts/0    -                08:00    0.00s  0.04s  0.00s w", (sec2 / 3600) % 24, (sec2 / 60) % 60)))
            }
            "last" => Some(r("tekniker pts/0        :0               Tue Sep 29 08:00   still logged in\ntekniker pts/0        :0               Mon Sep 28 07:58 - 16:31  (08:33)\nreboot   system boot  6.8.0-45-generic Sat Sep 26 06:12   still running\n\nwtmp begins Tue Sep  1 07:55:02 2026")),
            "env" => Some(r("USER=tekniker\nHOME=/home/tekniker\nSHELL=/bin/bash\nLANG=sv_SE.UTF-8\nPATH=/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin\nTERM=xterm-256color\nHOSTNAME=laptop")),
            "ping6" => {
                let Some(t7) = a[1..].iter().find(|x| !x.starts_with('-') && !is_digits(x)).cloned() else { return Some(r("ping6: usage error: Destination address required")) };
                if t7 == "::1" || t7 == "localhost" {
                    return Some(rs((0..4)
                        .map(|i| {
                            ck(
                                format!("{}\n", if i > 0 { format!("64 bytes from ::1: icmp_seq={} ttl=64 time=0.0{} ms", i, 3 + i) } else { "PING ::1(::1) 56 data bytes".to_string() }),
                                if i > 0 { 1000 } else { 50 },
                            )
                        })
                        .collect()));
                }
                Some(r("ping6: connect: Network is unreachable\n💡 Nordviks nät kör bara IPv4. Datorn har bara en länklokal IPv6-adress (fe80::), som inte kan nå andra nät."))
            }
            "ssh-copy-id" => {
                let t6 = a[1..].iter().find(|x| !x.starts_with('-')).cloned().unwrap_or_default();
                Some(r(format!(
                    "/usr/bin/ssh-copy-id: INFO: attempting to log in with the new key(s)\n/usr/bin/ssh-copy-id: ERROR: {} is a Cisco IOS device: no shell to install authorized_keys in\n💡 På Cisco-enheter läggs nycklar in med ip ssh pubkey-chain i konfigurationsläget. I spelet räcker lösenord.",
                    if t6.is_empty() { "host".to_string() } else { t6 }
                )))
            }
            "wget" => {
                let Some(wu) = a[1..].iter().find(|x| !x.starts_with('-')).cloned() else { return Some(r("wget: missing URL\nUsage: wget [OPTION]... [URL]...")) };
                let wb = self.lin_base(st, &format!("curl {}", wu));
                let mut rest: &str = &wu;
                if starts_ci(rest, "https://") {
                    rest = &rest[8..];
                } else if starts_ci(rest, "http://") {
                    rest = &rest[7..];
                }
                let wm: String = rest.chars().take_while(|c| *c != '/' && *c != ':').collect();
                let wsec = 8 * 3600 + st.time.floor() as i64;
                let stamp = format!(
                    "--2026-09-29 {:02}:{:02}:00--  {}",
                    (wsec / 3600) % 24,
                    (wsec / 60) % 60,
                    if starts_ci(&wu, "http:") || starts_ci(&wu, "https:") { wu.clone() } else { format!("http://{}/", wu) }
                );
                if wb.out.contains("Could not resolve") {
                    return Some(rd(format!("{}\nResolving {} ({})... failed: Name or service not known.\nwget: unable to resolve host address \u{2018}{}\u{2019}", stamp, wm, wm, wm), wb.delay));
                }
                let wip = self.resolve_for(st, &wm).unwrap_or_else(|_| wm.clone());
                if wb.out.contains("curl: (") {
                    let low = wb.out.to_lowercase();
                    return Some(rd(
                        format!("{}\nConnecting to {} ({})|{}|:80... failed: {}.", stamp, wm, wm, wip, if low.contains("timed out") || low.contains("timeout") { "Connection timed out" } else { "Connection refused" }),
                        wb.delay,
                    ));
                }
                let len = wb.out.chars().count();
                self.files.insert("index.html".into(), wb.out.clone());
                Some(rd(
                    format!(
                        "{}\nConnecting to {} ({})|{}|:80... connected.\nHTTP request sent, awaiting response... 200 OK\nLength: {} ({:.1}K) [text/html]\nSaving to: \u{2018}index.html\u{2019}\n\nindex.html          100%[===================>]   {}  --.-KB/s    in 0s\n\n\u{2018}index.html\u{2019} saved [{}/{}]\n💡 Visa filen med cat index.html",
                        stamp, wm, wm, wip, len, len as f64 / 1024.0, len, len, len
                    ),
                    wb.delay,
                ))
            }
            "history" => {
                if is_digits(a1) {
                    let base = self.lin_base(st, line);
                    let hh: Vec<&str> = base.out.split('\n').collect();
                    let n: usize = a1.parse().unwrap_or(0);
                    let start = if n == 0 { 0 } else { hh.len().saturating_sub(n) };
                    return Some(r(hh[start..].join("\n")));
                }
                None
            }
            "pwd" => Some(r("/home/tekniker")),
            "cd" => Some(r("")),
            "cat" => {
                if let Some(f) = self.files.get(a1) {
                    return Some(r(f.clone()));
                }
                if a1 == "~/.ssh/known_hosts" || a1 == "/home/tekniker/.ssh/known_hosts" || a1 == ".ssh/known_hosts" {
                    return Some(r(self.known.iter().map(|k| format!("{} ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQC{}...", k, to_base36(U::hash(k)))).collect::<Vec<_>>().join("\n")));
                }
                if a1 == "/etc/os-release" {
                    return Some(r("PRETTY_NAME=\"Ubuntu 24.04 LTS\"\nNAME=\"Ubuntu\"\nVERSION_ID=\"24.04\"\nVERSION=\"24.04 LTS (Noble Numbat)\"\nID=ubuntu"));
                }
                None
            }
            "ls" => {
                if a1.is_empty() && !self.files.is_empty() {
                    let mut lr = self.lin_base(st, line);
                    lr.out = format!("{}{}", if lr.out.is_empty() { String::new() } else { format!("{}  ", lr.out) }, self.files.keys().cloned().collect::<Vec<_>>().join("  "));
                    return Some(lr);
                }
                if a1.contains(".ssh") {
                    return Some(r(format!("known_hosts{}", if self.known.is_empty() { "  (tom)" } else { "" })));
                }
                None
            }
            _ => None,
        }
    }

    fn nmap(&mut self, st: &mut State, a: &[String], c: &Option<IpConf>) -> CmdResult {
        let id = self.dev.clone();
        let is_list = |x: &str| !x.is_empty() && x.split(',').all(|p| is_digits(p));
        let target = a[1..].iter().find(|x| !x.starts_with('-') && !is_list(x)).cloned();
        let Some(target) = target else {
            return r("Nmap 7.94SVN ( https://nmap.org )\nUsage: nmap [Scan Type(s)] [Options] {target specification}\n  nmap -sn 192.168.1.192/26\n  nmap -p 22,23,80 192.168.1.193");
        };
        let head = format!("Starting Nmap 7.94SVN ( https://nmap.org ) at 2026-09-29 08:{:02} CEST\n", ((st.time / 60.0).floor() as i64) % 60);
        let myip = cip(c);
        if a.iter().any(|x| x == "-sn") {
            let hosts = match subnet_hosts(&target) {
                Some(None) => return r(format!("{}Det här labbet skannar högst ett /24-nät åt gången. Prova till exempel 192.168.1.192/26.", head)),
                Some(Some(h)) => h,
                None => vec![target.clone()],
            };
            let mut up = vec![];
            let mut stream = vec![ck(head, 200)];
            for ip in &hosts {
                if c.is_some() && *ip == myip {
                    up.push(ip.clone());
                    continue;
                }
                let p = S::ping(st, &id, ip, PingOpts::default());
                if p.ok {
                    up.push(ip.clone());
                }
            }
            for ip in &up {
                let nm = if c.is_some() && *ip == myip { Some("laptop".to_string()) } else { device_name(st, ip) };
                stream.push(ck(
                    format!(
                        "Nmap scan report for {}\nHost is up (0.00{}s latency).\n",
                        match &nm {
                            Some(n) => format!("{} ({})", n, ip),
                            None => ip.clone(),
                        },
                        U::hash(ip) % 9 + 1
                    ),
                    120,
                ));
            }
            stream.push(ck(
                format!(
                    "Nmap done: {} IP address{} ({} host{} up) scanned in {:.2} seconds",
                    hosts.len(),
                    if hosts.len() > 1 { "es" } else { "" },
                    up.len(),
                    if up.len() == 1 { "" } else { "s" },
                    1.0 + hosts.len() as f64 * 0.03
                ),
                500,
            ));
            return rs(stream);
        }
        let ports: Vec<i64> = match a.iter().position(|x| x == "-p").and_then(|pi| a.get(pi + 1)) {
            Some(spec) => spec.split(',').filter_map(|x| x.trim().parse::<f64>().ok()).filter(|v| *v != 0.0 && v.is_finite()).map(|v| v as i64).collect(),
            None => vec![22, 23, 53, 80, 443, 445],
        };
        let ip = match self.resolve_for(st, &target) {
            Ok(ip) => ip,
            Err(_) => {
                return r(format!(
                    "{}Failed to resolve \"{}\".\nWARNING: No targets were specified, so 0 hosts scanned.\nNmap done: 0 IP addresses (0 hosts up) scanned in 0.05 seconds",
                    head, target
                ))
            }
        };
        let alive = S::ping(st, &id, &ip, PingOpts::default());
        let dn = device_name(st, &ip);
        let first = format!("Nmap scan report for {}", match &dn { Some(n) => format!("{} ({})", n, ip), None => ip.clone() });
        if !alive.ok && alive.reason.as_deref() != Some("acl") {
            return rd(
                format!("{}\nNote: Host seems down. If it is really up, but blocking our ping probes, try -Pn\nNmap done: 1 IP address (0 hosts up) scanned in 3.04 seconds", head.trim_end_matches('\n')),
                2500,
            );
        }
        let mut rows = vec![format!("{}{}", head, first), "Host is up (0.0011s latency).".to_string(), String::new(), "PORT     STATE    SERVICE".to_string()];
        for pn in ports.iter().take(20) {
            let t = S::ping(st, &id, &ip, PingOpts::tcp(*pn));
            let state = if t.ok { "open" } else if t.reason.as_deref() == Some("refused") { "closed" } else { "filtered" };
            rows.push(format!("{}{}{}", U::pad(format!("{}/tcp", pn), 9), U::pad(state, 9), portname(*pn)));
        }
        rows.push(String::new());
        rows.push("Nmap done: 1 IP address (1 host up) scanned in 1.21 seconds".into());
        rd(rows.join("\n"), 900)
    }

    fn scp(&mut self, st: &mut State, a: &[String]) -> CmdResult {
        let id = self.dev.clone();
        let usage = "usage: scp drift@<ip>:running-config backup.cfg\n💡 Routern måste ha ip scp server enable för att lämna ut filer.";
        let (Some(src), Some(dst)) = (a.get(1), a.get(2)) else { return r(usage) };
        // ^(?:([^@\s]+)@)?([^:\s]+):(\S+)$
        let Some((hp, path)) = src.split_once(':') else { return r(usage) };
        let (usr, host) = match hp.split_once('@') {
            Some((u, h)) if !u.is_empty() => (Some(u.to_string()), h.to_string()),
            _ => (None, hp.to_string()),
        };
        if host.is_empty() || path.is_empty() {
            return r(usage);
        }
        let ip = match self.resolve_for(st, &host) {
            Ok(ip) => ip,
            Err(_) => return r(format!("ssh: Could not resolve hostname {}: Name or service not known", host)),
        };
        let p9 = S::ping(st, &id, &ip, PingOpts::tcp(22));
        if !p9.ok {
            let refused = p9.reason.as_deref() == Some("refused");
            return rd(format!("ssh: connect to host {} port 22: {}", host, if refused { "Connection refused" } else { "Connection timed out" }), if refused { 300 } else { 3000 });
        }
        let d9 = S::get(st);
        let mut tid = None;
        for e in d9.eps.values() {
            if e.ip.as_deref() == Some(ip.as_str()) && e.up && st.devices.get(&e.dev).map(|d| d.os == Os::Ios).unwrap_or(false) {
                tid = Some(e.dev.clone());
            }
        }
        let Some(dev9) = tid else { return r(format!("scp: {}: No such file or directory", path)) };
        let user9 = usr.unwrap_or_else(|| "tekniker".into());
        self.set_pending(
            format!("{}@{}'s password: ", user9, host),
            true,
            PendKind::ScpPass { host, dev: dev9, user: user9, path: path.to_string(), dst: dst.clone() },
        );
        r("")
    }

    pub(super) fn scp_answer(&mut self, st: &mut State, host: &str, dev: &str, user: &str, path: &str, dst: &str, pw: &str) -> CmdResult {
        let (u9, scp_on, startup) = {
            let c = st.dev(dev).cfg();
            (c.users.get(user).cloned(), c.scp_server, st.dev(dev).startup.clone())
        };
        if u9.as_ref().map(|u| u.plain != pw).unwrap_or(true) {
            return r(format!(
                "Permission denied, please try again.{}",
                match &u9 {
                    Some(u) if u.by_game => format!("\n💡 Lösenordet för {} är {}.", user, u.plain),
                    _ => String::new(),
                }
            ));
        }
        if !scp_on {
            return r("scp: Connection closed\n💡 Routern lämnar bara ut filer med scp om ip scp server enable är konfigurerat.");
        }
        let file: Option<String> = if path.contains("running") {
            Some(crate::cli::ios_show::running_config(st, dev))
        } else if path.contains("startup") {
            startup.map(|s| crate::cli::ios_show::running_config_of(st, dev, &s))
        } else {
            None
        };
        let Some(file) = file else { return r(format!("scp: {}: No such file or directory", path)) };
        let name = if dst == "." { path.to_string() } else { dst.to_string() };
        let len = file.chars().count();
        self.files.insert(name, file);
        let _ = host;
        rd(format!("{}100% {}   {:.1}KB/s   00:00", U::pad(path, 40), U::pad_l(len, 6), len as f64 / 1024.0 / 0.3), 700)
    }
}
