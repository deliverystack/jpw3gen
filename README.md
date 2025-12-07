# jpw3gen

This program iterates all of the files and subdirectories in a source directory, replicating them to a target directory, converting markdown (.md) files to HTML files, and generating an index.html file in each subdirectory.

Build the program:

```
cd ~/git/jpw3gen
cargo build
```

Copy the styles into the directory that contains the markdown files (or the output directory)

```
cp ~/git/jpw3gen/styles.css ~/git/jpw3
```

Generate the output files:

```
~/git/jpw3gen/target/debug/jpw3gen -t /tmp/jw -s ~/git/jpw3
```

Run the web server:

```
python3 -m http.server 8000 --directory /tmp/jw
```

Browse to:

- http://localhost:8000

Issues:

- I want to move the HTML to a separate template.html file at the root of the source directory.
- I want to make paths in URLs relative.
- No need for right column
- No need for header
- URLs in markdowns that aren't links should be converted to links, especially if they're list items
- Nav tree isn't indenting nested links or using elipses properly (http://localhost:8000/articles/2025/December/worst-mistakes.html)
- If there is no H1, then use the first H2.
- Add the file indicator if none is present.
- Don't overwrite HTML or other files if binary content has not changed.
- Color warnings and errors