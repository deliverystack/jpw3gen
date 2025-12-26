#!/bin/bash

set -euo pipefail
shopt -s globstar 
script_name=$(basename "$0")

LIB_PATH="$HOME/bin/lib.bash"
if [[ ! -f "$LIB_PATH" ]]; then
    echo "ERROR: Library $LIB_PATH not found. Exiting."
    exit 1
fi
source "$LIB_PATH"

supported_images="jpg|jpeg|png|bmp|gif"
debug=false
verbose=false

usage() {
    info "${script_name}: Process images in a directory."
    info "Usage: ${script_name} [options]"
    info "Options:"
    info "  -d, --debug             Enable debug mode (default: ${debug})"
    info "  -h, --help              Show this help message"
    info "  -r, --root DIR          Root directory to process (default: current directory, ${PWD})"
    info "  -v, --verbose           Enable verbose mode (default: ${verbose})"
    exit 0
}

root_dir="."
while [[ $# -gt 0 ]]; do
    case "$1" in
        -d|--debug)
            debug=true
            shift
            ;;
        -h|--help)
            usage
            ;;
        -r|--root)
            root_dir="$2"
            shift 2
            ;;
        -v|--verbose)
            verbose=true
            shift
            ;;
        *)
            error "Unknown option: $1"
            usage
            ;;
    esac
done

//TODO: better parsing
//TODO: check feh

process_image() {
    local file="$1"
    while true; do
        info "Processing: $file"
        info "Choose an action:"

        if [[ "$file" != *.png ]]; then
            info "  [c] Convert to PNG"
        fi

        info "  [d] Done"
        info "  [m] Move"
        info "  [r] Rename"
        read -rp "Action: " action

        case "$action" in
            m)
                file=move_file "$file"
                ;;
            r)
                new_name=clean_name "$file"
                read -rp "Enter new name: " $new_name
                new_path="$(dirname "$file")/$new_name"
                mv "$file" $new_path
                file=$new_path
                ;;
            c)
                if [[ "$file" != *.png ]]; then
                    convert "$file" "${file%.*}.png"
                else
                    warn "$file is already a PNG."
                fi
                ;;
            d)
                break
                ;;
            *)
                warn "Invalid action. Please choose again."
                ;;
        esac
    done
}

move_file() {
    local file="$1"
    info "Available directories:"
    local dirs=()
    local i=1
    for dir in */; do
        dirs+=("$dir")
        printf "  [%d] %s\n" "$i" "$dir"
        ((i++))
    done

    printf "  [%d] Create new directory\n" "$i"
    read -rp "Select an option: " choice

    if ((choice > 0 && choice <= ${#dirs[@]})); then
        local selected_dir="${dirs[$((choice - 1))]}"
        info "Moving $file to $selected_dir"
        mv "$file" "$selected_dir"
    elif ((choice == i)); then
        read -rp "Enter new directory name: " new_dir
        mkdir -p "$new_dir"
        info "Moving $file to $new_dir"
        mv "$file" "$new_dir"
    else
        warn "Invalid choice. Skipping move."
    fi

    echo selected_dir/$(basename $file)
}

if [[ ! -d "$root_dir" ]]; then
    error "Root directory $root_dir does not exist."
    exit 1
fi

for file in "$root_dir"/**/*; do
    if [[ -f "$file" && "$file" =~ \.($supported_images)$ ]]; then
        display_image "$file"
        process_image "$file"
    fi
done