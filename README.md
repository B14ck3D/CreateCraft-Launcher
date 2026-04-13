# CreateCrafts Launcher

Electron + React: MCLC, Microsoft / offline, profile, RAM, paczka NeoForge.

## Uruchomienie

```bash
npm install
npm start
```

## Build instalatora (Windows, MSI)

Czyszczenie artefaktów (`out/`, `.vite/`, opcjonalnie certyfikat lokalny):

```bash
npm run clean
npm run clean:all
```

Pełna przebudowa od zera (clean + `prep:brand` + **nowy** certyfikat self-signed + podpisany MSI):

```bash
npm run rebuild:all
```

Pojedynczy build (bez czyszczenia):

```bash
npm run make
```

Instalator: `out\createcrafts-installer\createcrafts-installer.msi` (bez numeru wersji w nazwie pliku). Paczka przed MSI: `out\createcrafts-installer-win32-x64\` — plik uruchomieniowy aplikacji musi nazywać się **`CreateCrafts Launcher.exe`** (ustawione w `forge.config.js` jako `executableName`). Ikona: `build/icon.png` z `public/logomain.png` (`npm run prep:brand`).

**Uwaga:** przy pierwszym buildzie MSI electron-builder zwykle sam pobiera binaria **WiX**; przy błędzie kompilacji MSI zobacz log electron-builder lub [dokumentację MSI](https://www.electron.build/msi.html).

### Podpis MSI (Authenticode)

1. **Wygeneruj certyfikat (self-signed, do testów / wewnętrznie):** `npm run codesign:cert`  
   Powstanie `build/codesign.pfx` oraz `build/codesign-password.txt` (oba w `.gitignore`).

2. **Zbuduj podpisany instalator:** `npm run make:signed`  
   Skrypt ustawia `CSC_LINK` i `CSC_KEY_PASSWORD` (standard electron-builder) i odpala `npm run make`.

Dla **publicznej** dystrybucji użyj certyfikatu z **zaufanego urzędu (CA)** — self-signed nie usuwa ostrzeżeń SmartScreen przy pobieraniu z internetu.

Dane gry: `%AppData%\CreateCrafts\` (wcześniej `.super-smp-client` — migracja przy pierwszym starcie).
