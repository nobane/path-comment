// src/test.rs
use super::*;
use std::fs::{self, File, create_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use crate::args::Args;
use crate::cli::Cli; // Keep specific import for Cli
use crate::comments::Style as CommentStyle;

struct TestArgsBuilder {
    args: Args,
    // Keep track of temp dir path for convenience in tests
    temp_dir_path: PathBuf,
}

impl TestArgsBuilder {
    // Modify new to accept temp_dir path directly
    fn new(temp_dir: &TempDir) -> Self {
        let path = temp_dir.path().to_path_buf();
        Self {
            args: Args {
                dir: path.to_string_lossy().to_string(), // Default dir to temp dir
                base: None,
                no_git_base: false, // Default to allowing git search
                extensions: None,
                config_file: None,
                recursive: true,
                dry_run: false,
                comment_style: None,
                force: false,
                strip: true,
                print_extensions: false,
                no_ignore_merge: false, // Default to allowing merge
                clean: false,
            },
            temp_dir_path: path,
        }
    }

    // Helper to set the processing dir relative to temp_dir
    fn dir(mut self, relative_dir: &str) -> Self {
        self.args.dir = self
            .temp_dir_path
            .join(relative_dir)
            .to_string_lossy()
            .to_string();
        self
    }

    fn base(mut self, base: &str) -> Self {
        // Allow relative base paths for testing flexibility
        self.args.base = Some(base.to_string());
        self
    }

    fn no_git(mut self, no_git: bool) -> Self {
        self.args.no_git_base = no_git;
        self
    }

    fn no_ignore_merge(mut self, no_merge: bool) -> Self {
        self.args.no_ignore_merge = no_merge;
        self
    }

    fn extensions(mut self, extensions: &str) -> Self {
        self.args.extensions = Some(extensions.to_string());
        self
    }

    // TODO: Test this!
    #[allow(unused)]
    fn config_file(mut self, config_file: &str) -> Self {
        // Assume config file is relative to temp dir root for tests
        self.args.config_file = Some(
            self.temp_dir_path
                .join(config_file)
                .to_string_lossy()
                .to_string(),
        );
        self
    }

    // TODO: Test this!
    #[allow(unused)]
    fn recursive(mut self, recursive: bool) -> Self {
        self.args.recursive = recursive;
        self
    }

    fn dry_run(mut self, dry_run: bool) -> Self {
        self.args.dry_run = dry_run;
        self
    }

    fn comment_style(mut self, style: CommentStyle) -> Self {
        self.args.comment_style = Some(style);
        self
    }

    fn force(mut self, force: bool) -> Self {
        self.args.force = force;
        self
    }

    fn strip(mut self, strip: bool) -> Self {
        self.args.strip = strip;
        self
    }

    fn build(self) -> (Args, PathBuf) {
        // Return both Args and the temp_dir path for use in tests
        (self.args, self.temp_dir_path)
    }
}

// Helper to create files, ensuring parent dirs exist
fn create_test_file(base_dir: &Path, relative_name: &str, content: &str) -> PathBuf {
    let path = base_dir.join(relative_name);
    if let Some(parent) = path.parent() {
        create_dir_all(parent).unwrap();
    }
    let mut file = File::create(&path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    path
}

// Helper to get base_dir and gitignore_path based on Args and test setup
// Simulates the logic in main.rs
fn determine_test_paths(args: &Args, temp_root: &Path) -> (PathBuf, Option<PathBuf>) {
    let mut git_base_used = false;
    let start_dir = Path::new(&args.dir)
        .canonicalize()
        .unwrap_or_else(|_| panic!("Test dir {} not found", args.dir));

    let base_dir = match args.base {
        Some(ref base) => temp_root
            .join(base)
            .canonicalize()
            .unwrap_or_else(|_| panic!("Test base {} not found", base)),
        None => {
            if args.no_git_base {
                temp_root.canonicalize().unwrap() // Simulate CWD fallback to temp_root
            } else if let Some(git_root) = find_git_root(&start_dir) {
                git_base_used = true;
                git_root
            } else {
                temp_root.canonicalize().unwrap() // Simulate CWD fallback
            }
        }
    };

    let gitignore_path = if git_base_used && !args.no_ignore_merge {
        Some(base_dir.join(".gitignore"))
    } else {
        None
    };

    (base_dir, gitignore_path)
}

// --- Basic Functionality Tests (Adapted) ---

#[test]
fn test_should_process_file() {
    let temp_dir = TempDir::new().unwrap();
    let (args, temp_path) = TestArgsBuilder::new(&temp_dir).build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path);
    let processor = Cli::new(args, base_dir, gitignore_path);

    assert!(processor.should_process_file(Path::new("test.rs")));
    assert!(processor.should_process_file(Path::new("test.py")));
    assert!(processor.should_process_file(Path::new("test.js")));
    assert!(processor.should_process_file(Path::new("test.tsx")));
    assert!(!processor.should_process_file(Path::new("test.xyz")));
    assert!(!processor.should_process_file(Path::new("test"))); // No extension
}

#[test]
fn test_extensions_filter() {
    let temp_dir = TempDir::new().unwrap();
    let (args, temp_path) = TestArgsBuilder::new(&temp_dir).extensions("rs,py").build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path);
    let processor = Cli::new(args, base_dir, gitignore_path);

    assert!(processor.should_process_file(Path::new("test.rs")));
    assert!(processor.should_process_file(Path::new("test.py")));
    assert!(!processor.should_process_file(Path::new("test.js")));
    assert!(!processor.should_process_file(Path::new("test.tsx")));
}

#[test]
fn test_determine_comment_style() {
    let temp_dir = TempDir::new().unwrap();
    let (args, temp_path) = TestArgsBuilder::new(&temp_dir).build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path);
    let processor = Cli::new(args, base_dir, gitignore_path);

    let style_rs = processor
        .determine_comment_style(Path::new("test.rs"))
        .unwrap();
    assert_eq!(style_rs.delimiters(), ("// ", ""));
    let style_py = processor
        .determine_comment_style(Path::new("test.py"))
        .unwrap();
    assert_eq!(style_py.delimiters(), ("# ", ""));
    let style_html = processor
        .determine_comment_style(Path::new("test.html"))
        .unwrap();
    assert_eq!(style_html.delimiters(), ("<!-- ", " -->"));
}

#[test]
fn test_explicit_comment_style() {
    let temp_dir = TempDir::new().unwrap();
    let (args, temp_path) = TestArgsBuilder::new(&temp_dir)
        .comment_style(CommentStyle::Hash)
        .build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path);
    let processor = Cli::new(args, base_dir, gitignore_path);

    let style = processor
        .determine_comment_style(Path::new("test.rs"))
        .unwrap();
    assert_eq!(style.delimiters(), ("# ", "")); // Overrides default for .rs
}

#[test]
fn test_process_file_new() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = create_test_file(temp_dir.path(), "test.js", "content();\n");
    let (args, temp_path) = TestArgsBuilder::new(&temp_dir).build(); // Base defaults to temp_path
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path);
    let processor = Cli::new(args, base_dir, gitignore_path);

    processor.process_file(&test_file).unwrap();
    let new_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!("// test.js\ncontent();\n", new_content);
}

#[test]
fn test_process_file_update() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = create_test_file(
        temp_dir.path(),
        "test.js",
        "// old/path/test.js\ncontent();\n",
    );
    let (args, temp_path) = TestArgsBuilder::new(&temp_dir).build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path);
    let processor = Cli::new(args, base_dir, gitignore_path);

    processor.process_file(&test_file).unwrap();
    let new_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!("// test.js\ncontent();\n", new_content);
}

#[test]
fn test_process_file_dry_run() {
    let temp_dir = TempDir::new().unwrap();
    let original_content = "content();\n";
    let test_file = create_test_file(temp_dir.path(), "test.js", original_content);
    let (args, temp_path) = TestArgsBuilder::new(&temp_dir).dry_run(true).build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path);
    let processor = Cli::new(args, base_dir, gitignore_path);

    processor.process_file(&test_file).unwrap();
    let new_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(original_content, new_content); // Content should not change
}

#[test]
fn test_strip_disabled() {
    let temp_dir = TempDir::new().unwrap();
    let original_content = "// old/path/test.js\ncontent();\n";
    let test_file = create_test_file(temp_dir.path(), "test.js", original_content);
    let (args, temp_path) = TestArgsBuilder::new(&temp_dir).strip(false).build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path);
    // Expect base_dir to be temp_path itself
    let processor = Cli::new(args, base_dir, gitignore_path);

    processor.process_file(&test_file).unwrap();
    let new_content = fs::read_to_string(&test_file).unwrap();
    // Should add the new comment but KEEP the old one because strip is false
    let expected = "// test.js\n// old/path/test.js\ncontent();\n";
    assert_eq!(expected, new_content);
}

#[test]
fn test_empty_file_comment_addition() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = create_test_file(temp_dir.path(), "App.tsx", "");
    let (args, temp_path) = TestArgsBuilder::new(&temp_dir).build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path);
    let processor = Cli::new(args, base_dir, gitignore_path);

    processor.process_file(&test_file).unwrap();
    let new_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!("// App.tsx\n", new_content); // Ensure trailing newline
}

// --- New Tests for Base Directory and Ignore Logic ---

#[test]
fn test_base_dir_explicit() {
    let temp_dir = TempDir::new().unwrap();
    let sub_dir = temp_dir.path().join("project");
    create_dir_all(&sub_dir).unwrap();
    let test_file = create_test_file(&sub_dir, "src/main.rs", "fn main(){}\n");

    // Explicitly set base to "project" relative to temp_dir
    let (args, temp_path) = TestArgsBuilder::new(&temp_dir).base("project").build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path); // Should resolve to project dir
    let processor = Cli::new(args, base_dir, gitignore_path);

    processor.process_file(&test_file).unwrap();
    let new_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!("// src/main.rs\nfn main(){}\n", new_content); // Path relative to "project"
}

#[test]
fn test_base_dir_git_detect() {
    let temp_dir = TempDir::new().unwrap();
    let git_root = temp_dir.path().join("my_repo");
    create_dir_all(&git_root).unwrap();
    create_dir_all(git_root.join(".git")).unwrap(); // Simulate .git dir
    let project_src = git_root.join("code/lib");
    create_dir_all(&project_src).unwrap();
    let test_file = create_test_file(&project_src, "util.py", "def helper(): pass\n");

    // Process starting inside the repo, NO explicit --base
    let (args, _temp_path) = TestArgsBuilder::new(&temp_dir)
        .dir(git_root.join("code").to_str().unwrap())
        .build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, temp_dir.path()); // Should detect git_root

    // Check base_dir was detected correctly
    assert_eq!(base_dir, git_root.canonicalize().unwrap());

    let processor = Cli::new(args, base_dir, gitignore_path);
    processor.process_file(&test_file).unwrap();

    let new_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!("# code/lib/util.py\ndef helper(): pass\n", new_content); // Path relative to git_root
}

#[test]
fn test_base_dir_no_git_flag() {
    let temp_dir = TempDir::new().unwrap();
    let git_root = temp_dir.path().join("my_repo");
    create_dir_all(&git_root).unwrap();
    create_dir_all(git_root.join(".git")).unwrap();
    let test_file = create_test_file(&git_root, "main.rs", "fn main(){}\n");

    // Process repo root, but disable git detection
    let (args, temp_path) = TestArgsBuilder::new(&temp_dir)
        .dir(git_root.to_str().unwrap())
        .no_git(true)
        .build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path); // Should fall back to temp_path

    // Check base_dir fell back correctly (simulated CWD = temp_path)
    assert_eq!(base_dir, temp_path.canonicalize().unwrap());

    let processor = Cli::new(args, base_dir, gitignore_path);
    processor.process_file(&test_file).unwrap();

    let new_content = fs::read_to_string(&test_file).unwrap();
    // Path should be relative to temp_dir (simulated CWD), not the git_root
    assert_eq!("// my_repo/main.rs\nfn main(){}\n", new_content);
}

#[test]
fn test_base_dir_no_git_found_fallback() {
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path().join("my_project");
    create_dir_all(&project_dir).unwrap();
    let test_file = create_test_file(&project_dir, "app.js", "console.log('hi');\n");

    // Process project dir, no .git anywhere
    let (args, temp_path) = TestArgsBuilder::new(&temp_dir)
        .dir(project_dir.to_str().unwrap())
        .build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path); // Should fall back to temp_path

    assert_eq!(base_dir, temp_path.canonicalize().unwrap()); // Check fallback

    let processor = Cli::new(args, base_dir, gitignore_path);
    processor.process_file(&test_file).unwrap();

    let new_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!("// my_project/app.js\nconsole.log('hi');\n", new_content); // Relative to temp_path
}

#[test]
fn test_default_ignore_loaded() {
    let temp_dir = TempDir::new().unwrap();
    let (args, temp_path) = TestArgsBuilder::new(&temp_dir).build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path);
    let processor = Cli::new(args, base_dir, gitignore_path);

    // Check some defaults loaded from ignore.cfg (embedded via include_str!)
    assert!(processor.ignored_dirs().contains("node_modules"));
    assert!(processor.ignored_dirs().contains("target"));
    assert!(processor.ignored_dirs().contains(".git"));
    assert!(!processor.ignored_dirs().contains("src")); // Should not be ignored by default
}

#[test]
fn test_gitignore_merge() {
    let temp_dir = TempDir::new().unwrap();
    let git_root = temp_dir.path().join("my_repo");
    create_dir_all(&git_root).unwrap();
    create_dir_all(git_root.join(".git")).unwrap(); // Needs .git to trigger merge logic

    // Create a .gitignore file
    let gitignore_content = r#"
 # Comment line
 build/
 /dist
 *.log
 vendor # Simple name
     "#;
    create_test_file(&git_root, ".gitignore", gitignore_content);

    // Process starting inside the repo
    let (args, _temp_path) = TestArgsBuilder::new(&temp_dir)
        .dir(git_root.to_str().unwrap())
        .build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, temp_dir.path()); // Should detect git_root and find .gitignore

    assert_eq!(base_dir, git_root.canonicalize().unwrap());
    assert!(gitignore_path.is_some()); // Ensure gitignore path was determined

    let processor = Cli::new(args, base_dir, gitignore_path);

    // Check defaults are still there
    assert!(processor.ignored_dirs().contains("node_modules"));
    // Check simple merges from .gitignore
    assert!(processor.ignored_dirs().contains("build")); // Trailing / removed
    assert!(processor.ignored_dirs().contains("dist")); // Leading / removed (simplistic parsing)
    assert!(processor.ignored_dirs().contains("vendor"));
    // Check complex pattern was NOT added by simple parsing
    assert!(!processor.ignored_dirs().contains("*.log"));
}

#[test]
fn test_gitignore_merge_disabled() {
    let temp_dir = TempDir::new().unwrap();
    let git_root = temp_dir.path().join("my_repo");
    create_dir_all(&git_root).unwrap();
    create_dir_all(git_root.join(".git")).unwrap();
    create_test_file(&git_root, ".gitignore", "build/\nvendor\n");

    // Disable merging
    let (args, _temp_path) = TestArgsBuilder::new(&temp_dir)
        .dir(git_root.to_str().unwrap())
        .no_ignore_merge(true) // Disable merge
        .build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, temp_dir.path()); // Should still detect git_root but NOT pass gitignore path

    assert_eq!(base_dir, git_root.canonicalize().unwrap());
    assert!(gitignore_path.is_none()); // Ensure gitignore path is None due to flag

    let processor = Cli::new(args, base_dir, gitignore_path); // Pass None for gitignore

    // Check defaults are there
    assert!(processor.ignored_dirs().contains("node_modules"));
    assert!(processor.ignored_dirs().contains("build")); // Default ignore should still be present
    // Check ignores from .gitignore were NOT merged
    assert!(!processor.ignored_dirs().contains("vendor")); // THIS is the correct check
}

#[test]
fn test_directory_skipping_default() {
    let temp_dir = TempDir::new().unwrap();
    let node_modules = temp_dir.path().join("node_modules/mypackage");
    create_dir_all(&node_modules).unwrap();
    let test_file = create_test_file(&node_modules, "index.js", "module.exports = {};\n");

    let (args, temp_path) = TestArgsBuilder::new(&temp_dir).build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path);
    let processor = Cli::new(args.clone(), base_dir.clone(), gitignore_path.clone());

    // should_skip_directory checks if any component matches ignored set
    assert!(processor.should_skip_directory(&node_modules));
    assert!(processor.should_skip_directory(&test_file)); // Also true for file inside ignored dir

    let cli_arc = cli::Cli::new_arc(args, base_dir, gitignore_path);
    cli_arc.run();
    let (processed, skipped) = cli_arc.get_stats();
    assert_eq!(processed, 0); // File should be skipped by walker filter
    //  Assert skipped is 0 because the filter prevents reaching the counting logic
    assert_eq!(skipped, 0);
}

#[test]
fn test_directory_skipping_gitignore_merged() {
    let temp_dir = TempDir::new().unwrap();
    let git_root = temp_dir.path().join("my_repo");
    create_dir_all(&git_root).unwrap();
    create_dir_all(git_root.join(".git")).unwrap();
    create_test_file(&git_root, ".gitignore", "vendor/\n"); // Ignore 'vendor'

    let vendor_dir = git_root.join("vendor/somelib");
    create_dir_all(&vendor_dir).unwrap();
    let test_file = create_test_file(&vendor_dir, "lib.js", "// Lib code\n");

    let (args, _temp_path) = TestArgsBuilder::new(&temp_dir)
        .dir(git_root.to_str().unwrap())
        .build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, temp_dir.path());
    let processor = Cli::new(args.clone(), base_dir.clone(), gitignore_path.clone());

    assert!(processor.ignored_dirs().contains("vendor")); // Check merge happened
    assert!(processor.should_skip_directory(&vendor_dir));
    assert!(processor.should_skip_directory(&test_file));

    let cli_arc = cli::Cli::new_arc(args, base_dir, gitignore_path);
    cli_arc.run();
    let (processed, skipped) = cli_arc.get_stats();
    assert_eq!(processed, 0); // File should be skipped by walker filter
    // Assert skipped is 0 because the filter prevents reaching the counting logic
    assert_eq!(skipped, 0);
}

#[test]
fn test_directory_skipping_force() {
    let temp_dir = TempDir::new().unwrap();
    let node_modules = temp_dir.path().join("node_modules/mypackage");
    create_dir_all(&node_modules).unwrap();
    let test_file = create_test_file(&node_modules, "index.js", "module.exports = {};\n");

    let (args, temp_path) = TestArgsBuilder::new(&temp_dir).force(true).build(); // Enable force
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path);
    let processor = Cli::new(args.clone(), base_dir.clone(), gitignore_path.clone());

    // Force flag overrides skipping logic
    assert!(!processor.should_skip_directory(&node_modules));
    assert!(!processor.should_skip_directory(&test_file));

    let cli_arc = cli::Cli::new_arc(args, base_dir, gitignore_path);
    cli_arc.run();
    let (processed, skipped) = cli_arc.get_stats();
    assert_eq!(processed, 1); // File should be processed now
    assert_eq!(skipped, 0);
}

// Re-include tests that might have been implicitly removed or need slight adaptation
#[test]
fn test_relative_path_calculation_standard() {
    let temp_dir = TempDir::new().unwrap();
    let sub_dir = temp_dir.path().join("src");
    create_dir_all(&sub_dir).unwrap();
    let test_file = create_test_file(&sub_dir, "test.js", "content();\n");

    // Base dir is temp_dir (default behavior when no .git and no --base)
    let (args, temp_path) = TestArgsBuilder::new(&temp_dir)
        .dir(sub_dir.to_str().unwrap())
        .build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path);
    assert_eq!(base_dir, temp_path.canonicalize().unwrap()); // Ensure base is temp path

    let processor = Cli::new(args, base_dir, gitignore_path);
    processor.process_file(&test_file).unwrap();

    let new_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!("// src/test.js\ncontent();\n", new_content); // Relative to temp_path
}

#[test]
fn test_exact_path_comment_matching() {
    let temp_dir = TempDir::new().unwrap();
    let (args, temp_path) = TestArgsBuilder::new(&temp_dir).build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path);
    let processor = Cli::new(args, base_dir, gitignore_path);

    // Existing comment matches exactly what we would add (relative to base_dir = temp_path)
    let content_correct = "// test1.js\ncontent();\n";
    let file_correct = create_test_file(temp_dir.path(), "test1.js", content_correct);
    processor.process_file(&file_correct).unwrap();
    assert_eq!(content_correct, fs::read_to_string(&file_correct).unwrap());
    let (_, skipped) = processor.get_stats();
    assert!(skipped > 0); // Should be skipped as no change needed

    // Existing comment is a different path
    let content_wrong_path = "// src/test2.js\ncontent();\n";
    let file_wrong_path = create_test_file(temp_dir.path(), "test2.js", content_wrong_path);
    processor.process_file(&file_wrong_path).unwrap();
    assert_eq!(
        "// test2.js\ncontent();\n",
        fs::read_to_string(&file_wrong_path).unwrap()
    );

    // Existing comment looks like a path but has extra stuff
    let content_extra = "// test3.js - My note\ncontent();\n";
    let file_extra = create_test_file(temp_dir.path(), "test3.js", content_extra);
    processor.process_file(&file_extra).unwrap();
    // With strip=true (default), the non-matching path comment is removed
    assert_eq!(
        "// test3.js\n// test3.js - My note\ncontent();\n",
        fs::read_to_string(&file_extra).unwrap()
    );

    // Existing comment looks like a path but uses different slashes (should still match regex)
    let content_slashes = "// path\\to\\test4.js\ncontent();\n";
    let file_slashes = create_test_file(temp_dir.path(), "test4.js", content_slashes);
    processor.process_file(&file_slashes).unwrap();
    // Replaced with correct path using forward slashes
    assert_eq!(
        "// test4.js\ncontent();\n",
        fs::read_to_string(&file_slashes).unwrap()
    );
}

#[test]
fn test_multiple_path_comments() {
    let temp_dir = TempDir::new().unwrap();
    let content = "// src/App.tsx\n// /opt/App.tsx\nimport React from 'react';\n// Another path comment: ./component.tsx\n";
    let test_file = create_test_file(temp_dir.path(), "App.tsx", content);
    let (args, temp_path) = TestArgsBuilder::new(&temp_dir).build();
    let (base_dir, gitignore_path) = determine_test_paths(&args, &temp_path);
    let processor = Cli::new(args, base_dir, gitignore_path);

    processor.process_file(&test_file).unwrap();
    let new_content = fs::read_to_string(&test_file).unwrap();
    // Should add correct header and strip all lines matching path comment regex
    assert_eq!(
        "// App.tsx\nimport React from 'react';\n// Another path comment: ./component.tsx\n",
        new_content
    );
}

// Import the find_git_root function if it's not public or in scope
use crate::find_git_root;
