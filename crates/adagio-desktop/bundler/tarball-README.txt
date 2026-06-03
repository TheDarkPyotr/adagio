Adagio — Portable Linux Archive
================================

This archive contains three Adagio binaries:

  adagio-desktop   — the graphical sync client
  adagio-daemon    — the background sync engine (required)
  adagio-cli       — the command-line control tool

Quick start
-----------
1. Extract the archive to a directory of your choice:
     tar -xzf adagio_*_linux_amd64.tar.gz

2. Start the daemon (runs in the background):
     ./adagio-daemon &

3. Launch the desktop app:
     ./adagio-desktop

Optional: install to your system
---------------------------------
For a proper system install with app-launcher integration,
use the .deb or .rpm package from the same release page instead.

If you prefer to use this tarball on a system without a package manager,
you can manually install the desktop entry:

1. Copy the binaries to a directory on your PATH:
     sudo cp adagio-desktop adagio-daemon adagio-cli /usr/local/bin/

2. Download adagio.desktop from the release page and install it:
     cp adagio.desktop ~/.local/share/applications/

3. Install the Adagio icon (download adagio-256x256.png from the release):
     mkdir -p ~/.local/share/icons/hicolor/256x256/apps/
     cp adagio-256x256.png ~/.local/share/icons/hicolor/256x256/apps/adagio.png
     gtk-update-icon-cache ~/.local/share/icons/hicolor/ 2>/dev/null || true

Uninstall
---------
Remove the binaries you copied and delete the .desktop file.

Issues and source
-----------------
https://github.com/yourusername/adagio
