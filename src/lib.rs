use anyhow::{Context, Result, bail};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat};
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Jpeg,
    Png,
    Webp,
    Gif,
    Bmp,
    Tiff,
}

impl OutputFormat {
    fn from_extension(extension: &str) -> Option<Self> {
        match normalize_extension(extension).ok()?.as_str() {
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "png" => Some(Self::Png),
            "webp" => Some(Self::Webp),
            "gif" => Some(Self::Gif),
            "bmp" => Some(Self::Bmp),
            "tif" | "tiff" => Some(Self::Tiff),
            _ => None,
        }
    }

    fn to_image_format(self) -> ImageFormat {
        match self {
            Self::Jpeg => ImageFormat::Jpeg,
            Self::Png => ImageFormat::Png,
            Self::Webp => ImageFormat::WebP,
            Self::Gif => ImageFormat::Gif,
            Self::Bmp => ImageFormat::Bmp,
            Self::Tiff => ImageFormat::Tiff,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeMode {
    Fit,
    Exact,
    Fill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResizeOptions {
    pub width: u32,
    pub height: u32,
    pub mode: ResizeMode,
}

impl ResizeOptions {
    pub fn new(width: u32, height: u32, mode: ResizeMode) -> Result<Self> {
        if width == 0 || height == 0 {
            bail!("resize width and height must be greater than zero");
        }

        Ok(Self {
            width,
            height,
            mode,
        })
    }

    fn magick_geometry(self) -> String {
        match self.mode {
            ResizeMode::Fit => format!("{}x{}", self.width, self.height),
            ResizeMode::Exact => format!("{}x{}!", self.width, self.height),
            ResizeMode::Fill => format!("{}x{}^", self.width, self.height),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConvertOptions {
    pub overwrite: bool,
    pub quality: Option<u8>,
    pub resize: Option<ResizeOptions>,
    pub magick_fallback: bool,
    pub recover_corrupted: bool,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            overwrite: false,
            quality: None,
            resize: None,
            magick_fallback: true,
            recover_corrupted: true,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct BatchReport {
    pub converted: usize,
    pub skipped: usize,
    pub failed: usize,
}

pub fn convert_image_file(input: &Path, output: &Path, options: ConvertOptions) -> Result<()> {
    validate_input_and_output(input, output, options)?;

    let extension = output
        .extension()
        .and_then(|value| value.to_str())
        .context("output path must include a file extension")?;
    let extension = normalize_extension(extension)?;

    if let Some(format) = OutputFormat::from_extension(&extension) {
        let mut image = match decode_image(input) {
            Ok(image) => image,
            Err(decode_error) => {
                if options.magick_fallback && options.recover_corrupted {
                    return convert_with_magick_backend(input, output, options).with_context(|| {
                        format!(
                            "native decode failed, attempted ImageMagick fallback: {decode_error:#}"
                        )
                    });
                }
                return Err(decode_error);
            }
        };

        if let Some(resize) = options.resize {
            image = resize_image(image, resize);
        }

        return write_native_image(&image, output, format, options);
    }

    if !options.magick_fallback {
        bail!(
            "output extension `{extension}` is not natively supported and ImageMagick fallback is disabled"
        );
    }

    convert_with_magick_backend(input, output, options)
}

pub fn convert_directory(
    input_dir: &Path,
    output_dir: &Path,
    to_extension: &str,
    options: ConvertOptions,
    recursive: bool,
) -> Result<BatchReport> {
    if !input_dir.is_dir() {
        bail!("input directory not found: {}", input_dir.display());
    }

    fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "failed to create output directory: {}",
            output_dir.display()
        )
    })?;

    let to_extension = normalize_extension(to_extension)?;
    let files = collect_input_files(input_dir, recursive)?;
    let mut report = BatchReport::default();

    for source_path in files {
        let Ok(relative_path) = source_path.strip_prefix(input_dir) else {
            report.failed += 1;
            continue;
        };

        let mut target_path = output_dir.join(relative_path);
        target_path.set_extension(&to_extension);

        if target_path.exists() && !options.overwrite {
            report.skipped += 1;
            continue;
        }

        match convert_image_file(&source_path, &target_path, options) {
            Ok(()) => report.converted += 1,
            Err(_) => report.failed += 1,
        }
    }

    Ok(report)
}

fn validate_input_and_output(input: &Path, output: &Path, options: ConvertOptions) -> Result<()> {
    if !input.is_file() {
        bail!("input file not found: {}", input.display());
    }

    if output.exists() && !options.overwrite {
        bail!(
            "output file exists (use --overwrite to replace): {}",
            output.display()
        );
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory: {}", parent.display()))?;
    }

    Ok(())
}

fn decode_image(input: &Path) -> Result<DynamicImage> {
    let bytes = fs::read(input)
        .with_context(|| format!("failed to read input image: {}", input.display()))?;

    if let Ok(format) = image::guess_format(&bytes) {
        return image::load_from_memory_with_format(&bytes, format)
            .with_context(|| format!("failed to decode input image: {}", input.display()));
    }

    image::load_from_memory(&bytes)
        .with_context(|| format!("failed to decode input image: {}", input.display()))
}

fn resize_image(image: DynamicImage, resize: ResizeOptions) -> DynamicImage {
    match resize.mode {
        ResizeMode::Fit => image.resize(resize.width, resize.height, FilterType::Lanczos3),
        ResizeMode::Exact => image.resize_exact(resize.width, resize.height, FilterType::Lanczos3),
        ResizeMode::Fill => image.resize_to_fill(resize.width, resize.height, FilterType::Lanczos3),
    }
}

fn write_native_image(
    image: &DynamicImage,
    output: &Path,
    format: OutputFormat,
    options: ConvertOptions,
) -> Result<()> {
    match format {
        OutputFormat::Jpeg => {
            let quality = options.quality.unwrap_or(85);
            if !(1..=100).contains(&quality) {
                bail!("quality must be between 1 and 100");
            }

            let file = File::create(output)
                .with_context(|| format!("failed to create output file: {}", output.display()))?;
            let mut writer = BufWriter::new(file);
            let mut encoder = JpegEncoder::new_with_quality(&mut writer, quality);
            encoder
                .encode_image(image)
                .with_context(|| format!("failed to write jpeg: {}", output.display()))?;
        }
        _ => {
            image
                .save_with_format(output, format.to_image_format())
                .with_context(|| format!("failed to write image: {}", output.display()))?;
        }
    }

    Ok(())
}

fn convert_with_magick_backend(input: &Path, output: &Path, options: ConvertOptions) -> Result<()> {
    let mut command = Command::new("magick");
    command.arg(input);

    if let Some(resize) = options.resize {
        command.arg("-resize").arg(resize.magick_geometry());
    }

    if let Some(quality) = options.quality {
        if !(1..=100).contains(&quality) {
            bail!("quality must be between 1 and 100");
        }
        command.arg("-quality").arg(quality.to_string());
    }

    command.arg(output);

    let status = command.status().with_context(|| {
        "failed to run ImageMagick (`magick`). Install it first (macOS: `brew install imagemagick`)."
    })?;

    if !status.success() {
        bail!(
            "ImageMagick conversion failed: {} -> {}",
            input.display(),
            output.display()
        );
    }

    Ok(())
}

fn collect_input_files(input_dir: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if recursive {
        for entry in WalkDir::new(input_dir)
            .into_iter()
            .filter_map(|entry| entry.ok())
        {
            if entry.file_type().is_file() {
                files.push(entry.into_path());
            }
        }
    } else {
        for entry in fs::read_dir(input_dir)
            .with_context(|| format!("failed to read directory: {}", input_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                files.push(path);
            }
        }
    }

    Ok(files)
}

fn normalize_extension(extension: &str) -> Result<String> {
    let extension = extension.trim().trim_start_matches('.');
    if extension.is_empty() {
        bail!("format/extension cannot be empty");
    }

    Ok(extension.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_supported_output_format_extensions() {
        assert_eq!(
            OutputFormat::from_extension("jpg"),
            Some(OutputFormat::Jpeg)
        );
        assert_eq!(
            OutputFormat::from_extension("jpeg"),
            Some(OutputFormat::Jpeg)
        );
        assert_eq!(OutputFormat::from_extension("png"), Some(OutputFormat::Png));
        assert_eq!(
            OutputFormat::from_extension("webp"),
            Some(OutputFormat::Webp)
        );
        assert_eq!(OutputFormat::from_extension("gif"), Some(OutputFormat::Gif));
        assert_eq!(OutputFormat::from_extension("bmp"), Some(OutputFormat::Bmp));
        assert_eq!(
            OutputFormat::from_extension("tif"),
            Some(OutputFormat::Tiff)
        );
        assert_eq!(
            OutputFormat::from_extension("tiff"),
            Some(OutputFormat::Tiff)
        );
    }

    #[test]
    fn reject_unknown_output_extension() {
        assert_eq!(OutputFormat::from_extension("avif"), None);
    }

    #[test]
    fn normalize_extension_values() {
        assert_eq!(normalize_extension(" JPG ").unwrap(), "jpg");
        assert_eq!(normalize_extension(".WebP").unwrap(), "webp");
    }

    #[test]
    fn validate_resize_bounds() {
        assert!(ResizeOptions::new(0, 200, ResizeMode::Fit).is_err());
        assert!(ResizeOptions::new(200, 0, ResizeMode::Fit).is_err());
        assert!(ResizeOptions::new(200, 100, ResizeMode::Fit).is_ok());
    }
}
