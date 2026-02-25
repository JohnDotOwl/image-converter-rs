# image-converter-rs

A fast, lightweight Rust command-line tool and library for converting images between formats. Supports PNG, JPEG, WebP, GIF, BMP, TIFF natively with automatic ImageMagick fallback for formats like AVIF, HEIC, and more.

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/Rust-1.85+-orange.svg)](https://www.rust-lang.org/)

## Features

- **Batch image conversion** — convert entire directories of images in one command, with recursive directory traversal
- **Image resizing** — resize images during conversion with fit (preserve aspect ratio) or exact mode
- **Quality control** — set JPEG quality for native conversions and ImageMagick fallback
- **Corrupted image recovery** — automatically retry broken or partially corrupted images through ImageMagick
- **ImageMagick fallback** — seamlessly handles any format ImageMagick supports (AVIF, HEIC, SVG, ICO, RAW, and more)
- **Native Rust performance** — fast, dependency-free conversion for common formats using the `image` crate

## Supported Formats

| Format | Native (built-in) | ImageMagick fallback |
|--------|-------------------|---------------------|
| JPEG / JPG | Read & Write | Read & Write |
| PNG | Read & Write | Read & Write |
| WebP | Read & Write | Read & Write |
| GIF | Read & Write | Read & Write |
| BMP | Read & Write | Read & Write |
| TIFF | Read & Write | Read & Write |
| AVIF | — | Read & Write |
| HEIC / HEIF | — | Read & Write |
| SVG | — | Read only |
| ICO | — | Read & Write |
| RAW (CR2, NEF, ARW, etc.) | — | Read only |
| Any other ImageMagick format | — | Depends on IM build |

## Supported Conversions

Every conversion pair listed below works out of the box. This is not an exhaustive list — any format pair that ImageMagick supports can be used via the automatic fallback.

### Native conversions (no external dependencies)

| | JPG | PNG | WebP | GIF | BMP | TIFF |
|---|---|---|---|---|---|---|
| **JPG** | — | Convert JPG to PNG | Convert JPG to WebP | Convert JPG to GIF | Convert JPG to BMP | Convert JPG to TIFF |
| **PNG** | Convert PNG to JPG | — | Convert PNG to WebP | Convert PNG to GIF | Convert PNG to BMP | Convert PNG to TIFF |
| **WebP** | Convert WebP to JPG | Convert WebP to PNG | — | Convert WebP to GIF | Convert WebP to BMP | Convert WebP to TIFF |
| **GIF** | Convert GIF to JPG | Convert GIF to PNG | Convert GIF to WebP | — | Convert GIF to BMP | Convert GIF to TIFF |
| **BMP** | Convert BMP to JPG | Convert BMP to PNG | Convert BMP to WebP | Convert BMP to GIF | — | Convert BMP to TIFF |
| **TIFF** | Convert TIFF to JPG | Convert TIFF to PNG | Convert TIFF to WebP | Convert TIFF to GIF | Convert TIFF to BMP | — |

### ImageMagick fallback conversions (requires `magick` in PATH)

- Convert JPG to AVIF | Convert AVIF to JPG
- Convert PNG to AVIF | Convert AVIF to PNG
- Convert WebP to AVIF | Convert AVIF to WebP
- Convert HEIC to JPG | Convert HEIC to PNG | Convert HEIC to WebP
- Convert JPG to HEIC | Convert PNG to HEIC
- Convert RAW to JPG | Convert RAW to PNG
- Convert SVG to PNG | Convert SVG to JPG
- Convert ICO to PNG | Convert PNG to ICO
- And any other format pair that ImageMagick supports

## Installation

### From source (requires Rust 1.85+)

```bash
git clone https://github.com/JohnDotOwl/image-converter-rs.git
cd image-converter-rs
cargo install --path .
```

After installation, the `image-converter-rs` binary is available system-wide.

### Optional: ImageMagick

For AVIF, HEIC, SVG, RAW, and other extended format support, install [ImageMagick](https://imagemagick.org/):

```bash
# macOS
brew install imagemagick

# Ubuntu / Debian
sudo apt install imagemagick

# Windows (via Chocolatey)
choco install imagemagick
```

## Usage

### Single File Conversion

```bash
# Convert PNG to JPG
image-converter-rs convert ./photo.png ./photo.jpg

# Convert JPG to WebP with quality setting
image-converter-rs convert ./photo.jpg ./photo.webp --quality 85

# Convert TIFF to AVIF (uses ImageMagick fallback)
image-converter-rs convert ./scan.tif ./scan.avif

# Convert and resize to fit within 1920x1080
image-converter-rs convert ./photo.jpg ./photo.webp --resize 1920x1080 --resize-mode fit
```

### Batch Conversion

```bash
# Convert all images in a directory to WebP
image-converter-rs batch ./images ./converted --to webp

# Recursive batch conversion with resizing
image-converter-rs batch ./images ./converted --to webp --recursive --resize 1600x1600

# Batch convert to AVIF
image-converter-rs batch ./images ./converted --to avif --recursive
```

### Resize Images

```bash
# Fit within dimensions (preserves aspect ratio)
image-converter-rs convert ./in.jpg ./out.jpg --resize 800x600 --resize-mode fit

# Exact dimensions (stretches to match)
image-converter-rs convert ./in.jpg ./out.jpg --resize 800x600 --resize-mode exact
```

## Library Usage

Use `image-converter-rs` as a Rust library in your own projects:

```rust
use image_converter_rs::{convert_image_file, ConvertOptions, ResizeMode, ResizeOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let options = ConvertOptions {
        overwrite: true,
        quality: Some(85),
        resize: Some(ResizeOptions::new(1920, 1080, ResizeMode::Fit)?),
        magick_fallback: true,
        recover_corrupted: true,
    };
    convert_image_file(Path::new("in.png"), Path::new("out.jpg"), options)?;
    Ok(())
}
```

## How It Works

`image-converter-rs` uses a two-tier conversion pipeline:

1. **Native Rust path** — For supported format pairs (JPEG, PNG, WebP, GIF, BMP, TIFF), the tool uses the [`image`](https://crates.io/crates/image) crate for fast, dependency-free decoding and encoding. Quality settings and resize operations are applied natively.

2. **ImageMagick fallback** — When the native path cannot handle a format (e.g., AVIF, HEIC, SVG, RAW) or when an image is corrupted and fails to decode, the tool automatically shells out to ImageMagick's `magick convert` command. This provides broad format coverage without sacrificing performance for common conversions.

The fallback is transparent — you use the same CLI interface regardless of which path handles the conversion.

## License

Licensed under the [Apache License 2.0](LICENSE).
