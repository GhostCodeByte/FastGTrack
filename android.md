# Android Test Build

## Voraussetzungen
- Android Studio mit SDK, Platform-Tools und NDK
- Rust-Targets `x86_64-linux-android` und/oder `aarch64-linux-android`
- `cargo-apk`

## Schnellstart Emulator
1. Android-Emulator starten
2. Im Projekt ausfuehren:

```powershell
./scripts/build-android-apk.ps1 -Target x86_64-linux-android -Install
```

## Release-Testbuild

```powershell
./scripts/build-android-apk.ps1 -Target aarch64-linux-android -Release
```

## Hinweise
- Standard fuer Emulator ist `x86_64-linux-android`.
- Fuer echte ARM-Geraete `aarch64-linux-android` verwenden.
- Das Script setzt `ANDROID_HOME`, `ANDROID_NDK_ROOT` und `JAVA_HOME` nach Moeglichkeit automatisch.
- Die Android-App nutzt denselben Slint/Rust-Codepfad wie der Desktop-MVP, aber mit Android-Einstiegspunkt via `android_main`.
