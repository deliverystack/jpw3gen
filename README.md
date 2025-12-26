# Static Site Generator Technical Documentation

## Overview
This project is a static site generator written in Rust that converts a directory structure of Markdown files into a navigable HTML website. It supports metadata extraction via JSON blocks, automatic navigation tree generation, and smart file synchronization.

---

## 1. Project Architecture
The system is modularized into several components that handle specific stages of the build process:

* **main.rs**: The orchestration layer that coordinates the execution flow.
* **args.rs**: Manages CLI argument parsing using `clap` (source, target, verbose flags).
* **config.rs**: Defines core data structures like `PageMetadata`, `NavItem`, and global configuration.
* **site_map.rs**: Discovers and maps all source files to identify what needs processing.
* **processing.rs**: Contains high-level logic for directory traversal and the Markdown-to-HTML conversion pipeline.
* **markdown.rs**: Implements Markdown parsing, content normalization, and link rewriting.
* **nav.rs**: Constructs a recursive navigation tree and generates `index.html` files for directories.
* **html.rs**: Handles final HTML formatting via templates and generates `sitemap.xml`.
* **io.rs**: Provides utility functions for console output and file reading.

---

## 2. Data Structures
The system relies on several key structures for state management:

### PageMetadata
Extracted from JSON code blocks at the end of Markdown files.
* `page_title`: Explicit title for the `<title>` tag.
* `nav_title`: Text used in the navigation menu.
* `avoid_generation`: If true, skips generating an HTML file for this source.
* `sort_key`: Used to override alphabetical sorting in navigation.
* `computed_title`: Fallback title extracted from the first H1 heading.

### NavItem
A recursive enum used to build the site’s hierarchy:
* `File`: Represents a single page with its relative path and current status.
* `Directory`: Represents a folder containing a `BTreeMap` of child `NavItems`.

---

## 3. Functional Call Tree
The following tree represents the execution flow from program start:

* `main()`
    * `parse_args()`: Validates directories and verbosity.
    * `read_template()`: Loads the HTML skeleton (`template.html`).
    * `build_site_map()`: Recursively catalogs all source files.
    * `load_all_metadata_from_files()`: Scans all `.md` files to build a global metadata map.
    * `process_directory()`: Primary recursive loop.
        * `markdown_to_html()`: Processes `.md` files into HTML.
            * `normalize_markdown_content()`: Cleans up control characters and formatting.
            * `check_broken_links()`: Validates internal links via regex.
            * `process_markdown_events()`: Rewrites links and handles auto-titling.
            * `generate_navigation_html()`: Builds the site-wide sidebar.
        * `smart_copy_file()`: Copies assets only if the content has changed.
    * `generate_all_index_files()`: Ensures every folder has an index page.
    * `generate_sitemap_xml()`: Produces the final SEO sitemap.



---

## 4. Key Logic & Processing

### Markdown Normalization
The `normalize_markdown_content` function performs several cleanup tasks:
* Removes non-essential control characters.
* Standardizes line endings and replaces special Unicode characters (like smart quotes) with standard ASCII equivalents.
* Converts source code comments (lines starting with `//`) into standard text.
* Applies specific formatting to `TODO` markers.

### Link Rewriting
Links within Markdown files are dynamically transformed:
* `.md` extensions are converted to `.html`.
* Absolute site paths (starting with `/`) are resolved relative to the current file.
* Links to directories are automatically pointed to that directory's `index.html`.
* If a link contains `{title}`, it is replaced with the computed title of the target page.

### Navigation Tree Generation
The navigation is built by iterating through the `SiteMap`. It respects `exclude_from_nav` flags and uses `sort_key` for ordering. Branches are automatically expanded if they contain the currently active page or if the branch is a parent of the active page.

---

## 5. Deployment and Outputs
* **Target Directory**: The final site is mirrored in the specified target path.
* **Sitemap**: A `sitemap.xml` is generated in the root, including `lastmod`, `changefreq`, and `priority` based on file metadata.
* **Index Files**: For directories missing an `index.md`, a default index is created listing available content.





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

- Documentation including features
- Refactoring, cleanup, and comment code
- Comment rust code.
- Document features (possibly using ChatGPT conversation?).
- Report links to deliverystack for update.
- Generate robots.txt
- --debug N for verbosity level
- navigation sorting seems to not work, for example articles/2025/May
- Storing the entire nav in every HTML file requires too much generation and makes the files to big and slow.
- Needs refactoring. Config.rs contains too much code, as does processing; some needs a new HTML library or something.
- Site search is not working well
- Clippy reports some issues; exit is commented in jpw3gen.sh