# jpw3gen Static Site Generator Technical Documentation

## jpw3gen Overview

jpw3gen is a static site generator written in Rust to convert a directory structure of markdown files into a navigable HTML website. jpw3gen supports metadata extraction via JSON blocks in the markdown, automatic navigation tree generation based on directory structure, and file system synchronization rather than replacement. 

### jpw3gen Source Files

The jpw3gen Rust projecIt consists of `/Cargo.toml` and the files in the `/src` directory. 

- [`/Cargo.toml`](../main/Cargo.toml)
- [`/src/main.rs`](../main/src/main.rs) - Starting point
- [`/src/args.rs`](../main/Cargo.toml) - Command line arguments (Clap)
- [`/src/config.rs`](../main/Cargo.toml) - Data structures and program configuration
- [`/src/processing.rs`](../main/Cargo.toml) - Directory traversal and markdown conversion control
- [`/src/html.rs`](../main/Cargo.toml) - HTML generation
- [`/src/io.rs`](../main/Cargo.toml) - File system interaction
- [`/src/markdown.rs`](../main/Cargo.toml) - Markdown processing including gnormalization and link rewriting
- [`/src/nav.rs`](../main/Cargo.toml) - Navigation generation
- [`/src/sitem_map.rs`](../main/Cargo.toml) - Source file metadata

The `/bin/jpw3gen.sh` script builds the jpw3gen rust binary and invokes it to convert a source markdown directory to a target HTML directory.

https://github.com/deliverystack/jpw3gen
https://github.com/deliverystack/jpw3gen 

### jpw3gen Process

The jpw3gen.sh shell script invokes the jpw3gen Rust command line tool to create a target file system from a source file system. The Rust command:

- Creates a directory in the target for each directory in the source.
- Creates an .html file in the target for each .md file in the source.
- Creates an index.html file in the target even if there is no .md file in the source.
- Copies most other files from the target to the source.
- Generates `/sitemap.xml`.

The process ignores hidden files and directories (those that start with `.`). 

The process only touches files for which content has changed.

//TODO: The process ignores files named `template.html` and `favicon.ico` as well as any files ending `css`, `js`, `xml`, `html`, or `json`.

//TODO: Navigation ignores files named  `template.html` and `favicon.ico` as well as any files ending `css`, `js`, `xml`, `html`, or `ico`.

//TODO:         const EXCLUDED_FILE_NAMES: [&str; 2] = ["template.html", "favicon.ico"];
//TODO: const EXCLUDED_EXTENSIONS: [&str; 5] = ["css", "js", "xml", "html", "json"]; 
//TODO: const EXCLUDED_FILE_NAMES: [&str; 2] = ["template.html", "favicon.ico"];
//TODO: const EXCLUDED_EXTENSIONS: [&str; 5] = ["css", "js", "xml", "html", "ico"];
//TODO: managed by jpw3gen.sh script

Conversion from markdown to markup subtitutes the following tokens in `/template.html` in the source:

 Token                  | Value
 -----------------------|------
`{{ title }}`           | HTML page title.
`{{ canonical_url }}`   | HTML page canonical URL.
`{{ header_title }}`    | HTML header title.
`{{ breadcrumb_html }}` | HTML breadcrumb.
`{{ nav_html }}`        | Navigation HTML.
`{{ content }}`         | Page content.
`{{ source_path }}`     | Markdown file path.
`{{ date_created }}`    | Markdown file date created.
`{{ last_modified }}`   | Markdown file date modified.

###//TODO: To determine the title:

###//TODO: To determine the navigation title:

###//TODO: Markdown JSON Format

### Images, Links, and URLs 

The JSON in each markup file can specify the navigation title for the each generated page including index.html for directories.

The static site generation process attempts to report broken local page and image URLs.

The static site generation process attempts to convert links such as ({title})[../page.md], replacing ({title}) or (../page.md) or () with the navigation title of the page. 

Static site generation attempts to convert bare URLs to links. 

### Markdown JSON Fragment Format

Each markdown file can contain metadata in a JSON fragment at the end.

```json
{
  "page_title": "Page Title",       // HTML page title.
  "nav_title": "Short Title",       // Short title for navigation.
  "avoid_generation": false,        // Don't generate an HTML file or process directory.
  "exclude_from_nav": false,        // Exclude this file from the site nav.
  "keep_json_in_content": false,    // Include this JSON in the HTML.
  "sort_key": "text"                // For sorting the entry relative to its siblings.
}
```





























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