#!/bin/bash

# Temporary file to track hashes of extracted files
tmpfile=$(mktemp)

# Find all zip files recursively
find . -type f -name "*.zip" -print0 | while IFS= read -r -d '' zipfile; do
    # Create a target directory matching the zip path
    outdir="${zipfile%.zip}"
    mkdir -p "$outdir"

    # Create a temporary folder to extract files
    tmpdir=$(mktemp -d)
    unzip -qq "$zipfile" -d "$tmpdir"

    # Move only unique files based on hash
    find "$tmpdir" -type f -print0 | while IFS= read -r -d '' file; do
        hash=$(md5sum "$file" | cut -d' ' -f1)
        if ! grep -qx "$hash" "$tmpfile"; then
            mv "$file" "$outdir/"
            echo "$hash" >> "$tmpfile"
        fi
    done

    # Clean up temporary extraction folder
    rm -rf "$tmpdir"
done

# Remove hash tracking file
rm -f "$tmpfile"
