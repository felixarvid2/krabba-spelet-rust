//! Gemensamma hjälpfunktioner: IP-räkning, interfacenamn, VLAN-listor, hash.

pub fn ip_to_int(ip: &str) -> Option<u32> {
    let p: Vec<&str> = ip.split('.').collect();
    if p.len() != 4 {
        return None;
    }
    let mut n: u32 = 0;
    for part in p {
        if part.is_empty() || part.len() > 3 || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let v: u32 = part.parse().ok()?;
        if v > 255 {
            return None;
        }
        n = n.wrapping_mul(256).wrapping_add(v);
    }
    Some(n)
}

/// Som ip_to_int men ger 0 för ogiltiga adresser (som JS gör med null i bitoperationer).
pub fn ipn(ip: &str) -> u32 {
    ip_to_int(ip).unwrap_or(0)
}

pub fn int_to_ip(n: u32) -> String {
    format!("{}.{}.{}.{}", (n >> 24) & 255, (n >> 16) & 255, (n >> 8) & 255, n & 255)
}

pub fn is_ip(s: &str) -> bool {
    ip_to_int(s).is_some()
}

pub fn mask_to_prefix(mask: &str) -> Option<u32> {
    let m = ip_to_int(mask)?;
    let mut bits = 0;
    let mut seen_zero = false;
    for i in (0..32).rev() {
        if m & (1 << i) != 0 {
            if seen_zero {
                return None;
            }
            bits += 1;
        } else {
            seen_zero = true;
        }
    }
    Some(bits)
}

pub fn prefix_to_mask(p: u32) -> String {
    if p == 0 {
        return "0.0.0.0".into();
    }
    int_to_ip(0xffff_ffffu32 << (32 - p.min(32)))
}

pub fn is_valid_mask(mask: &str) -> bool {
    mask_to_prefix(mask).is_some()
}

pub fn network(ip: &str, mask: &str) -> String {
    int_to_ip(ipn(ip) & ipn(mask))
}

pub fn broadcast(ip: &str, mask: &str) -> String {
    int_to_ip((ipn(ip) & ipn(mask)) | !ipn(mask))
}

pub fn same_subnet(a: &str, b: &str, mask: &str) -> bool {
    let m = ipn(mask);
    (ipn(a) & m) == (ipn(b) & m)
}

pub fn in_net(ip: &str, net: &str, mask: &str) -> bool {
    same_subnet(ip, net, mask)
}

/// Wildcard-matchning för ACL: 0-bitar måste stämma.
pub fn wild_match(ip: &str, base: &str, wild: &str) -> bool {
    let care = !ipn(wild);
    (ipn(ip) & care) == (ipn(base) & care)
}

pub fn is_private(ip: &str) -> bool {
    if ip_to_int(ip).is_none() {
        return true;
    }
    in_net(ip, "10.0.0.0", "255.0.0.0")
        || in_net(ip, "172.16.0.0", "255.240.0.0")
        || in_net(ip, "192.168.0.0", "255.255.0.0")
        || in_net(ip, "169.254.0.0", "255.255.0.0")
}

const IF_TYPES: &[(&str, &str)] = &[
    ("GigabitEthernet", "Gi"),
    ("FastEthernet", "Fa"),
    ("Vlan", "Vl"),
    ("Loopback", "Lo"),
    ("Serial", "Se"),
    ("Embedded-Service-Engine", "Em"),
];

/// "gi0/1", "g0/1", "GigabitEthernet0/1" -> "GigabitEthernet0/1"
pub fn norm_if(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    // ([a-zA-Z-]+)\s*([\d/.]+)
    let letters_end = name
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_alphabetic() || *c == '-'))
        .map(|(i, _)| i)?;
    if letters_end == 0 {
        return None;
    }
    let t = &name[..letters_end];
    let rest = name[letters_end..].trim_start();
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit() || c == '/' || c == '.') {
        return None;
    }
    let tl = t.to_lowercase();
    for (long, _) in IF_TYPES {
        if long.to_lowercase().starts_with(&tl) {
            return Some(format!("{}{}", long, rest));
        }
    }
    None
}

pub fn short_if(name: &str) -> String {
    for (long, short) in IF_TYPES {
        if let Some(r) = name.strip_prefix(long) {
            return format!("{}{}", short, r);
        }
    }
    name.to_string()
}

/// "GigabitEthernet0/5 - 9" eller "gi0/5-9,gi0/12"
pub fn expand_range(spec: &str) -> Option<Vec<String>> {
    let mut out = vec![];
    for part in spec.split(',') {
        let p = part.trim();
        // ^([a-zA-Z-]+\s*\d+\/)(\d+)\s*-\s*(\d+)$
        if let Some(dash) = p.rfind('-') {
            let left = p[..dash].trim();
            let right = p[dash + 1..].trim();
            if let Some(slash) = left.rfind('/') {
                let num = &left[slash + 1..];
                if !num.is_empty()
                    && num.bytes().all(|b| b.is_ascii_digit())
                    && !right.is_empty()
                    && right.bytes().all(|b| b.is_ascii_digit())
                {
                    let base = norm_if(left)?;
                    let prefix = &base[..base.rfind('/').unwrap() + 1];
                    let a: u32 = num.parse().ok()?;
                    let b: u32 = right.parse().ok()?;
                    for k in a..=b {
                        out.push(format!("{}{}", prefix, k));
                    }
                    continue;
                }
            }
        }
        out.push(norm_if(p)?);
    }
    Some(out)
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VlanList {
    All,
    List(Vec<u16>),
}

pub fn parse_vlan_list(s: &str) -> Option<VlanList> {
    if s == "all" {
        return Some(VlanList::All);
    }
    let mut out: Vec<u16> = vec![];
    for part in s.split(',') {
        let p = part.trim();
        if p.is_empty() {
            return None;
        }
        let (a, b) = match p.split_once('-') {
            Some((x, y)) => (x, y),
            None => (p, p),
        };
        if a.is_empty() || b.is_empty() || !a.bytes().all(|c| c.is_ascii_digit()) || !b.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let a: u32 = a.parse().ok()?;
        let b: u32 = b.parse().ok()?;
        if a < 1 || b > 4094 || b < a {
            return None;
        }
        for v in a..=b {
            let v = v as u16;
            if !out.contains(&v) {
                out.push(v);
            }
        }
    }
    out.sort();
    Some(VlanList::List(out))
}

pub fn vlan_list_str(list: &[u16]) -> String {
    if list.is_empty() {
        return "none".into();
    }
    let mut s = vec![];
    let mut i = 0;
    while i < list.len() {
        let mut j = i;
        while j + 1 < list.len() && list[j + 1] == list[j] + 1 {
            j += 1;
        }
        if j - i >= 2 {
            s.push(format!("{}-{}", list[i], list[j]));
        } else if j > i {
            s.push(format!("{},{}", list[i], list[j]));
        } else {
            s.push(format!("{}", list[i]));
        }
        i = j + 1;
    }
    s.join(",")
}

pub fn pad(s: impl std::fmt::Display, n: usize) -> String {
    let mut s = s.to_string();
    while s.chars().count() < n {
        s.push(' ');
    }
    s
}

pub fn pad_l(s: impl std::fmt::Display, n: usize) -> String {
    let s = s.to_string();
    let c = s.chars().count();
    if c >= n {
        return s;
    }
    format!("{}{}", " ".repeat(n - c), s)
}

/// "3cd92b770142" -> "3cd9.2b77.0142"
pub fn mac_dots(mac: &str) -> String {
    if mac.len() < 12 {
        return mac.to_string();
    }
    format!("{}.{}.{}", &mac[0..4], &mac[4..8], &mac[8..12])
}

/// Windows-stil: 3C-D9-2B-77-01-42
pub fn mac_dash(mac: &str) -> String {
    mac.as_bytes()
        .chunks(2)
        .map(|c| String::from_utf8_lossy(c).to_uppercase())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn mac_colon(mac: &str) -> String {
    mac.as_bytes().chunks(2).map(|c| String::from_utf8_lossy(c).to_string()).collect::<Vec<_>>().join(":")
}

/// FNV-1a över UTF-16-kodenheter (som JS charCodeAt).
pub fn hash(s: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for c in s.encode_utf16() {
        h ^= c as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

/// Liten deterministisk slumpgenerator (xorshift).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }
    pub fn next_f64(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn range(&mut self, n: usize) -> usize {
        ((self.next_f64() * n as f64) as usize).min(n.saturating_sub(1))
    }
}

/// Dagar sedan 1970-01-01 -> (år, månad 1-12, dag)
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let m = m as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i64 - 719468
}

/// Millisekunder sedan epoken för ett UTC-datum (månad 0-11 som i JS Date.UTC).
pub fn date_utc(y: i64, mon0: u32, d: u32, h: u32, mi: u32, s: u32) -> f64 {
    let days = days_from_civil(y, mon0 + 1, d);
    (days * 86400 + (h * 3600 + mi * 60 + s) as i64) as f64 * 1000.0
}

pub struct DateParts {
    pub year: i64,
    pub mon0: u32,
    pub day: u32,
    pub dow: u32,
    pub h: u32,
    pub m: u32,
    pub s: u32,
    pub ms: u32,
}

pub fn date_parts(ms: f64) -> DateParts {
    let total_ms = ms.floor() as i64;
    let days = total_ms.div_euclid(86_400_000);
    let rem = total_ms.rem_euclid(86_400_000);
    let (y, m, d) = civil_from_days(days);
    let dow = ((days % 7 + 7 + 4) % 7) as u32; // 1970-01-01 var torsdag
    DateParts {
        year: y,
        mon0: m - 1,
        day: d,
        dow,
        h: (rem / 3_600_000) as u32,
        m: ((rem / 60_000) % 60) as u32,
        s: ((rem / 1000) % 60) as u32,
        ms: (rem % 1000) as u32,
    }
}

pub fn now_ms() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        crate::engine::web::now_ms() as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

/// Enkel regex-ersättning för `| include` m.m.: stöder ^, $, ., .* och vanlig text.
pub struct MiniRe {
    alts: Vec<Vec<ReTok>>,
}

#[derive(Clone)]
enum ReTok {
    Start,
    End,
    Any,
    AnyStar,
    Ch(char),
    Class(Vec<(char, char)>, bool),
    Star(Box<ReTok>),
    Plus(Box<ReTok>),
    Opt(Box<ReTok>),
}

impl MiniRe {
    pub fn new(pat: &str) -> MiniRe {
        let mut alts = vec![];
        for alt in split_alt(pat) {
            let mut toks: Vec<ReTok> = vec![];
            let cs: Vec<char> = alt.chars().collect();
            let mut i = 0;
            while i < cs.len() {
                let c = cs[i];
                let tok = match c {
                    '^' if i == 0 => ReTok::Start,
                    '$' if i == cs.len() - 1 => ReTok::End,
                    '.' => ReTok::Any,
                    '\\' if i + 1 < cs.len() => {
                        i += 1;
                        match cs[i] {
                            'd' => ReTok::Class(vec![('0', '9')], false),
                            's' => ReTok::Class(vec![(' ', ' '), ('\t', '\t')], false),
                            'w' => ReTok::Class(vec![('a', 'z'), ('A', 'Z'), ('0', '9'), ('_', '_')], false),
                            o => ReTok::Ch(o),
                        }
                    }
                    '[' => {
                        let mut j = i + 1;
                        let mut neg = false;
                        if j < cs.len() && cs[j] == '^' {
                            neg = true;
                            j += 1;
                        }
                        let mut ranges = vec![];
                        while j < cs.len() && cs[j] != ']' {
                            if j + 2 < cs.len() && cs[j + 1] == '-' && cs[j + 2] != ']' {
                                ranges.push((cs[j], cs[j + 2]));
                                j += 3;
                            } else {
                                ranges.push((cs[j], cs[j]));
                                j += 1;
                            }
                        }
                        i = j;
                        ReTok::Class(ranges, neg)
                    }
                    '_' => ReTok::Class(vec![(' ', ' '), (',', ','), ('(', ')'), ('{', '{'), ('}', '}')], false),
                    o => ReTok::Ch(o),
                };
                i += 1;
                if i < cs.len() && cs[i] == '*' {
                    i += 1;
                    if let ReTok::Any = tok {
                        toks.push(ReTok::AnyStar);
                    } else {
                        toks.push(ReTok::Star(Box::new(tok)));
                    }
                } else if i < cs.len() && cs[i] == '+' {
                    i += 1;
                    toks.push(ReTok::Plus(Box::new(tok)));
                } else if i < cs.len() && cs[i] == '?' {
                    i += 1;
                    toks.push(ReTok::Opt(Box::new(tok)));
                } else {
                    toks.push(tok);
                }
            }
            alts.push(toks);
        }
        MiniRe { alts }
    }
    pub fn is_match(&self, s: &str) -> bool {
        let cs: Vec<char> = s.chars().collect();
        for toks in &self.alts {
            for start in 0..=cs.len() {
                if m_at(toks, 0, &cs, start) {
                    return true;
                }
                if matches!(toks.first(), Some(ReTok::Start)) {
                    break;
                }
            }
        }
        false
    }
}

fn split_alt(p: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut depth = 0;
    let mut esc = false;
    for c in p.chars() {
        if esc {
            cur.push(c);
            esc = false;
            continue;
        }
        match c {
            '\\' => {
                cur.push(c);
                esc = true;
            }
            '[' => {
                depth += 1;
                cur.push(c)
            }
            ']' => {
                depth -= 1;
                cur.push(c)
            }
            '|' if depth == 0 => out.push(std::mem::take(&mut cur)),
            '(' | ')' if depth == 0 => {}
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

fn one(t: &ReTok, c: char) -> bool {
    match t {
        ReTok::Any => true,
        ReTok::Ch(x) => *x == c,
        ReTok::Class(r, neg) => r.iter().any(|(a, b)| c >= *a && c <= *b) != *neg,
        _ => false,
    }
}

fn m_at(toks: &[ReTok], ti: usize, cs: &[char], ci: usize) -> bool {
    if ti == toks.len() {
        return true;
    }
    match &toks[ti] {
        ReTok::Start => ci == 0 && m_at(toks, ti + 1, cs, ci),
        ReTok::End => ci == cs.len(),
        ReTok::AnyStar => (ci..=cs.len()).any(|k| m_at(toks, ti + 1, cs, k)),
        ReTok::Star(t) => {
            let mut k = ci;
            loop {
                if m_at(toks, ti + 1, cs, k) {
                    return true;
                }
                if k < cs.len() && one(t, cs[k]) {
                    k += 1;
                } else {
                    return false;
                }
            }
        }
        ReTok::Plus(t) => {
            let mut k = ci;
            if !(k < cs.len() && one(t, cs[k])) {
                return false;
            }
            k += 1;
            loop {
                if m_at(toks, ti + 1, cs, k) {
                    return true;
                }
                if k < cs.len() && one(t, cs[k]) {
                    k += 1;
                } else {
                    return false;
                }
            }
        }
        ReTok::Opt(t) => (ci < cs.len() && one(t, cs[ci]) && m_at(toks, ti + 1, cs, ci + 1)) || m_at(toks, ti + 1, cs, ci),
        t => ci < cs.len() && one(t, cs[ci]) && m_at(toks, ti + 1, cs, ci + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basics() {
        assert_eq!(norm_if("gi0/1").unwrap(), "GigabitEthernet0/1");
        assert_eq!(norm_if("g0/0.10").unwrap(), "GigabitEthernet0/0.10");
        assert_eq!(norm_if("vlan 99").unwrap(), "Vlan99");
        assert_eq!(norm_if("se0/0/0").unwrap(), "Serial0/0/0");
        assert_eq!(expand_range("gi0/5 - 7").unwrap().len(), 3);
        assert_eq!(vlan_list_str(&[10, 20, 30, 99]), "10,20,30,99");
        assert_eq!(vlan_list_str(&[1, 2, 3, 5]), "1-3,5");
        assert_eq!(mask_to_prefix("255.255.255.192"), Some(26));
        assert_eq!(prefix_to_mask(26), "255.255.255.192");
        assert_eq!(broadcast("192.168.1.0", "255.255.255.192"), "192.168.1.63");
        assert!(MiniRe::new("^Gi0/1 ").is_match("Gi0/1  up"));
        assert!(MiniRe::new("up|down").is_match("is down"));
        let p = date_parts(date_utc(2026, 8, 29, 6, 0, 0));
        assert_eq!((p.year, p.mon0, p.day, p.h, p.dow), (2026, 8, 29, 6, 2));
    }
}
