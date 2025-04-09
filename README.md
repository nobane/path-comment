# `path-comment` CLI tool

```
CLI tool to prepend file paths as comments to source code files

Usage: path-comment [OPTIONS] <DIR>

Arguments:
  <DIR>  Directory to process files in

Options:
  -b, --base <BASE>                    Base directory for calculating relative paths. If not provided, searches upwards for a .git directory to use as the base. Falls back to the current working directory if no .git directory is found
  -k, --keep                           Keep other existing path comments in the file. By default, all path comments are removed from the file
      --clean                          If used, the --keep is ignored
  -f, --force                          Process folders that would normally be ignored (node_modules, venv, etc.)
      --no-git                         Disable searching for a .git directory to determine the base path. If --base is not provided, uses the current working directory
      --no-recursive                   Disables processing files recursively
      --no-ignore-merge                Disable merging ignore rules from .gitignore found in the base directory
  -e, --extensions <EXTENSIONS>        File extensions to process (comma-separated), eg `rs,ts,toml`
      --config <CONFIG_FILE>           Configuration file for file extensions and comment styles
  -d, --dry-run                        Dry run (don't modify files, just print what would be done)
  -s, --comment-style <COMMENT_STYLE>  Force override a specific comment style to use (overrides config file) [possible values: slash, slash-star, hash, semi, xml, double-dash, percent]
  -p, --print-extensions               Print configured extensions styles, then exit
  -h, --help                           Print help
  -V, --version                        Print version
```

## Note

This tool is still an active WIP. **Please use with caution**.