<p align="center">
  <img src="assets/icon/krabba-128.png" width="120" alt="Krabba spelet">
</p>

<h1 align="center">Krabba spelet 🦀</h1>

<p align="center">
  <strong>Du är nätverkstekniker på Nordvik. Krabban har varit framme igen.</strong><br>
  Ett 2D-nätverksspel i Rust där du felsöker Cisco-utrustning med riktiga CLI-kommandon.
</p>

<p align="center">
  <a href="https://github.com/felixarvid2/krabba-spelet-rust/releases/latest"><img src="https://img.shields.io/github/v/release/felixarvid2/krabba-spelet-rust?label=senaste%20release&color=orange" alt="Senaste release"></a>
  <a href="https://github.com/felixarvid2/krabba-spelet-rust/actions/workflows/release.yml"><img src="https://github.com/felixarvid2/krabba-spelet-rust/actions/workflows/release.yml/badge.svg" alt="Release"></a>
  <img src="https://img.shields.io/badge/Rust-2024-000000?logo=rust" alt="Rust 2024">
  <img src="https://img.shields.io/badge/plattformar-webb%20%7C%20Linux-4c6ef5" alt="Webb och Linux">
</p>

---

**Krabba spelet** är ett nätverksspel för kurserna *Nätverksteknik* och *Routing och switching*. Du går runt på huvudkontoret i Göteborg, på lagret i Borås och – i kurs 2 – i Mölndal. Varje vecka har "Krabban" gjort sönder två saker i racket, och en kollega har ett ärende till dig. Du kopplar in konsolkabeln, felsöker med riktiga CLI-kommandon, rättar felet och skriver en felrapport.

Spelet är en Rust-port av 2D-delen av [Krabba-passet](https://github.com/felixarvid2/test/tree/main/natverksspel). 3D och HD-2D är borttagna, och allt ritas av en egen liten spelmotor – Krabbmotorn – direkt i mjukvara.

<p align="center">
  <a href="#spela">Spela</a> ·
  <a href="#innehåll">Innehåll</a> ·
  <a href="#styrning">Styrning</a> ·
  <a href="#utveckling">Utveckling</a> ·
  <a href="#licenser">Licenser</a>
</p>

## Spela

Det enklaste är att hämta senaste versionen från [Releases](https://github.com/felixarvid2/krabba-spelet-rust/releases/latest).

### I webbläsaren

1. Ladda ner `krabba-spelet-v<version>-webb.zip`.
2. Packa upp zip-filen.
3. Dubbelklicka på `index.html`.

Fungerar i Chrome, Edge och Firefox – utan internet och utan installation. Framstegen sparas i webbläsaren.

### Ubuntu och Debian

```bash
sudo apt install ./krabba-spelet_<version>_amd64.deb
krabba-spelet
```

Du kan också starta *Krabba spelet* från programmenyn. Paketet kräver Ubuntu 22.04, Debian 12 eller nyare.

### Från källkoden

Du behöver [Rust](https://rustup.rs) (stabil, 2024-utgåvan).

```bash
git clone https://github.com/felixarvid2/krabba-spelet-rust.git
cd krabba-spelet-rust
cargo run --release
```

På Linux behövs X11- eller Wayland-biblioteken och ALSA:

```bash
sudo apt install libx11-dev libxkbcommon-dev libasound2-dev
```

Windows och macOS fungerar utan extra paket.

### Spara och inställningar

Framsteg, inställningar, anteckningar och terminalhistorik sparas i:

| Plattform | Sökväg |
|---|---|
| Linux | `~/.krabba-spelet/save.json` |
| Windows | `%APPDATA%\krabba-spelet\save.json` |

Miljövariabeln `KRABBA_SAVE` pekar om filen:

```bash
KRABBA_SAVE=/tmp/min-sparning.json cargo run --release
```

## Innehåll

### Kurs 1 – vecka 1–10

Konsolen, ramar och MAC-adresser, IP och subnätning, VLAN och trunkar, routing, NAT och SSH, drift och övervakning, trådlöst, säkerhet och brandvägg, VPN och lastbalansering.

### Kurs 2 – vecka 11–14

Nav och ekrar, brandvägg och VPN, OSPF, BGP med PAT och reservdefault.

### Bygg från grunden 1–5

Fabriksinställda enheter som du konfigurerar steg för steg.

### Mer att upptäcka

- **Svårighetsgrad** (Inställningar): Lätt, Normal eller Svår. I svårt läge säger spelet inte till när ett fel är löst – du måste bevisa det, till exempel genom att pinga från rätt dator eller visa rätt `show`-utdata. Ärendelistan (Tab) visar vad som räknas som bevis. Ingen kompasspil och 1,5 gånger XP.
- **Karriär** (J): XP, rang och 50 prestationer. Veckoutmaningar, kaffe som gör dig snabbare och Krabban som gömmer sig nära ett av veckans fel – fånga den för en ledtråd.
- **Liv i kontoret**: kollegorna går omkring, katten, ankorna, duvorna och måsen, dag, kväll och natt, sol, moln och regn.
- **Ljud**: syntetiserade ljud som i originalet – fotsteg efter golv, fläktar, regn, kontorssorl, terminalknatter, djur och bakgrundsmusik. Ställs in under Inställningar.
- **Handboken** (H): kommandon, felbibliotek, adressplan, subnätsräknare, OSI-modellen och ordlista.

Varje fel är simulerat på riktigt: symptomen räknas fram ur konfigurationen (länkar, duplex, VLAN, STP, routing, OSPF, BGP, ARP, ACL, NAT, DHCP, DNS, PoE, port security, IPsec, MTU och lastbalansering). Ett fel försvinner bara när orsaken är rättad.

### Så går en vecka till

1. Öppna **ärendelistan** med `Tab` och läs vad kollegorna behöver hjälp med.
2. Gå till racket och koppla in konsolkabeln.
3. Felsök med `show`-kommandon och jämför med handboken.
4. Rätta felet – och bevisa att det fungerar.
5. Skriv **felrapporten** med `F`.

## Styrning

### Rörelse

| Tangent | Gör |
|---|---|
| `W` `A` `S` `D` / piltangenter | Gå |
| `Shift` | Spring |
| Klick | Gå dit / använd det du klickar på |
| `E` / `Mellanslag` / `Enter` | Använd: koppla in konsolen, prata, använda dator |
| Mushjul / `Z` / `X` | Zooma |

### Verktyg och fönster

| Tangent | Gör |
|---|---|
| `T` | Öppna laptopen |
| `F` | Skriv felrapport |
| `L` | Ledtråd |
| `H` | Handbok: kommandon, felbibliotek, adressplan, subnätsräknare, OSI, ordlista |
| `Tab` | Ärendelistan |
| `M` | Karta: klicka för att gå dit, snabbresa |
| `K` | Minikarta |
| `J` | Karriär och prestationer |
| `N` | Anteckningar |
| `F1` | Hjälp |
| `Esc` | Paus / stäng |

### I terminalen

`?`, `Tab`, `↑`/`↓`, `Ctrl+C`, `Ctrl+Z`, `Ctrl+A K` (stäng `screen`), `Ctrl+L`, `Ctrl+U`, `Ctrl+W` och `PageUp`/`PageDown` fungerar som vanligt.

Konsolen kopplas med:

```bash
sudo screen /dev/ttyUSB0 9600
```

SSH-kontot står på lappen: `drift` / `nordvik`.

## Utveckling

### Projektstruktur

```
src/
├── engine/        Krabbmotorn: pixelbuffert, sprites, text (VT323, Press Start 2P),
│                  ljudsyntes (cpal / Web Audio), fönster (minifb), webbläsare och PNG
├── net/           nätmodellen (model.rs), simulatorn (sim.rs) och OSPF/BGP (dynroute.rs)
├── cli/           Cisco IOS (ios*.rs), Windows/Linux (host*.rs), WLC och lastbalanseraren
├── game/          veckorna (levels*.rs), 2D-världen (world*.rs, art.rs), terminal,
│                  dialoger, handbok, gränssnitt och sparning
├── lib.rs
└── main.rs
tests/             simulatorn, veckorna och byggena, terminalerna, motorn och världen
assets/            typsnitt (SIL Open Font License), ikon och .desktop-fil
web/index.html     webbsidan som kör spelet som WebAssembly
tools/             byggskript för webbversionen och .deb-paketet
docs/              portningsanteckningar, releaseinstruktioner och releaseanteckningar
```

### Bygg och testa

```bash
cargo build --release
cargo test --release
```

`tests/levels.rs` kontrollerar att varje fel i alla veckor och byggen syns från start och försvinner med lösningen i ledtrådarna. `tests/sim.rs` testar DHCP, ping, NAT, DNS, STP, OSPF, BGP, IPsec och lastbalanseraren i det felfria nätet.

Spelet kan också köras utan skärm och spara skärmdumpar:

```bash
cargo run --release -- --headless-demo /tmp/demo
```

### Bygga releasefilerna lokalt

```bash
rustup target add wasm32-unknown-unknown
tools/build-web.sh     # dist/web: index.html + spelet som WebAssembly
tools/build-deb.sh     # dist/krabba-spelet_<version>_amd64.deb
```

### Releaser

Releaser byggs och publiceras av GitHub Actions när en `docs/releases/vX.Y.Z.md` slås ihop till main. Läs mer i [docs/RELEASE.md](docs/RELEASE.md).

## Licenser

Typsnitten VT323 och Press Start 2P används under SIL Open Font License – se `assets/fonts/OFL-*.txt`.
