# Kilowatt Tycoon Logo Kit

## Brand Colors

| Role     | Value                              |
|----------|------------------------------------|
| Primary  | Radial gradient `#e0dd47` → `#f58820` (yellow → orange) |
| Dark     | `#121212`                          |
| White    | `#ffffff`                          |

## Typeface

The wordmark uses **Unbounded** (variable weight). The font file is at `Fonts/Unbounded-VariableFont_wght.ttf`.

## File Formats

Each variant ships as EPS, JPG, PNG, and SVG. PNGs are exported at 1024px with transparent backgrounds. SVGs include the source gradient definitions and may contain background rectangles for composition.

---

## Full Wordmark (Logo_Logo 1–5)

The mark (rounded-rect with four lightning bolts) alongside the "KILOWATT TYCOON" text.

| File         | Mark                          | Text             | Background  | Use                          |
|--------------|-------------------------------|------------------|-------------|------------------------------|
| Logo_Logo 1  | Gradient, dark bolts          | Dark (`#121212`) | Transparent | Light backgrounds            |
| Logo_Logo 2  | Dark, white bolt cutouts      | Dark (`#121212`) | Transparent | Light backgrounds (mono)     |
| Logo_Logo 3  | Gradient, dark bolts          | White            | Transparent | **Dark backgrounds**         |
| Logo_Logo 4  | Dark, white bolt cutouts      | Dark (`#121212`) | Gradient    | Standalone / any background  |
| Logo_Logo 5  | White, dark bolt cutouts      | White            | Transparent | Dark backgrounds (mono)      |

---

## Icon Mark — Raw Bolts (Logo_mark 1–4)

Four lightning bolt shapes without the rounded-rect container.

| File         | Bolts                                   | Background  | Use                              |
|--------------|-----------------------------------------|-------------|----------------------------------|
| Logo_mark 1  | Dark (`#121212`)                        | Gradient    | App icons, social avatars        |
| Logo_mark 2  | Dark (`#121212`)                        | Transparent | Light backgrounds                |
| Logo_mark 3  | Gradient (each bolt individually fills) | Transparent | Light or transparent backgrounds |
| Logo_mark 4  | White                                   | Transparent | Dark backgrounds                 |

---

## Icon Mark — Contained (Logo_mark 5–9)

Four lightning bolts inside a rounded-rect container.

| File         | Container                | Bolts / Cutouts              | Background  | Use                             |
|--------------|--------------------------|------------------------------|-------------|---------------------------------|
| Logo_mark 5  | Dark, gradient bolt cutouts | Gradient shows through     | Transparent | App icons, any background       |
| Logo_mark 6  | Gradient                 | Dark cutouts                 | Transparent | **Dark backgrounds, nav icons** |
| Logo_mark 7  | Gradient, dark bolts on top | Dark bolts overlay         | Transparent | **Favicons**, light backgrounds |
| Logo_mark 8  | White, dark bolt cutouts | Dark shows through           | Transparent | Dark backgrounds (mono)         |
| Logo_mark 9  | Dark, white bolt cutouts | White shows through          | Transparent | Light backgrounds (mono)        |

---

## Usage on kwtycoon.com

The website has a dark theme (`#0f172a` → `#1e293b`). The following files are used:

| Location       | File             | Why                                                                 |
|----------------|------------------|---------------------------------------------------------------------|
| Hero section   | Logo_Logo 3      | Full wordmark with gradient mark + white text — readable on dark bg |
| Nav bar icon   | Logo_mark 6      | Gradient contained mark — compact, recognizable at 40px             |
| Footer icon    | Logo_mark 6      | Same as nav for consistency                                         |
| Favicon        | Logo_mark 7      | Gradient mark with dark bolts on transparent bg — crisp at 16–32px  |
| Apple touch    | Logo_mark 7      | Same source, resized to 180px                                       |
| OG / Twitter   | Logo_Logo 3      | Full wordmark for social share previews                             |

Web-friendly copies live in `assets/ui/`:

```
assets/ui/logo_full.png         ← Logo_Logo 3
assets/ui/logo_mark.png         ← Logo_mark 6
assets/ui/logo_mark_light.png   ← Logo_mark 7
assets/ui/favicon-32x32.png     ← Logo_mark 7, resized
assets/ui/favicon-16x16.png     ← Logo_mark 7, resized
assets/ui/apple-touch-icon.png  ← Logo_mark 7, resized
```
