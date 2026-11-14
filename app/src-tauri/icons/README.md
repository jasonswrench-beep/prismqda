# Application icon

These files come from the PrismQDA prism in `assets/brand`:

- `icon.ico` is `assets/brand/app-icon.ico` unchanged (seven sizes: 16, 24, 32, 48, 64, 128, 256 pixels).
- `32x32.png`, `128x128.png`, `128x128@2x.png` (256 pixels), and `icon.png` (512 pixels) are `assets/brand/app-icon.svg`
  drawn at those sizes on a transparent background.

To change the icon, replace `assets/brand/app-icon.svg` and `app-icon.ico`, draw the four PNG sizes from the SVG again,
and rebuild the installer with `app/BUILD-INSTALLER.ps1`. The earlier icon here was a placeholder drawn by a script,
which has been removed.
