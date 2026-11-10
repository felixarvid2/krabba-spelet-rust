# Så gör du en GitHub-release av Krabba spelet

Repo: felixarvid2/krabba-spelet-rust. Releasen byggs och publiceras av GitHub Actions (`.github/workflows/release.yml`). Du bygger aldrig releasen själv på GitHub; du förbereder en release-gren, får testerna gröna lokalt, slår ihop PR:en till main och verifierar sedan att arbetsflödet publicerade releasen.

## Så triggas arbetsflödet

`release.yml` körs på tre sätt:

1. **Vanliga vägen:** en push till main som ändrar en fil `docs/releases/vX.Y.Z.md` (det sker när PR:en slås ihop). Taggen tas från filnamnet; finns flera ändrade filer tas den högsta versionen.
2. En push av en tagg `vX.Y.Z`.
3. Manuellt (workflow_dispatch) med en taggparameter.

Arbetsflödet kör på Ubuntu 22.04 (glibc 2.35, så paketet fungerar på Ubuntu 22.04 / Debian 12 och nyare):

- `cargo test --release` – fallerar testerna blir det ingen release.
- `tools/build-web.sh` – webbversionen (WebAssembly) som zip: `krabba-spelet-vX.Y.Z-webb.zip`.
- `tools/build-deb.sh` – `krabba-spelet_X.Y.Z_amd64.deb`.
- `tools/build-apk.sh` – Android-appen, publiceras som `krabba-spelet-vX.Y.Z.apk` (versionCode = X·10000 + Y·100 + Z).
- Skriver release-texten (en svensk installationsrad + innehållet i `docs/releases/vX.Y.Z.md`) och publicerar releasen "Krabba spelet vX.Y.Z" med de tre filerna bifogade.

Taggen måste matcha `^v[0-9]+\.[0-9]+\.[0-9]+$` och versionen i `Cargo.toml` måste vara samma, annars avbryts jobbet.

## Steg för steg

1. **Gren:** utgå från senaste origin/main, t.ex. `release/vX.Y.Z`. Om du använder worktree, hämta med `git fetch origin '+refs/heads/*:refs/remotes/origin/*'` (origin kan sakna fetch-refspec).
2. **Version:** sätt `version = "X.Y.Z"` i `Cargo.toml` (och kör `cargo check` så att `Cargo.lock` uppdateras). Semver: ny funktion = minor, bara fixar = patch.
3. **Release-anteckningar:** skapa `docs/releases/vX.Y.Z.md` på svenska, för spelare, i samma stil som tidigare filer: rubrik `## Nytt sedan vFÖRRA`, sedan `###`-sektioner med korta punkter. Inga tekniska detaljer, filnamn eller testsiffror. Inga modellnamn. Det är denna fil som triggar releasen.
4. **Dokumentation:** uppdatera `README.md` om något spelaren eller utvecklaren bör veta har ändrats.
5. **Bygg och testa lokalt:**

   ```
   cargo test --release
   tools/build-web.sh
   tools/build-deb.sh
   tools/build-apk.sh   # kräver Android SDK (platforms;android-34, build-tools) och JDK 17
   ```

   Räkna testerna i utskriften från `cargo test` (raderna `test result`). Prova gärna webbversionen (`dist/web/index.html`) i en webbläsare.
6. **Commit och push** (aldrig `--force`). Commit-meddelandet slutar med de trailers som sessionen anger (Co-Authored-By och Claude-Session).

   ```
   git push -u origin release/vX.Y.Z
   ```

7. **PR till main:** kort engelsk sammanfattning, testantal, och avsluta med sessionens attributionsrader.
8. **Merge:** ägaren har gett stående godkännande att slå ihop när testerna är gröna. Slå ihop med merge commit (inte squash/rebase), med hela 40-teckens-SHA:n för huvudet (`git rev-parse HEAD`).
9. **Verifiera releasen:** arbetsflödet "Release" körs på mergecommiten (ca 10–15 min). Vänta inte med sleep; schemalägg en återkoll. Kontrollera sedan att releasen `vX.Y.Z` finns och att alla tre filerna är uppladdade. Misslyckades jobbet: läs loggen, fixa på en ny gren, ny PR. Taggen kan köras om manuellt via workflow_dispatch.
10. **Rapportera till ägaren på svenska:** länk till releasen (`https://github.com/felixarvid2/krabba-spelet-rust/releases/tag/vX.Y.Z`), direktlänkar till filerna (`…/releases/download/vX.Y.Z/krabba-spelet-vX.Y.Z-webb.zip`, `…/krabba-spelet_X.Y.Z_amd64.deb` och `…/krabba-spelet-vX.Y.Z.apk`), och vad som bara kan testas på en riktig Android-enhet och vad som är nytt i korthet.

## Regler

- Aldrig force-push, aldrig skippa eller stänga av tester för att få grönt.
- Inga modellidentifierare i release-anteckningarna.
- Släpp aldrig utan att `docs/releases/vX.Y.Z.md` finns; utan den triggas inget och releasen saknar text.
