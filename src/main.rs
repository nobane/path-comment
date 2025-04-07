use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use clap::{Parser, ValueEnum};
use ignore::{WalkBuilder, WalkState};
use regex::Regex;

#[cfg(test)]
mod test;

/// CLI tool to prepend file paths as comments to source code files
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory to process files in
    #[arg(required = true)]
    dir: String,

    /// Base directory for calculating relative paths
    #[arg(short, long)]
    base: Option<String>,

    /// File extensions to process (comma-separated)
    #[arg(short, long)]
    extensions: Option<String>,

    /// Process files recursively
    #[arg(short, long, default_value_t = true)]
    recursive: bool,

    /// Dry run (don't modify files, just print what would be done)
    #[arg(short, long)]
    dry_run: bool,

    /// Comment style to use
    #[arg(short, long, value_enum, default_value_t = CommentStyle::Auto)]
    comment_style: CommentStyle,

    /// Keep existing path comments
    #[arg(short, long)]
    keep_existing: bool,

    /// Force processing of dependency directories (node_modules, venv, etc.)
    #[arg(short, long)]
    force: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum CommentStyle {
    Auto,
    Slash,     // //
    SlashStar, // /* */
    Hash,      // #
    Semi,      // ;
    Html,      // <!-- -->
    Percent,   // %
}

struct FileProcessor {
    base_dir: PathBuf,
    comment_style: CommentStyle,
    dry_run: bool,
    no_update: bool,
    extensions: Option<Vec<String>>,
    force: bool,
    processed_count: Arc<Mutex<usize>>,
    skipped_count: Arc<Mutex<usize>>,
}

enum AnsiColor {
    Red,
    Green,
    Yellow,
}

impl AnsiColor {
    fn code(&self) -> &'static str {
        match self {
            AnsiColor::Red => "\x1b[31m",
            AnsiColor::Green => "\x1b[32m",
            AnsiColor::Yellow => "\x1b[33m",
        }
    }
}

const ANSI_RESET: &str = "\x1b[0m";

fn colorize(text: &str, color: AnsiColor) -> String {
    format!("{}{}{}", color.code(), text, ANSI_RESET)
}

impl FileProcessor {
    fn new(
        base_dir: PathBuf,
        comment_style: CommentStyle,
        dry_run: bool,
        no_update: bool,
        extensions: Option<String>,
        force: bool,
    ) -> Self {
        let extensions = extensions.map(|ext| {
            ext.split(',')
                .map(|e| e.trim().to_string().to_lowercase())
                .collect()
        });

        Self {
            base_dir,
            comment_style,
            dry_run,
            no_update,
            extensions,
            force,
            processed_count: Arc::new(Mutex::new(0)),
            skipped_count: Arc::new(Mutex::new(0)),
        }
    }

    fn should_process_file(&self, path: &Path) -> bool {
        if let Some(extension) = path.extension() {
            if let Some(extensions) = &self.extensions {
                extensions.contains(&extension.to_string_lossy().to_lowercase())
            } else {
                // By default, process common source code file types
                matches!(
                    extension.to_string_lossy().to_lowercase().as_str(),
                    "rs" | "py"
                        | "js"
                        | "jsx"
                        | "ts"
                        | "tsx"
                        | "java"
                        | "c"
                        | "cpp"
                        | "h"
                        | "hpp"
                        | "cs"
                        | "php"
                        | "rb"
                        | "go"
                        | "swift"
                        | "kt"
                        | "scala"
                        | "sh"
                        | "pl"
                        | "lua"
                        | "sql"
                        | "html"
                        | "css"
                        | "scss"
                        | "sass"
                        | "less"
                        | "xml"
                        | "json"
                        | "yaml"
                        | "yml"
                        | "md"
                        | "markdown"
                        | "r"
                        | "m"
                        | "mm"
                        | "ex"
                        | "exs"
                        | "erl"
                        | "fs"
                        | "fsx"
                        | "hs"
                        | "dart"
                )
            }
        } else {
            false
        }
    }

    fn determine_comment_style(&self, path: &Path) -> Option<(String, String, String)> {
        // If user specified a style, use that
        if self.comment_style != CommentStyle::Auto {
            return match self.comment_style {
                CommentStyle::Slash => Some(("// ".to_string(), "".to_string(), "".to_string())),
                CommentStyle::SlashStar => {
                    Some(("/* ".to_string(), " */".to_string(), " * ".to_string()))
                }
                CommentStyle::Hash => Some(("# ".to_string(), "".to_string(), "".to_string())),
                CommentStyle::Semi => Some(("; ".to_string(), "".to_string(), "".to_string())),
                CommentStyle::Html => {
                    Some(("<!-- ".to_string(), " -->".to_string(), "".to_string()))
                }
                CommentStyle::Percent => Some(("% ".to_string(), "".to_string(), "".to_string())),
                CommentStyle::Auto => unreachable!(),
            };
        }

        // Otherwise, determine by extension
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match extension.as_deref() {
            // C-style comments (// or /* */)
            Some("rs") | Some("js") | Some("jsx") | Some("ts") | Some("tsx") | Some("java")
            | Some("c") | Some("cpp") | Some("h") | Some("hpp") | Some("cs") | Some("go")
            | Some("swift") | Some("kt") | Some("scala") | Some("php") | Some("css")
            | Some("scss") | Some("sass") | Some("less") | Some("dart") => {
                Some(("// ".to_string(), "".to_string(), "".to_string()))
            }

            // Hash comments (#)
            Some("py") | Some("rb") | Some("pl") | Some("sh") | Some("r") | Some("yaml")
            | Some("yml") | Some("ex") | Some("exs") => {
                Some(("# ".to_string(), "".to_string(), "".to_string()))
            }

            // Semi comments (;)
            Some("lisp") | Some("clj") | Some("edn") => {
                Some(("; ".to_string(), "".to_string(), "".to_string()))
            }

            // HTML/XML comments (<!-- -->)
            Some("html") | Some("xml") | Some("md") | Some("markdown") => {
                Some(("<!-- ".to_string(), " -->".to_string(), "".to_string()))
            }

            // Percent comments (%)
            Some("tex") | Some("m") => Some(("% ".to_string(), "".to_string(), "".to_string())),

            // Multi-line style for special cases
            Some("sql") => Some(("-- ".to_string(), "".to_string(), "".to_string())),

            _ => panic!("unhandled extension {}", extension.as_deref()), // Some(("// ".to_string(), "".to_string(), "".to_string())),
        }
    }
    fn should_skip_directory(&self, path: &Path) -> bool {
        if self.force == true {
            return false;
        }

        // Check the entire path for any component that matches our skip list
        for component in path.components() {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_string_lossy().to_lowercase();
                if matches!(
                    name_str.as_str(),
                    "node_modules"
                        | "venv"
                        | ".venv"
                        | ".env"
                        | "dist"
                        | "build"
                        | "target"
                        | ".git"
                        | "__pycache__"
                        | "bin"
                        | "obj"
                        | "pkg"
                ) {
                    return true;
                }
            }
        }

        false
    }
    fn process_file(&self, path: &Path) -> io::Result<()> {
        if !self.should_process_file(path) {
            *self.skipped_count.lock().unwrap() += 1;
            return Ok(());
        }

        let comment_style = match self.determine_comment_style(path) {
            Some(style) => style,
            None => {
                *self.skipped_count.lock().unwrap() += 1;
                return Ok(());
            }
        };

        // Read the file content
        let mut content = String::new();
        let mut file = fs::File::open(path)?;
        file.read_to_string(&mut content)?;

        // Calculate the relative path
        let rel_path = match path.strip_prefix(&self.base_dir) {
            Ok(rel) => rel.to_string_lossy().to_string(),
            Err(_) => path.to_string_lossy().to_string(),
        };

        // Build the new header comment
        let (comment_start, comment_end, _) = comment_style;
        let comment_line = format!("{}{}{}", comment_start, rel_path, comment_end);

        // Split the content into lines for easier manipulation
        let lines: Vec<&str> = content.lines().collect();

        // First, check if the first line is exactly our desired comment
        if !lines.is_empty() && lines[0].trim() == comment_line.trim() {
            println!(
                "Processed {} {}",
                path.display(),
                colorize(&comment_line, AnsiColor::Yellow)
            );
            *self.skipped_count.lock().unwrap() += 1;
            return Ok(());
        }

        // Get the filename for matching
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Define regex pattern to find path comments
        let exact_path_pattern = format!(
            r"^({})([^{{\s][^\s]*?{}(?:\.[a-zA-Z0-9]+)?)({})$",
            regex::escape(&comment_start),
            regex::escape(&file_name),
            regex::escape(&comment_end)
        );

        let exact_path_re = Regex::new(&exact_path_pattern).unwrap();

        // Find all path-looking comments
        let mut path_comment_lines = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if exact_path_re.is_match(line) {
                path_comment_lines.push((i, *line));
            }
        }

        // Create the new content
        let mut new_lines = Vec::new();

        // Add the new comment line at the beginning
        new_lines.push(comment_line.as_str());

        let new_first_line = colorize(&format!("+ {}", comment_line), AnsiColor::Green);

        let processed = path.display();
        if !self.no_update && !path_comment_lines.is_empty() {
            // Display removals in diff
            if path_comment_lines.len() == 1 && lines.len() <= 2 {
                let old_first_line =
                    colorize(&format!("- {}", path_comment_lines[0].1), AnsiColor::Red);

                println!("Processed {processed} {old_first_line} {new_first_line}");
            } else {
                println!("Processed {processed}");

                for (_, line) in &path_comment_lines {
                    let old_first_line = colorize(&format!("- {line}"), AnsiColor::Red);
                    println!("{old_first_line}")
                }

                // Show addition in diff
                println!("{new_first_line}\n");
            }
            // Add the non-path-comment lines
            for (i, line) in lines.iter().enumerate() {
                if !path_comment_lines.iter().any(|(idx, _)| *idx == i) {
                    new_lines.push(line);
                }
            }
        } else {
            // Add all original lines
            new_lines.extend_from_slice(&lines);

            // For the case where we're adding a comment but not removing any
            println!("Processed {processed} {new_first_line}");
        }

        // Join the lines back together
        let new_content = new_lines.join("\n");

        // Add trailing newline in these cases:
        // 1. The original content had a trailing newline, or
        // 2. The file was empty (lines.is_empty())
        let new_content = if content.ends_with('\n') || lines.is_empty() {
            format!("{}\n", new_content)
        } else {
            new_content
        };

        if !self.dry_run && content != new_content {
            fs::write(path, new_content)?;
            *self.processed_count.lock().unwrap() += 1;
        } else if self.dry_run {
            // In dry run mode we still count it as processed for stats
            *self.processed_count.lock().unwrap() += 1;
        } else {
            // File didn't change
            *self.skipped_count.lock().unwrap() += 1;
        }

        Ok(())
    }
    fn get_stats(&self) -> (usize, usize) {
        (
            *self.processed_count.lock().unwrap(),
            *self.skipped_count.lock().unwrap(),
        )
    }
}
fn main() {
    let args = Args::parse();

    // Determine the base directory for relative paths
    let base_dir = if let Some(base) = args.base {
        PathBuf::from(base)
    } else {
        std::env::current_dir().expect("Failed to get current directory")
    };

    // Create the file processor
    let processor = Arc::new(FileProcessor::new(
        base_dir,
        args.comment_style,
        args.dry_run,
        args.keep_existing,
        args.extensions,
        args.force,
    ));

    // Build the walker
    let mut builder = WalkBuilder::new(&args.dir);
    if !args.recursive {
        builder.max_depth(Some(1));
    }

    // Add custom filter for dependency directories
    let processor_clone = Arc::clone(&processor);
    builder.filter_entry(move |entry| {
        // Don't filter files, only directories
        if entry.file_type().map_or(false, |ft| ft.is_dir()) {
            !processor_clone.should_skip_directory(entry.path())
        } else {
            true
        }
    });

    // Process files in parallel
    builder.build_parallel().run(|| {
        let processor = Arc::clone(&processor);
        Box::new(move |result| {
            match result {
                Ok(entry) => {
                    // Only process files, not directories
                    if entry
                        .file_type()
                        .as_ref()
                        .is_some_and(std::fs::FileType::is_file)
                    {
                        if let Err(err) = processor.process_file(entry.path()) {
                            eprintln!("Error processing {}: {}", entry.path().display(), err);
                        }
                    }
                    WalkState::Continue
                }
                Err(err) => {
                    eprintln!("Error: {}", err);
                    WalkState::Continue
                }
            }
        })
    });

    // Print final stats
    let (processed, skipped) = processor.get_stats();
    println!("\nSummary:");
    println!("  Files processed: {}", processed);
    println!("  Files skipped: {}", skipped);

    if args.dry_run {
        println!("\nThis was a dry run. No files were modified.");
    }
}
