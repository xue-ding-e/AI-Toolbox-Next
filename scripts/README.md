# Scripts

## transparent-white.js

Convert white/near-white pixels to transparent in a PNG image.

Default target (in-place):

```bash
node scripts/transparent-white.js
```

Custom input/output/threshold:

```bash
node scripts/transparent-white.js <input> [output] [threshold]
```

- `input`: source PNG path
- `output`: destination PNG path (defaults to input path)
- `threshold`: 0-255, treats any pixel with RGB >= threshold as white (default: 245)

## Regenerate Tauri icons

The main icon source is `public/icon.png`. Regenerate platform icons with:

```bash
pnpm tauri icon "public/icon.png"
```

This updates the generated icons in the Tauri icon output directories.
