
# jpw3gen - Generate Static HTML Site from Markdown File System

This program iterates all of the files and subdirectories in a source directory, replicating them to a target directory, converting markdown (`.md`) files to HTML files, and generating an index.html file in each subdirectory.

- http://localhost:8000/articles/2025/December/static-site.html

## Manual Process Before Use

This process depends on two files in the source directory. You can copy these from this project.

- `/template.html`: Template for HTML files.
- `/styles.css` (technically optional): CSS referenced in HTML files.

Make some decisions:

- Location of the source file system is (`/home/jw/git/jpw3/` in the examples).
- Location of the target file system (`/home/jw/tmp/jpw3` in the examples).
- Where to store the file system builder source code (`/tmp/git/jpw3gen/` in the examples). 
- Where to build the file system builder binary (`/tmp/cargo/` in the examples)

## Building the File System Generator

You may want to see:

- https://github.com/deliverystack/jpw3gen/blob/main/jwbnr.sh
- https://deliverystack.net/2025/12/10/fedora-linux-simple-static-web-server-startup

To build the program:

```
export CARGO_TARGET_DIR=/tmp/cargo      # build to the working directory
mkdir /tmp/git                          # base directory for project (source code)
cd /tmp/git
gh repo clone deliverystack/jpw3gen     # get the code
cd jpw3gen           
cargo build                             # build the binary
```

## Generating the File System

Generate the output files:

```
rm -r /tmp/jw                           # (remove existing target; optional)
/tmp/cargo/debug/jpw3gen --source ~/git/jpw3 --target /tmp/jw 
```

## Run and Access a Web Server

Run the web server:

```
python3 -m http.server 8000 --directory /tmp/jw
```

Browse to:

- http://localhost:8000

## Features

- Replicate directory structure.
- Convert .md files in source to HTML files in target.
- Generate index.html in each directory (use index.md if it exists).
- Rewrite links to local markdown files to link to corresponding HTML files.
- Report links to local markdown files that do not exist.
- Use the first # or ## markdown heading in the .md file as the HTML page title, or the file path otherwise.
- Copy every other file (except maybe styles.css and template.html).
- Only overwrite files if binary content has changed.
- In each HTML file, generate navigation based on directory structure.

## Outstanding Issues

- Open external URLs in new tabs.
- Apply ideas from https://github.com/deliverystack/wink/blob/main/wince to jpw3gen.sh
- Documentation including features
- Refactoring, cleanup, and comment code
- Nav has issues.
- Comment rust code.
- Document features (possibly using ChatGPT conversation?).
- Report links to deliverystack for update.
- It seems to ignore index.md in directories, for example "exclude_from_nav": true. Maybe index shouldn't appear in nav.
- Generate Sitemap.xml and optionally robots.txt (using JSON)
- --debug N for verbosity level
- exlcude favion from nav
- confirm nav sort sorts by nav title when present, not file name, etc.