// src/cli.rs
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use ignore::{DirEntry, WalkBuilder, WalkState}; // Added DirEntry import

use crate::{args, comments};

pub struct Cli {
    args: args::Args,
    base_dir: PathBuf,
    processed_count: Arc<AtomicUsize>,
    skipped_count: Arc<AtomicUsize>,
    extension_styles: HashMap<String, comments::Style>,
    ignored_dirs: HashSet<String>,
}

const ANSI_RESET: &str = "\x1b[0m";

fn added(s: &str) -> String {
    const ANSI_GREEN: &str = "\x1b[32m";
    format!("{ANSI_GREEN}+ {s}{ANSI_RESET}")
}

fn removed(s: &str) -> String {
    const ANSI_RED: &str = "\x1b[31m";
    format!("{ANSI_RED}- {s}{ANSI_RESET}")
}

fn no_change(s: &str) -> String {
    const ANSI_YELLOW: &str = "\x1b[33m";
    format!("{ANSI_YELLOW} {s}{ANSI_RESET}")
}

const DEFAULT_IGNORE_CONFIG: &str = include_str!("ignore.cfg");

fn load_ignored_dirs(gitignore_path: Option<&Path>) -> HashSet<String> {
    let mut ignored = HashSet::new();

    // Load defaults from ignore.cfg
    for line in DEFAULT_IGNORE_CONFIG.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            ignored.insert(trimmed.to_string());
        }
    }

    // Merge from .gitignore if provided and exists
    if let Some(path) = gitignore_path {
        if path.is_file() {
            println!("Merging ignore rules from {}", path.display());
            if let Ok(file) = fs::File::open(path) {
                let reader = BufReader::new(file);
                for line_content in reader.lines().map_while(Result::ok) {
                    // Handle inline comments by splitting at '#'
                    let line_before_comment = line_content.split('#').next().unwrap_or("").trim();

                    // Use line_before_comment for checks instead of trimmed
                    if !line_before_comment.is_empty()
                        // No need to check for '#' start anymore as split handles it
                        && !line_before_comment.contains('*')
                        && !line_before_comment.contains('?')
                        && !line_before_comment.contains('[')
                        && !line_before_comment.contains('!')
                        && !line_before_comment.contains('\\')
                    {
                        // Remove trailing slash if present
                        let dir_name = line_before_comment
                            .strip_suffix('/')
                            .unwrap_or(line_before_comment);
                        // Ensure we don't insert empty strings if a line was just a comment or whitespace
                        if !dir_name.is_empty() {
                            ignored.insert(dir_name.to_string());
                        }
                    }
                }
            } else {
                eprintln!(
                    "Warning: Could not read .gitignore file at {}",
                    path.display()
                );
            }
        } else {
            // Check if the path components contain .git, otherwise don't warn (e.g. no .git found case)
            if path.components().any(|comp| comp.as_os_str() == ".git") {
                eprintln!(
                    "Warning: .gitignore path specified but not found or not a file: {}",
                    path.display()
                );
            }
        }
    }

    ignored
}

impl Cli {
    pub fn new(
        args: args::Args,
        base_dir: PathBuf,
        gitignore_path: Option<PathBuf>, // Pass potential .gitignore path
    ) -> Self {
        // Load extension styles from config file or use default
        let extension_styles = if let Some(config_path) = &args.config_file {
            match fs::read_to_string(config_path) {
                Ok(content) => {
                    println!("Loading config from {config_path}");
                    comments::parse_config(&content)
                }
                Err(e) => {
                    eprintln!("Error reading config file {config_path}: {e}");
                    println!("Using default configuration");
                    comments::default_config()
                }
            }
        } else {
            // Use default config
            comments::default_config()
        };

        // If extensions are specified in args, filter to only those
        let extension_styles = if let Some(extensions) = &args.extensions {
            let specified_extensions: Vec<String> = extensions
                .split(',')
                .map(|e| e.trim().to_lowercase())
                .collect();

            let mut filtered = HashMap::new();
            for ext in &specified_extensions {
                if let Some(&style) = extension_styles.get(ext) {
                    filtered.insert(ext.clone(), style);
                } else {
                    // Default to slash comment style if not found
                    eprintln!(
                        "Warning: Extension '.{}' specified but no configuration found, defaulting to '//' style.",
                        ext
                    );
                    filtered.insert(ext.clone(), comments::Style::Slash);
                }
            }
            filtered
        } else {
            extension_styles
        };

        // Load ignored directories (potentially merging .gitignore)
        let ignored_dirs = load_ignored_dirs(gitignore_path.as_deref());

        Self {
            args,
            base_dir,
            extension_styles,
            ignored_dirs, // Use loaded set
            processed_count: Arc::new(AtomicUsize::new(0)),
            skipped_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    #[cfg(test)]
    pub fn ignored_dirs(&self) -> &HashSet<String> {
        &self.ignored_dirs
    }

    pub fn new_arc(
        args: args::Args,
        base_dir: PathBuf,
        gitignore_path: Option<PathBuf>,
    ) -> Arc<Self> {
        Arc::new(Self::new(args, base_dir, gitignore_path))
    }

    pub fn should_process_file(&self, path: &Path) -> bool {
        if let Some(extension) = path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            self.extension_styles.contains_key(&ext)
        } else {
            false
        }
    }

    pub fn determine_comment_style(&self, path: &Path) -> Option<comments::Style> {
        // If user specified a style on command line, use that
        if let Some(style) = self.args.comment_style {
            return Some(style);
        }

        // Otherwise, look up in our extension map
        if let Some(extension) = path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            self.extension_styles.get(&ext).copied()
        } else {
            None
        }
    }

    pub fn should_skip_directory(&self, path: &Path) -> bool {
        if self.args.force {
            return false;
        }

        // Check the entire path for any component that matches our skip list
        for component in path.components() {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_string_lossy();
                if self.ignored_dirs.contains(name_str.as_ref()) {
                    return true;
                }
            }
        }
        false
    }

    pub fn process_file(&self, path: &Path) -> io::Result<()> {
        if !self.should_process_file(path) {
            // Don't increment skipped count here, it's not explicitly skipped due to config/state,
            // it just doesn't match the criteria. Let the caller handle skipping if needed.
            return Ok(());
        }

        // Determine the comment style for this file
        let comment_style = match self.determine_comment_style(path) {
            Some(style) => style,
            None => {
                // This case should ideally not be reached if should_process_file is true,
                // but handle defensively.
                eprintln!(
                    "Internal Error: Could not determine comment style for {} (but should_process_file was true). Skipping.",
                    path.display()
                );
                self.skipped_count.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
        };

        // Get the comment delimiters
        let (comment_start, comment_end) = match comments::DELIMITERS.get(&comment_style) {
            Some(delimiters) => delimiters,
            None => {
                // This should also ideally not be reached if determine_comment_style succeeded
                eprintln!(
                    "Internal Error: No delimiters found for comment style {:?}. Skipping {}.",
                    comment_style,
                    path.display()
                );
                self.skipped_count.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
        };

        let processed = format!("Processed {}", path.display()); // Renamed variable

        // Read the file content
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(ref e) if e.kind() == io::ErrorKind::InvalidData => {
                // Likely a binary file or non-UTF8 encoding
                // Use no_change style for visual consistency
                println!("{} {}", processed, no_change("Skipped non-UTF8 file"));
                self.skipped_count.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
            Err(e) => return Err(e), // Propagate other read errors
        };

        // Calculate the relative path
        let rel_path = match path.strip_prefix(&self.base_dir) {
            Ok(rel) => rel.to_path_buf(),
            // If stripping fails (e.g., path is not under base_dir), use the full path.
            // This might happen if base_dir logic changes or symlinks are involved.
            Err(_) => path.to_path_buf(),
        };
        // Convert to string, ensuring forward slashes for consistency
        let rel_path_str = rel_path.to_string_lossy().replace('\\', "/");
        let rel_path_str = rel_path_str.trim_start_matches("./").to_string();

        // Build the new header comment
        let first_line = format!("{comment_start}{rel_path_str}{comment_end}");

        // Split the content into lines for easier manipulation
        let lines: Vec<&str> = content.lines().collect();

        // First, check if the first line is exactly our desired comment
        let mut already_had_path_comment = false;
        if !lines.is_empty() && lines[0].trim() == first_line.trim() {
            already_had_path_comment = true;

            // If the correct comment is already there AND we are not stripping other potential
            // path comments, we can skip modification entirely.
            if !self.args.strip || !self.args.clean {
                println!("{processed} {}", no_change(&first_line));
                self.skipped_count.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
            // If strip is true, we still need to continue to check for *other* path comments.
        }

        // Get the regex for the current comment style
        let path_comment_re = match comments::REGEXES.get(&comment_style) {
            Some(regex) => regex,
            None => {
                // This should also ideally not be reached
                eprintln!(
                    "Internal Error: No regex found for comment style {:?}. Skipping {}.",
                    comment_style,
                    path.display()
                );
                self.skipped_count.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
        };

        // Find all existing path-looking comments *if* stripping is enabled
        let mut path_comment_line_numbers = Vec::new();
        if self.args.strip {
            for (line_num, line) in lines.iter().enumerate() {
                if line_num == 0 && already_had_path_comment {
                    continue;
                }
                // Use trim() to ignore leading/trailing whitespace when matching
                if path_comment_re.is_match(line.trim()) {
                    path_comment_line_numbers.push(line_num);
                }
            }
        }

        // --- Visualization ---

        let mut removed_lines_output = Vec::new();
        let mut added_lines_output = Vec::new();

        if path_comment_line_numbers.is_empty() {
            // First line is identical, show as no change
            if already_had_path_comment {
                println!("{processed} {}", no_change(&first_line));
            } else if self.args.clean {
                println!("{processed} {}", removed(&first_line));
            } else {
                println!("{processed} {}", added(&first_line));
            }
        } else {
            println!("{processed} ");
            if !self.args.clean {
                added_lines_output.push(added(&first_line));
                // } else if already_had_path_comment {
                // removed_lines_output.push(removed(&first_line));
            }
        }

        // Show other path comments being removed (if stripping)
        if self.args.strip {
            for &line_num in &path_comment_line_numbers {
                removed_lines_output.push(removed(lines[line_num]));
            }
        }

        // Print collected changes
        for line in removed_lines_output {
            println!("{}", line);
        }
        for line in added_lines_output {
            println!("{}", line);
        }
        // Only print the blank line if changes were actually visualized
        println!();

        // Build the final content lines vector
        // Start with the new first line we already added
        let mut final_content_lines: Vec<&str> = if !self.args.clean {
            vec![first_line.as_str()]
        } else {
            vec![]
        };

        // Add original lines, skipping the ones identified as path comments (if stripping)
        // Also skip the original line 0 if it was a path comment that we are replacing/stripping.
        for (i, line) in lines.iter().enumerate() {
            let is_path_comment_to_strip =
                self.args.strip && path_comment_line_numbers.contains(&i);

            if i == 0 {
                // We already added the new/correct first line.
                // Skip adding the original line 0 if:
                // 1. It was a path comment being stripped/replaced OR
                // 2. The first line wasn't changed (meaning the original line 0 was already correct)
                if is_path_comment_to_strip || already_had_path_comment {
                    continue;
                }
                // Otherwise (first line changed BUT original line 0 wasn't a path comment), add original line 0
            }

            // For lines other than 0, or if line 0 meets criteria above:
            // Add the line if it's not a path comment we're stripping
            if !is_path_comment_to_strip {
                final_content_lines.push(line);
            }
        }

        // Join the lines back together
        let mut new_content = final_content_lines.join("\n");

        // Preserve trailing newline if original had one or was empty
        if content.ends_with('\n') || content.is_empty() {
            // Ensure only one trailing newline
            if !new_content.ends_with('\n') {
                new_content.push('\n');
            }
        } else {
            // Original didn't end with newline, ensure new one doesn't either
            // (unless it's now empty, which join won't produce anyway)
            if new_content.ends_with('\n') && !new_content.is_empty() {
                new_content.pop();
            }
        }

        // Compare final content with original before writing
        if content == new_content {
            // This can happen if strip=true but the only path comment found was
            // already the correct first line. needs_write might have been true initially,
            // but the final result is identical.
            if already_had_path_comment {
                // If the first line was already correct...
                println!("{processed} {}", no_change(&first_line)); // Re-print no_change msg
            } // Otherwise the changes were already printed.
            self.skipped_count.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }

        if !self.args.dry_run {
            match fs::write(path, &new_content) {
                Ok(_) => {
                    self.processed_count.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    eprintln!("Error writing file {}: {}", path.display(), e);
                    // Treat as skipped if write fails
                    self.skipped_count.fetch_add(1, Ordering::Relaxed);
                    return Err(e); // Propagate write error
                }
            }
        } else {
            // In dry run mode we still count it as processed for stats because we *would* have written it
            self.processed_count.fetch_add(1, Ordering::Relaxed);
        }

        Ok(())
    }

    pub fn get_stats(&self) -> (usize, usize) {
        (
            self.processed_count.load(Ordering::Relaxed),
            self.skipped_count.load(Ordering::Relaxed),
        )
    }

    fn print_extension_styles(&self) {
        if self.extension_styles.is_empty() {
            println!("No file extensions configured.");
            return;
        }

        // Print the extensions that will be processed

        println!("File extensions that will be processed:");
        let mut extensions: Vec<(&String, &comments::Style)> =
            self.extension_styles.iter().collect();
        extensions.sort_by(|a, b| a.0.cmp(b.0)); // Sort by extension

        for (ext, style) in extensions {
            let (start, end) = style.delimiters();
            println!("  .{ext}: {start}{end}");
        }
        println!();
    }

    pub fn run(self: &Arc<Self>) {
        if self.args.print_extensions {
            self.print_extension_styles();
            return;
        }

        println!("Processing directory: {}", self.args.dir);
        println!("Using base directory: {}", self.base_dir.display());
        if self.args.dry_run {
            println!("Dry run mode enabled. No files will be modified.");
        }
        if self.args.force {
            println!("Force mode enabled. Ignoring default directory skip list.");
        }
        println!(); // Blank line for readability before processing starts

        // Build the walker
        let mut builder = WalkBuilder::new(&self.args.dir);
        builder.standard_filters(true); // Use .gitignore, .ignore etc by default
        if !self.args.recursive {
            builder.max_depth(Some(1));
        }

        let cli = self.clone();
        builder.filter_entry(move |entry: &DirEntry| -> bool {
            if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                // Use the cloned Arc inside the closure
                let should_skip = cli.should_skip_directory(entry.path());
                if should_skip {
                    // println!("Skipping directory due to config: {}", entry.path().display()); // Optional debug noise
                }
                !should_skip // Keep directory if it's NOT skipped by our custom logic
            } else {
                true // Always keep files initially, standard filters and process_file will handle later
            }
        });

        // Process files in parallel
        builder.build_parallel().run(|| {
            let cli = self.clone(); // Clone Arc for the worker closure
            Box::new(move |result| {
                match result {
                    Ok(entry) => {
                        // Check if it's a file *after* filtering (standard filters might remove files)
                        if entry.file_type().is_some_and(|ft| ft.is_file()) {
                            if cli.should_process_file(entry.path()) {
                                // Process the file if the extension matches
                                if let Err(err) = cli.process_file(entry.path()) {
                                    eprintln!("Error processing {}: {err}", entry.path().display());
                                    // Note: process_file increments skipped_count on specific internal errors/skips
                                }
                            } else {
                                // File doesn't match our extension list, count as skipped for summary
                                cli.skipped_count.fetch_add(1, Ordering::Relaxed);
                            }
                        } // Ignore directories and other types here
                        WalkState::Continue
                    }
                    Err(err) => {
                        eprintln!("Error walking directory: {err}");
                        // Potentially skip this entry or stop the walk? Continuing for now.
                        WalkState::Continue
                    }
                }
            })
        });

        println!("\nSummary:");
        let (processed, skipped) = self.get_stats();
        println!("  Files processed: {processed}");
        println!("  Files skipped: {skipped}");

        if self.args.dry_run {
            println!("\nThis was a dry run. No files were modified.");
        }
    }
}
