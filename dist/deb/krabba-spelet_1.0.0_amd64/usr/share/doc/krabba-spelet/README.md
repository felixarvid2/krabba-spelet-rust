# Krabba spelet 🦀

Ett nätverksspel för kurserna *Nätverksteknik* och *Routing och switching*, skrivet i Rust. Du är ny nätverkstekniker på Nordvik och går runt på huvudkontoret i Göteborg, på lagret i Borås och (i kurs 2) i Mölndal. Varje vecka har "Krabban" gjort sönder två saker i racket, och en kollega har ett ärende till dig. Du kopplar in konsolkabeln i Cisco-utrustningen, felsöker med riktiga CLI-kommandon, rättar felet och skriver en felrapport.

Det här är en Rust-port av 2D-delen av [Krabba-passet](https://github.com/felixarvid2/test/tree/main/natverksspel). 3D och HD-2D är borttagna. Spelet har en egen liten spelmotor (Krabbmotorn) som ritar allt i mjukvara.

## Starta

Du behöver [Rust](https://rustup.rs) (stabil, 2024-utgåvan).

```
cargo run --release
```

På Linux behövs X11- eller Wayland-biblioteken (`libx11-dev`, `libxkbcommon-dev` eller motsvarande). Windows och macOS fungerar utan extra paket.

Framsteg, inställningar, anteckningar och terminalhistorik sparas i `~/.krabba-spelet/save.json` (Windows: `%APPDATA%\krabba-spelet\save.json`). Miljövariabeln `KRABBA_SAVE` pekar om filen.

## Innehåll

- **Kurs 1, vecka 1–10**: konsolen, ramar och MAC-adresser, IP och subnätning, VLAN och trunkar, routing, NAT och SSH, drift och övervakning, trådlöst, säkerhet och brandvägg, VPN och lastbalansering.
- **Bygg från grunden 1–5**: fabriksinställda enheter som du konfigurerar steg för steg.
- **Kurs 2, vecka 11–14**: nav och ekrar, brandvägg och VPN, OSPF, BGP med PAT och reservdefault.

Varje fel är simulerat på riktigt: symptomen räknas fram ur konfigurationen (länkar, duplex, VLAN, STP, routing, OSPF, BGP, ARP, ACL, NAT, DHCP, DNS, PoE, port security, IPsec, MTU och lastbalansering). Ett fel försvinner bara när orsaken är rättad.

## Styrning

| Tangent | Gör |
|---|---|
| W A S D / pilar, Shift | Gå, spring |
| Klick | Gå dit / använd det du klickar på |
| E / Mellanslag / Enter | Använd: koppla in konsolen, prata, använda dator |
| Mushjul / Z / X | Zooma |
| T | Öppna laptopen |
| F | Skriv felrapport |
| L | Ledtråd |
| H | Handbok: kommandon, felbibliotek, adressplan, subnätsräknare, OSI, ordlista |
| Tab | Ärendelistan |
| M | Karta med snabbresa |
| N | Anteckningar |
| F1 | Hjälp |
| Esc | Paus / stäng |

I terminalen fungerar `?`, Tab, ↑/↓, Ctrl+C, Ctrl+Z, Ctrl+A K (stäng `screen`), Ctrl+L, Ctrl+U, Ctrl+W och PageUp/PageDown. Konsolen kopplas med `sudo screen /dev/ttyUSB0 9600` från laptopen. SSH-kontot står på lappen: `drift` / `nordvik`.

## Kod

```
src/
  engine/        Krabbmotorn: pixelbuffert, sprites, text (VT323, Press Start 2P), fönster (minifb), PNG
  net/           nätmodellen (model.rs), simulatorn (sim.rs) och OSPF/BGP (dynroute.rs)
  cli/           Cisco IOS (ios*.rs), Windows/Linux (host*.rs), WLC (wlc.rs), lastbalanseraren (lb.rs)
  game/          veckorna (levels*.rs), 2D-världen (world*.rs, art.rs), terminalen, dialoger, handbok, sparning
tests/           simulatorn, veckorna och byggena, terminalerna, motorn och världen
assets/fonts/    VT323 och Press Start 2P (SIL Open Font License)
```

### Tester

```
cargo test --release
```

`tests/levels.rs` kontrollerar att varje fel i alla veckor och byggen syns från start och försvinner med lösningen i ledtrådarna. `tests/sim.rs` testar DHCP, ping, NAT, DNS, STP, OSPF, BGP, IPsec och lastbalanseraren i det felfria nätet.

Spelet kan köras utan skärm och spara skärmdumpar:

```
cargo run --release -- --headless-demo /tmp/demo
```

## Licenser

Typsnitten VT323 och Press Start 2P används under SIL Open Font License (se `assets/fonts/OFL-*.txt`).
