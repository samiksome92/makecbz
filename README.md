# makecbz
A simple program which takes a directory with images and creates a cbz from it.

## Installation
Either download a release directly from [releases](https://github.com/samiksome92/makecbz/releases) or use `cargo`:

    cargo install --git https://github.com/samiksome92/makecbz

## Usage

    makecbz [OPTIONS] <DIRS>...

Arguments:

    <DIRS>...  Directory(s) containing images

Options:

    -n, --no-rename  Don't rename files
    -d, --delete     Delete original files
    -v, --verify     Verify image data
        --overwrite  Overwrite output file if it exists
    -h, --help       Print help
    -V, --version    Print version

Scans each directory provided for images, non-image files and excluded files. If `--verify` is specified the image files are decoded to ensure there is no corruption.

By default all images are renamed numerically starting from `1`, unless `--no-rename` is specified in which case file names are kept as-is.

If the output file already exists the user is prompted whether it should be overwritten. If `--overwrite` is provided the output file is automatically overwritten.

`--delete` can be specified to delete the original directories after cbz creation.