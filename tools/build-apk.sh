#!/bin/sh
# Bygger dist/krabba-spelet_<version>.apk: webbversionen (WebAssembly) i en Android-WebView.
# Kräver Android SDK (platform 34 eller nyare och build-tools) och JDK 17+. Ingen Gradle behövs.
# SDK:n hittas via ANDROID_HOME, ANDROID_SDK_ROOT eller /opt/android-sdk.
set -e
cd "$(dirname "$0")/.."
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-/opt/android-sdk}}"
BT=$(ls -d "$SDK"/build-tools/*/ 2>/dev/null | sort -V | tail -n1)
PLAT=$(ls -d "$SDK"/platforms/android-*/ 2>/dev/null | sed 's#.*/android-\([0-9]*\)/#\1#' | sort -n | awk '$1>=34' | tail -n1)
if [ -z "$BT" ] || [ -z "$PLAT" ]; then
  echo "Android SDK saknas (build-tools och platforms;android-34 behövs) i $SDK" >&2
  exit 1
fi
JAR="$SDK/platforms/android-$PLAT/android.jar"
echo "SDK: $SDK  build-tools: $(basename "$BT")  platform: android-$PLAT"

V=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
MAJ=$(echo "$V" | cut -d. -f1); MIN=$(echo "$V" | cut -d. -f2); PAT=$(echo "$V" | cut -d. -f3)
CODE=$((MAJ * 10000 + MIN * 100 + PAT))

tools/build-web.sh

B=target/apk
rm -rf "$B" && mkdir -p "$B/assets" "$B/res" "$B/classes" "$B/gen"
# Webbversionen utan den lösa .wasm-filen (spelet laddas från krabba_spelet.js)
for f in dist/web/*; do
  case "$f" in *.wasm) ;; *) cp "$f" "$B/assets/" ;; esac
done
sed -e "s/@VERSION_CODE@/$CODE/" -e "s/@VERSION_NAME@/$V/" android/AndroidManifest.xml > "$B/AndroidManifest.xml"

"$BT/aapt2" compile --dir android/res -o "$B/res/res.zip"
"$BT/aapt2" link -o "$B/base.apk" -I "$JAR" --manifest "$B/AndroidManifest.xml" \
  -A "$B/assets" --java "$B/gen" --min-sdk-version 24 --target-sdk-version 34 \
  --version-code "$CODE" --version-name "$V" "$B/res/res.zip"

javac -encoding UTF-8 --release 11 -classpath "$JAR" -d "$B/classes" \
  $(find android/src "$B/gen" -name '*.java')
"$BT/d8" --release --min-api 24 --lib "$JAR" --output "$B" $(find "$B/classes" -name '*.class')
(cd "$B" && zip -q -j base.apk classes.dex)

"$BT/zipalign" -f -p 4 "$B/base.apk" "$B/aligned.apk"
OUT="dist/krabba-spelet_${V}.apk"
mkdir -p dist
"$BT/apksigner" sign --ks android/krabba-debug.keystore --ks-pass pass:android --key-pass pass:android \
  --ks-key-alias krabba --out "$OUT" "$B/aligned.apk"
"$BT/apksigner" verify "$OUT"
echo "Klart: $OUT (versionCode $CODE)"
