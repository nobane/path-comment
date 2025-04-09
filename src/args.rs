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

    /// Keep other existing path comments in the file.
    /// By default, all path comments are removed from the file.
    #[arg(short, long, default_value_t = false)]
    pub keep: bool,

    /// If used, the --keep is ignored.
    #[arg(long, default_value_t = false)]
    pub clean: bool,

    /// Process folders that would normally be ignored (node_modules, venv, etc.)
    #[arg(short, long, default_value_t = false)]
    pub force: bool,

    /// Disable searching for a .git directory to determine the base path.
    /// If --base is not provided, uses the current working directory.
    #[arg(long, default_value_t = false)]
    pub no_git: bool,

    /// Disables processing files recursively
    #[arg(long, default_value_t = false)]
    pub no_recursive: bool,

    /// Disable merging ignore rules from .gitignore found in the base directory.
    #[arg(long, default_value_t = false)]
    pub no_ignore_merge: bool,

    /// File extensions to process (comma-separated), eg `rs,ts,toml`
    #[arg(short, long)]
    pub extensions: Option<String>,

    /// Configuration file for file extensions and comment styles
    #[arg(long = "config")]
    pub config_file: Option<String>,

    /// Dry run (don't modify files, just print what would be done)
    #[arg(short, long)]
    pub dry_run: bool,

    /// Force override a specific comment style to use (overrides config file)
    #[arg(short = 's', long, value_enum)]
    pub comment_style: Option<comments::Style>,

    /// Print configured extensions styles, then exit.
    #[arg(short, long)]
    pub print_extensions: bool,
}

impl Args {
    pub fn parse() -> Self {
        <Args as Parser>::parse()
    }
}
