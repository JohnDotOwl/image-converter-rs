use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use image_converter_rs::{
    ConvertOptions, ResizeMode, ResizeOptions, convert_directory, convert_image_file,
};
use std::path::PathBuf;
use std::fs;

#[derive(Parser)]
#[command(
    name = "image-converter-rs",
    version,
    about = "Focused Rust image conversion CLI"
)]
struct Cli {
    /// List supported formats and exit
    #[arg(long)]
    list_formats: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Info {
        input: PathBuf,
    },
    Convert {
        input: PathBuf,
        output: PathBuf,
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=100))]
        quality: Option<u8>,
        #[arg(long, value_parser = parse_resize)]
        resize: Option<ResizeInput>,
        #[arg(long, value_enum, default_value_t = ResizeModeArg::Fit)]
        resize_mode: ResizeModeArg,
        #[arg(long, default_value_t = false)]
        overwrite: bool,
        #[arg(long, default_value_t = false)]
        preserve_filename: bool,
    },
    Batch {
        input_dir: PathBuf,
        output_dir: PathBuf,
        #[arg(long, value_name = "EXTENSION")]
        to: String,
        #[arg(long, default_value_t = false)]
        recursive: bool,
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=100))]
        quality: Option<u8>,
        #[arg(long, value_parser = parse_resize)]
        resize: Option<ResizeInput>,
        #[arg(long, value_enum, default_value_t = ResizeModeArg::Fit)]
        resize_mode: ResizeModeArg,
        #[arg(long, default_value_t = false)]
        overwrite: bool,
    },
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    if cli.list_formats {
        println!("Supported formats: jpg, jpeg, png, webp, gif, bmp, tiff");
        return Ok(());
    }

    match cli.command {
        Commands::Info { input } => {
            let img = image::open(&input)
                .with_context(|| format!("failed to open {}", input.display()))?;
            let metadata = img.metadata();
            let size = fs::metadata(&input)
                .map(|m| m.len())
                .unwrap_or(0);
            let format = input.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("unknown");
            println!("File: {}", input.display());
            println!("Format: {}", format);
            println!("Dimensions: {}x{}", img.width(), img.height());
            println!("Size: {:.2} KB", size as f64 / 1024.0);
            println!("Color type: {:?}", metadata.color_type);
            println!("Bit depth: {}", metadata.bit_depth);
        }
        Commands::Convert {
            input,
            output,
            quality,
            resize,
            resize_mode,
            overwrite,
            preserve_filename,
        } => {
            let options = build_convert_options(overwrite, quality, resize, resize_mode)?;
            let output = if preserve_filename {
                let stem = input.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("output");
                output.with_file_name(stem)
            } else {
                output
            };
            convert_image_file(&input, &output, options).with_context(|| {
                format!(
                    "failed conversion from {} to {}",
                    input.display(),
                    output.display()
                )
            })?;
            println!("converted {} -> {}", input.display(), output.display());
        }
        Commands::Batch {
            input_dir,
            output_dir,
            to,
            recursive,
            quality,
            resize,
            resize_mode,
            overwrite,
        } => {
            let options = build_convert_options(overwrite, quality, resize, resize_mode)?;
            let report = convert_directory(&input_dir, &output_dir, &to, options, recursive)
                .with_context(|| {
                    format!(
                        "failed batch conversion from {} to {}",
                        input_dir.display(),
                        output_dir.display()
                    )
                })?;

            println!(
                "batch complete: converted={}, failed={}, skipped={}",
                report.converted, report.failed, report.skipped
            );
        }
    }

    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct ResizeInput {
    width: u32,
    height: u32,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ResizeModeArg {
    Fit,
    Exact,
    Fill,
}

impl From<ResizeModeArg> for ResizeMode {
    fn from(value: ResizeModeArg) -> Self {
        match value {
            ResizeModeArg::Fit => ResizeMode::Fit,
            ResizeModeArg::Exact => ResizeMode::Exact,
            ResizeModeArg::Fill => ResizeMode::Fill,
        }
    }
}

fn parse_resize(value: &str) -> std::result::Result<ResizeInput, String> {
    let normalized = value.trim().to_ascii_lowercase();
    let (width, height) = normalized
        .split_once('x')
        .ok_or_else(|| "resize must be in WIDTHxHEIGHT format (example: 1920x1080)".to_string())?;

    let width = width
        .parse::<u32>()
        .map_err(|_| "resize width must be an integer".to_string())?;
    let height = height
        .parse::<u32>()
        .map_err(|_| "resize height must be an integer".to_string())?;

    if width == 0 || height == 0 {
        return Err("resize width and height must be greater than zero".to_string());
    }

    Ok(ResizeInput { width, height })
}

fn build_convert_options(
    overwrite: bool,
    quality: Option<u8>,
    resize: Option<ResizeInput>,
    resize_mode: ResizeModeArg,
) -> Result<ConvertOptions> {
    let resize = resize
        .map(|value| ResizeOptions::new(value.width, value.height, resize_mode.into()))
        .transpose()?;

    Ok(ConvertOptions {
        overwrite,
        quality,
        resize,
        ..ConvertOptions::default()
    })
}
