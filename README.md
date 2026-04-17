# CreateCrafts Launcher

Oficjalny launcher **Create Crafts PL** do gry na paczce **NeoForge** (Minecraft 1.21.x): logowanie Microsoft lub offline, synchronizacja modów z serwera projektu i uruchamianie klienta przez **Tauri + Rust + React**.

## Platformy

- Windows: instalator `nsis` (`.exe`)
- Linux: paczka `deb` (lokalny build na Arch i większości dystrybucji)
- Linux (CI / Ubuntu): dodatkowo `AppImage` — `linuxdeploy` na Arch często pada przez `strip` (ELF `.relr.dyn`) i ścieżki `gdk-pixbuf` z Debiana; dlatego AppImage budujemy w GitHub Actions (`npm run build:linux-ci`).
- Arch Linux: szkielet paczki `PKGBUILD` (AUR/binary packaging flow)

## Wymagania lokalne

- Node.js 20+
- Rust stable (rustup + cargo)
- npm

### Linux (Ubuntu/Arch i pochodne)

Tauri 2 wymaga bibliotek systemowych GUI/WebKit.

Na Ubuntu/Debian:

```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf \
  libssl-dev \
  libsoup-3.0-dev
```

Na Arch Linux (pakiety mogą mieć minimalnie inne nazwy zależnie od mirrorów):

```bash
sudo pacman -S --needed \
  webkit2gtk-4.1 \
  gtk3 \
  libayatana-appindicator \
  librsvg \
  patchelf \
  openssl \
  libsoup3 \
  xdg-utils \
  base-devel
```

Skrypt `scripts/tauri-build.cjs` ustawia na Linuxie `APPIMAGE_EXTRACT_AND_RUN=1`, żeby narzędzia AppImage używane przy bundlowaniu (np. `linuxdeploy`) działały bez montowania FUSE w środowisku developerskim. Jeśli dalej coś pada, doinstaluj też `fuse2` i `squashfs-tools`.

## Komendy build

```bash
npm ci
```

- `npm run build:windows` - build artefaktów Windows (`nsis`)
- `npm run build:linux` - build `deb` (działa m.in. na Arch)
- `npm run build:linux-ci` - build `deb` + `AppImage` (np. w CI na Ubuntu 22.04)
- `npm run build:all` - build wg `tauri.conf.json` (na Linuxie: `deb`, na Windows: `nsis`)

## Java (runtime)

Launcher preferuje systemową Javę, a jeśli jej brakuje lub wersja jest za stara, używa portable **JDK 21 (Temurin)**.

- Windows: dostępna dodatkowa opcja instalatora systemowego JDK (MSI/UAC).
- Linux: brak ścieżki MSI/UAC; obowiązuje wykrycie systemowej Javy + fallback portable JDK.

## CI / Release

Workflow: `.github/workflows/release-build.yml`

- matrix build: `windows-latest` + `ubuntu-22.04`
- upload bundli z `src-tauri/target/release/bundle`
- dodatkowy job generuje pliki do paczki Arch w `packaging/arch/out`

## Arch packaging

Szkielet PKGBUILD:

- template: `packaging/arch/PKGBUILD.in`
- generator: `scripts/generate-arch-package.sh`
- output: `packaging/arch/out/PKGBUILD`
