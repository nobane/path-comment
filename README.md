# `path-comment` CLI tool

```
CLI tool to prepend file paths as comments to source code files

Usage: path-comment [OPTIONS] <DIR>

Arguments:
  <DIR>  Directory to process files in

Options:
  -b, --base <BASE>                    Base directory for calculating relative paths. If not provided, searches upwards for a .git directory to use as the base. Falls back to the current working directory if no .git directory is found
      --no-git-base                    Disable searching for a .git directory to determine the base path. If --base is not provided, uses the current working directory
  -e, --extensions <EXTENSIONS>        File extensions to process (comma-separated)
      --config <CONFIG_FILE>           Configuration file for file extensions and comment styles
  -r, --recursive                      Process files recursively
  -d, --dry-run                        Dry run (don't modify files, just print what would be done)
  -s, --comment-style <COMMENT_STYLE>  Comment style to use (overrides config file) [possible values: slash, slash-star, hash, semi, xml, double-dash, percent]
  -f, --force                          Force processing of dependency directories (node_modules, venv, etc.)
  -x, --strip                          Strip existing path comments
      --clean                          Remove existing path comments instead of adding/updating them. If used, the --strip flag is ignored
  -p, --print-extensions               Print configured extensions styles, then exit
      --no-ignore-merge                Disable merging ignore rules from .gitignore found in the base directory
  -h, --help                           Print help
  -V, --version                        Print version
  ```