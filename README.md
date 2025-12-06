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
