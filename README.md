# image-converter-rs

Focused Rust image conversion toolkit with:
- A CLI for one-off and batch conversion
- A reusable library API for integration into other Rust projects
- Native Rust conversion for common formats
- Automatic ImageMagick fallback for unsupported or corrupted images

## Format coverage

- Native output: `jpg`, `png`, `webp`, `gif`, `bmp`, `tiff`
- Any output extension is supported when ImageMagick fallback is enabled (default)
- Corrupted/partially broken images can be retried through ImageMagick fallback

## CLI usage

```bash
cargo run -- convert ./in.png ./out.jpg
cargo run -- convert ./in.png ./out.jpg --quality 82 --overwrite
```

```bash
cargo run -- convert ./in.tif ./out.avif
cargo run -- convert ./in.jpg ./out.webp --resize 1920x1080 --resize-mode fit
```

```bash
cargo run -- batch ./images ./converted --to webp --recursive --resize 1600x1600
cargo run -- batch ./images ./converted --to avif --recursive
```

## Library usage

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

## Notes

- `--quality` applies to JPEG natively and is forwarded to ImageMagick fallback conversions.
- `--resize WIDTHxHEIGHT` supports `fit` (keep aspect ratio) and `exact` (force exact dimensions).
- ImageMagick fallback is automatic when native conversion cannot decode or support a format.
- Automatic fallback requires `magick` to be installed and available in your PATH.
- Existing files are not overwritten unless you pass `--overwrite`.
