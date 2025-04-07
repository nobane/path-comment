// src/comments.rs
use std::collections::HashMap;

use clap::ValueEnum;
use once_cell::sync::Lazy;
use regex::Regex;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug, Hash)]
pub enum Style {
    Slash,      // //
    SlashStar,  // /* */
    Hash,       // #
    Semi,       // ;
    Xml,        // <!-- -->
    DoubleDash, // --
    Percent,    // %
}

impl Style {
    fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "//" => Some(Style::Slash),
            "/* */" => Some(Style::SlashStar),
            "#" => Some(Style::Hash),
            ";" => Some(Style::Semi),
            "<!-- -->" => Some(Style::Xml),
            "--" => Some(Style::DoubleDash),
            "%" => Some(Style::Percent),
            _ => None,
        }
    }
    // Method to get the comment delimiters
    pub fn delimiters(&self) -> (&'static str, &'static str) {
        match self {
            Style::Slash => ("// ", ""),
            Style::SlashStar => ("/* ", " */"),
            Style::Hash => ("# ", ""),
            Style::Semi => ("; ", ""),
            Style::Xml => ("<!-- ", " -->"),
            Style::DoubleDash => ("-- ", ""),
            Style::Percent => ("% ", ""),
        }
    }
}

// Comment style delimiters - adjusted to use delimiters() method where possible
pub static DELIMITERS: Lazy<HashMap<Style, (String, String)>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for style in [
        Style::Slash,
        Style::SlashStar,
        Style::Hash,
        Style::Semi,
        Style::Xml,
        Style::DoubleDash,
        Style::Percent,
    ] {
        let (start, end) = style.delimiters();
        map.insert(style, (start.to_string(), end.to_string()));
    }
    map
});

// Pre-baked regexes for each comment style
pub static REGEXES: Lazy<HashMap<Style, Regex>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for style in [
        Style::Slash,
        Style::SlashStar,
        Style::Hash,
        Style::Semi,
        Style::Xml,
        Style::DoubleDash,
        Style::Percent,
    ] {
        let (start_raw, end_raw) = style.delimiters();
        // Trim whitespace for regex matching flexibility
        let start = start_raw.trim_end();
        let end = end_raw.trim_start();

        // Regex Explanation:
        // ^                     - Start of line
        // \s*                   - Optional leading whitespace
        // ({start_esc})         - Escaped start delimiter (captured)
        // \s*                   - Optional whitespace after start delimiter
        // (                     - Start capture group for the path
        //   (?:/|\\|[A-Za-z]:)? - Optional root: /, \, or drive letter D:
        //   (?:[\w\-\.\s]+(?:/|\\))+ - One or more directory/file parts separated by / or \
        //                             Allows word chars, hyphens, dots, spaces within names
        //   [\w\-\.\s]+         - The final filename part
        //   (?:\.\w+)?          - Optional file extension
        // |                     - OR (for simpler paths like just 'file.ext')
        //   [\w\-\.\s]+\.\w+    - Filename with extension
        // )                     - End capture group for the path
        // \s*                   - Optional whitespace before end delimiter
        // ({end_esc})           - Escaped end delimiter (captured)
        // \s*                   - Optional trailing whitespace
        // $                     - End of line
        let pattern = format!(
            r"^\s*({start_esc})\s*((?:/|\\|[A-Za-z]:)?(?:[\w\-\.\s]+(?:/|\\))+[\w\-\.\s]+(?:\.\w+)?|[\w\-\.\s]+\.\w+)\s*({end_esc})\s*$",
            start_esc = regex::escape(start),
            end_esc = regex::escape(end)
        );

        map.insert(
            style,
            Regex::new(&pattern)
                .unwrap_or_else(|e| panic!("Failed to compile regex pattern for {style:?} style with pattern '{pattern}': {e}")),
        );
    }

    map
});

pub type CommentConfig = HashMap<String, Style>;
// Default configuration string with common file extensions and their comment styles
const DEFAULT_CONFIG: &str = include_str!("comments.cfg");
pub fn default_config() -> CommentConfig {
    parse_config(DEFAULT_CONFIG)
}

pub fn parse_config(content: &str) -> CommentConfig {
    let mut extension_styles = HashMap::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Split line into extension and comment style
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            // Extension is always the first part, remove leading dot if present
            let extension = parts[0].trim_start_matches('.').to_lowercase();
            let style_str = parts[1..].join(" ");

            if let Some(style) = Style::from_str(&style_str) {
                extension_styles.insert(extension, style);
            } else {
                eprintln!(
                    "Warning: Unknown comment style '{}' for extension '.{}' in config file, skipping",
                    style_str, extension
                );
            }
        } else if parts.len() == 1 {
            eprintln!(
                "Warning: Missing comment style for extension '.{}' in config file, skipping",
                parts[0]
            );
        }
    }

    extension_styles
}
