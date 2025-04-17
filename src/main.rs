use anyhow::{Context, Result};
use clap::Parser;
use colored::Colorize;
use image::{ImageFormat, ImageReader};
use indicatif::{ProgressBar, ProgressStyle};
use std::{
    cmp::max,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use zip::{ZipWriter, write::SimpleFileOptions};

/// Supported image formats.
const FORMATS: [ImageFormat; 4] = [
    ImageFormat::Jpeg,
    ImageFormat::Png,
    ImageFormat::Gif,
    ImageFormat::WebP,
];
/// Excluded file names.
const EXCLUDED_FILES: [&str; 1] = ["ComicInfo.xml"];

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    #[arg(required = true, help = "Directory(s) containing images")]
    dirs: Vec<PathBuf>,
    #[arg(short, long, help = "Don't rename files")]
    no_rename: bool,
    #[arg(short, long, help = "Delete original files")]
    delete: bool,
    #[arg(short, long, help = "Verify image data")]
    verify: bool,
    #[arg(long, help = "Overwrite output file if it exists")]
    overwrite: bool,
}

/// Image information.
///
/// Stores the path and guessed format of an image.
struct ImageInfo {
    path: PathBuf,
    format: ImageFormat,
}

/// Returns a sorted list of all paths in the provided directory.
///
/// Propagates any error with added context.
fn get_paths<P>(dir: P) -> Result<Vec<PathBuf>>
where
    P: AsRef<Path>,
{
    let dir = dir.as_ref();
    let mut paths = Vec::new();
    for entry in
        fs::read_dir(dir).with_context(|| format!("Failed to read directory {}", dir.display()))?
    {
        let entry = entry.with_context(|| format!("Error while reading {}", dir.display()))?;
        paths.push(entry.path());
    }

    paths.sort();
    Ok(paths)
}

/// Checks if a file is a valid image.
///
/// If it is a supported image, returns an ImageInfo with the path and guessed format, else returns None. If `verify`
/// is true the image is decoded to ensure there is no corruption. Propagates any error with added context.
fn check_file<P>(file: P, verify: bool) -> Result<Option<ImageInfo>>
where
    P: AsRef<Path>,
{
    let file = file.as_ref();
    let mut image = ImageReader::open(file)
        .with_context(|| format!("Failed to open {} for reading", file.display()))?;
    image.clear_format(); // Clear format guessed from file name.
    image = image
        .with_guessed_format()
        .with_context(|| format!("Failed to read file {}", file.display()))?;

    if let Some(format) = image.format() {
        if FORMATS.contains(&format) {
            if verify && image.decode().is_err() {
                return Ok(None);
            }

            return Ok(Some(ImageInfo {
                path: file.to_path_buf(),
                format,
            }));
        }
    }

    Ok(None)
}

/// Checks a directory for images.
///
/// Returns a tuple of supported image files, non-image files or non-supported files and excluded files. If `verify` is
/// true all images are decoded to ensure there is no corruption. Propgates any error.
fn check_dir<P>(dir: P, verify: bool) -> Result<(Vec<ImageInfo>, Vec<PathBuf>, Vec<PathBuf>)>
where
    P: AsRef<Path>,
{
    println!("Checking directory...");
    let mut imgs = Vec::new();
    let mut non_imgs = Vec::new();
    let mut excluded = Vec::new();
    let paths = get_paths(dir)?;
    let bar = ProgressBar::new(paths.len() as u64);
    bar.set_style(
        ProgressStyle::with_template("Verifying files {bar:40.white/white.dim} {pos}/{len}")
            .unwrap()
            .progress_chars("━╸━"),
    );
    for path in paths {
        if !path.is_file() {
            non_imgs.push(path);
        } else if EXCLUDED_FILES.contains(
            &path
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default(),
        ) {
            excluded.push(path);
        } else {
            match check_file(&path, verify)? {
                Some(image_info) => {
                    imgs.push(image_info);
                }
                None => {
                    non_imgs.push(path);
                }
            }
        }

        if verify {
            bar.inc(1);
        }
    }
    if verify {
        bar.finish();
    }

    Ok((imgs, non_imgs, excluded))
}

/// Creates a cbz file with images from given directory.
///
/// All image files are renamed to a numeric format unless `no_rename` is true. If `delete` is true `dir` is deleted
/// after creating the cbz. Images can be verified using `verified`. Unless `overwrite` is true if the output file
/// exists the user is prompted for overwriting it. Errors are propagated.
fn create_cbz<P>(dir: P, no_rename: bool, delete: bool, verify: bool, overwrite: bool) -> Result<()>
where
    P: AsRef<Path>,
{
    // Check if output file already exists.
    let dir = dir.as_ref();
    let zip_path = dir.with_extension("cbz");
    if !overwrite && zip_path.exists() {
        print!(
            "{} {} already exists. Overwrite? [y/N] ",
            "WARNING:".yellow().bold(),
            zip_path.display()
        );
        io::stdout().flush().context("Failed to flush stdout")?;
        let mut choice = String::new();
        io::stdin()
            .read_line(&mut choice)
            .context("Failed to read user input")?;
        choice = choice.to_lowercase();
        let choice = choice.trim();

        if choice != "y" && choice != "yes" {
            println!("Not creating cbz");
            return Ok(());
        }
    }

    // Check directory for images, non images and excluded files.
    let (imgs, non_imgs, excluded) = check_dir(dir, verify)?;

    if !non_imgs.is_empty() {
        println!("Found {} non-images/unsupported images...", non_imgs.len());
        for path in non_imgs {
            println!("\t{}", path.display());
        }
        return Ok(());
    }

    // Create cbz.
    println!("Creating cbz...");
    let mut zip = ZipWriter::new(
        fs::File::create(&zip_path)
            .with_context(|| format!("Failed to create file {}", zip_path.display()))?,
    );
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (idx, img) in imgs.iter().enumerate() {
        let buf = fs::read(&img.path)
            .with_context(|| format!("Failed to read file {}", img.path.display()))?;
        let file_name = if no_rename {
            img.path
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
                .to_string()
        } else {
            format!(
                "{:0pad$}.{}",
                idx + 1,
                img.format.extensions_str()[0],
                pad = max(imgs.len().to_string().len(), 2)
            )
        };
        zip.start_file(&file_name, options)
            .with_context(|| format!("Failed to add {} to {}", file_name, zip_path.display()))?;
        zip.write_all(&buf)
            .with_context(|| format!("Failed to write {} to {}", file_name, zip_path.display()))?;
    }
    for path in excluded {
        let buf =
            fs::read(&path).with_context(|| format!("Failed to read file {}", path.display()))?;
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();
        zip.start_file(file_name, options)
            .with_context(|| format!("Failed to add {} to {}", file_name, zip_path.display()))?;
        zip.write_all(&buf)
            .with_context(|| format!("Failed to write {} to {}", file_name, zip_path.display()))?;
    }
    zip.finish()
        .with_context(|| format!("Failed to finalize {}", zip_path.display()))?;

    // Delete directory.
    if delete {
        println!("Deleting original files and directory...");
        fs::remove_dir_all(dir)
            .with_context(|| format!("Failed to remove directory {}", dir.display()))?;
    }

    Ok(())
}

/// Parse command line arguments and call `create_cbz` for each provided directory.
fn main() {
    let args = Args::parse();

    for dir in args.dirs {
        println!("Processing {}...", dir.display());
        if let Err(e) = create_cbz(
            dir,
            args.no_rename,
            args.delete,
            args.verify,
            args.overwrite,
        ) {
            eprintln!("{} {e:#}", "ERROR:".red().bold());
        }
    }
}
