## `jpw3gen`: Simple Markdown to Static HTML Site Generator

`jpw3gen` is a static site generator written in Rust. `jpw3gen` repllicates a source directory structure containing markdown files as a navigable HTML website in a corresponding target directory structure.

This has been called the Cheapest Content Management System (CMS) implementation in history:

- https://www.jpw3.com/articles/2025/December/static-site.html

## Files, File Systems, and Decisions

For all build and deployment processes, there are four relevent file systems:

- Location of the source (markdown) file system (`/home/jw/git/jpw3/` in the examples).
- Location of the target (HTML) file system (`/home/jw/tmp/jpw3/` in the examples).
- Where to store the file system builder source code (`/tmp/git/jpw3gen/` in the examples). 
- Where to build the file system builder binary (`/tmp/cargo/` in the examples).

This process depends on two files in a source directory. You can copy these from this project.

- `/template.html`: Template for HTML files.
- `/styles.css` (technically optional): CSS referenced in HTML files.

### jpw3gen Source Files

The jpw3gen Rust project (`/tmp/git/jpw3gen/`) consists of `/Cargo.toml` and the files in the `/src` directory. 

- [`/Cargo.toml`](../main/Cargo.toml) - Project configuraiton
- [`/src/main.rs`](../main/src/main.rs) - Starting point
- [`/src/args.rs`](../main/src/args.rs) - Command line arguments (Clap)
- [`/src/config.rs`](../main/src/config.rs) - Data structures and program configuration
- [`/src/processing.rs`](../main/src/processing.rs) - Directory traversal and markdown conversion control
- [`/src/html.rs`](../main/src/html.rs) - HTML generation
- [`/src/io.rs`](../main/src/io.rs) - File system and console interaction
- [`/src/markdown.rs`](../main/src/markdown.rs) - Markdown processing including gnormalization and link rewriting
- [`/src/nav.rs`](../main/Cargo.toml) - Navigation generation including `/sitemap.xml`
- [`/src/sitem_map.rs`](../main/Cargo.toml) - Source file metadata

The `jpw3gen.sh` script builds the jpw3gen rust binary and invokes it to convert a source markdown directory to a target HTML directory.

- [`/bin/jpw3gen.sh`](../main/bin/jpw3gen.sh)

### jpw3gen Process

The jpw3gen.sh shell script builds and invokes the jpw3gen Rust command line tool to synchronize a target file system from a source file system. The Rust command:

- Creates a directory in the target for each directory in the source.
- Extracts optional JSON metadata from source markdown files indlucing `index.md` for directories.
- Creates an `.html` file in the target for each `.md` file in the source.
- Removes non-essential control characters, standardizes line endings, replaces special characters, and formats `TODO` markers.
- Attempts to convert links such as `[{title}](../page.md)`, replacing [{title}] or [../page.md] or [] with the navigation title of page.md. 
- Converts bare URLs in source markdown to HTML anchors (links).
- Reports links to local markdown files and images that do not exist.
- Creates an `index.html` file in each target directory even if there is no `index.md` file in the source.
- Links to directories are automatically pointed to that directory's `index.html`.
- In URLs, `.md` extensions are converted to `.html`.
- Embeds HTML navigation based on directory structure in each page.
- Copies most other files from the target to the source.
- Generates `/sitemap.xml`.
- Only overwrites files if binary content has changed.

> **NOTE**: The `jpw3gen.sh` that builds and invokes the Rust binary before optionaly invoking git may delete files before invoking the `jpw3gen` Rust command to generate files.

The resulting static HTML can contain JavaScript for client-side logic that post to other servers. 

The process may ignore hidden files and directories (those that start with `.`), files named `template.html` or `favicon.ico`, and any files ending `css`, `js`, `xml`, `html`, `json`, and/or `ico` as hard-coded into the `EXCLUDED_EXTENSIONS` and `EXCLUDED_FILE_NAMES` variables that appear twice for two different purposes. The `jpw3gen.sh` script may manage some of these files explicitly.

To generate markup files, the jpw3gen rust static site generation procecess subtitutes the following tokens from the `/template.html` file in the source directory with calcluated values that may involve a markdown file in the source directory:

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

The last steps in the `jpw3gen.sh` optionally truncate `git` history and check-in the generated HTML, which can trigger automatic deploymnent such as with Vercel.

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

## Run and Access a Web Server

Once you have built the target directory, you can run a web server and access it from a browser:

```
python3 -m http.server 8000 --directory /home/jw/tmp/jpw3
```

Browse to:

- http://localhost:8000

You can configure this web server to start automatically:

- https://www.jpw3.com/articles/2025/December/web-server.html

