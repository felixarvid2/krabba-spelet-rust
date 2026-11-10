# Portering av Krabba-passet (2D) till Rust – gemensamma regler

Källan ligger i `/home/claude/felixarvid2/test/natverksspel/` (JS). Bara 2D-läget portas: ingen 3D, ingen HD-2D, inga GLB-modeller.

## Regler för alla
- Varje del äger sina egna filer (se uppdelningen nedan). Ändra inte andras filer. Behöver du något från en annan del: använd API:t som redan finns (signaturerna i stubbarna) och skriv en kort notering i din slutrapport om vad som saknas.
- Signaturerna i stubbarna (`todo!()`) är kontraktet. Du får lägga till nya publika funktioner och fält, men ändra inte befintliga signaturer.
- Porta troget: samma texter (svenska i spelet, IOS-utdata på engelska exakt som i JS), samma logik, samma ordning. JS `Object.keys` på strängnycklar = insättningsordning (IndexMap); på heltalsnycklar = stigande (BTreeMap).
- `Math.random()` → `st.random()` när State finns, annars en egen `util::Rng`.
- Koden ska kompilera med `cargo check` utan fel. Inga nya crates utan stark anledning (finns: minifb, fontdue, indexmap, png, serde, serde_json).
- Kommentarer på svenska, korta.
- Tester: lägg enhetstester i din egen fil (`#[cfg(test)]`) eller i `tests/<din-del>.rs`.
- Simulatorn (`src/net/sim.rs`, `src/net/dynroute.rs`) skrivs parallellt. Tills den är klar ger anrop `todo!()`-panik vid körning. När `git log main` har en commit "sim: klar" kan du köra `git merge main` i din worktree för att testa på riktigt.
- Committa ditt arbete i din worktree-gren med ett tydligt meddelande när du är klar.

## Uppdelning (filer)
| Del | Filer | Källa |
|---|---|---|
| sim | src/net/sim.rs, src/net/dynroute.rs | js/net/sim.js, js/net/dynroute.js |
| ios-kärna | src/cli/ios_cmds.rs, src/cli/ios_help.rs | js/cli/ios.js rad 1–1610 (ramverket finns redan i src/cli/ios.rs) |
| ios-mer | src/cli/ios_more.rs, src/cli/ios_dyn.rs | js/cli/ios.js rad 1611–slut, js/cli/ios_dyn.js |
| ios-show | src/cli/ios_show.rs | js/cli/ios_show.js |
| host | src/cli/host.rs (+ src/cli/host_*.rs) | js/cli/host.js, js/cli/host_more.js |
| wlc/lb | src/cli/wlc.rs, src/cli/lb.rs | js/cli/wlc.js, js/cli/lb.js, js/cli/more7.js |
| nivåer | src/game/levels.rs (+ src/game/levels_*.rs), tests/levels.rs | js/levels.js, js/build.js, js/kurs2.js, test/levels.js, test/build.js, test/kurs2.js |
| motor | src/engine/*.rs | (ny) |
| värld | src/game/world.rs (+ src/game/world_*.rs, src/game/art.rs) | js/world/world2d.js, art2d.js, scene2d.js, path2d.js, life2d.js m.fl. 2D-filer |
| ui | src/game/mod.rs, src/game/terminal.rs, src/game/ui_*.rs, src/game/handbook.rs, src/main.rs | js/game.js, js/ui/terminal.js, ui.js, handbook.js, more.js, v7.js, v8.js |
