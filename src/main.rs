use std::{
    env,
    path::{Path, PathBuf},
    process,
};

#[cfg(test)]
mod test;

mod args;
mod cli;
mod comments;

/// Searches upwards from the `start_dir` for a directory containing `.git`.
/// Returns the path to the directory containing `.git` if found, otherwise None.
fn find_git_root(start_dir: &Path) -> Option<PathBuf> {
    let mut current = start_dir.to_path_buf();
    loop {
        if current.join(".git").is_dir() {
            return Some(current);
        }
        if !current.pop() {
            // Reached root directory
            return None;
        }
    }
}

fn main() {
    let args = args::Args::parse();

    let mut git_base_used = false; // Track if base was determined via .git

    // Determine the base directory for relative paths
    let base_dir = match args.base {
        Some(ref base) => PathBuf::from(base),
        None => {
            if args.no_git {
                // Git search disabled, use CWD
                env::current_dir().expect("Failed to get current directory")
            } else {
                // Try to find .git root starting from the target directory
                let target_dir = PathBuf::from(&args.dir);
                let absolute_target_dir = target_dir.canonicalize().unwrap_or_else(|e| {
                    eprintln!("Error accessing target directory '{}': {}", args.dir, e);
                    process::exit(1);
                });

                if let Some(git_root) = find_git_root(&absolute_target_dir) {
                    println!("Found .git repository root at: {}", git_root.display());
                    git_base_used = true;
                    git_root
                } else {
                    // No .git found, fall back to CWD
                    println!(
                        "No .git directory found upwards from target. Using current working directory as base."
                    );
                    env::current_dir().expect("Failed to get current directory")
                }
            }
        }
    };

    // Canonicalize base_dir to handle relative paths robustly
    let base_dir = base_dir.canonicalize().unwrap_or_else(|e| {
        eprintln!(
            "Error accessing base directory '{}': {}",
            base_dir.display(),
            e
        );
        process::exit(1);
    });

    let gitignore_path = if git_base_used && !args.no_ignore_merge {
        Some(base_dir.join(".gitignore"))
    } else {
        None
    };

    // Run the file processor, passing the determined base dir and potential gitignore path
    cli::Cli::new_arc(args, base_dir, gitignore_path).run();
}
