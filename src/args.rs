// src/args.rs
use clap::Parser;

use crate::comments;

/// CLI tool to prepend file paths as comments to source code files
#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Directory to process files in
    #[arg(required = true)]
    pub dir: String,

    /// Base directory for calculating relative paths.
    /// If not provided, searches upwards for a .git directory to use as the base.
    /// Falls back to the current working directory if no .git directory is found.
    #[arg(short, long)]
    pub base: Option<String>,

    /// Disable searching for a .git directory to determine the base path.
    /// If --base is not provided, uses the current working directory.
    #[arg(long, default_value_t = false)]
    pub no_git_base: bool,

    /// File extensions to process (comma-separated)
    #[arg(short, long)]
    pub extensions: Option<String>,

    /// Configuration file for file extensions and comment styles
    #[arg(long = "config")]
    pub config_file: Option<String>,

    /// Process files recursively
    #[arg(short, long, default_value_t = true)]
    pub recursive: bool,

    /// Dry run (don't modify files, just print what would be done)
    #[arg(short, long)]
    pub dry_run: bool,

    /// Comment style to use (overrides config file)
    #[arg(short = 's', long, value_enum)]
    pub comment_style: Option<comments::Style>,

    /// Force processing of dependency directories (node_modules, venv, etc.)
    #[arg(short, long)]
    pub force: bool,

    /// Strip existing path comments
    #[arg(short = 'x', long, default_value_t = true)]
    pub strip: bool,

    /// Remove existing path comments instead of adding/updating them.
    /// If used, the --strip flag is ignored.
    #[arg(long, default_value_t = false)]
    pub clean: bool,

    /// Print configured extensions styles, then exit.
    #[arg(short, long)]
    pub print_extensions: bool,

    /// Disable merging ignore rules from .gitignore found in the base directory.
    #[arg(long, default_value_t = false)]
    pub no_ignore_merge: bool,
}

impl Args {
    pub fn parse() -> Self {
        <Args as Parser>::parse()
    }
}
