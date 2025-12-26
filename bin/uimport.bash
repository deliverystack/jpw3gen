#!/usr/bin/bash

shopt -s globstar
script_name=$(basename "$0")
green=$(tput setaf 2)
yellow=$(tput setaf 3)
red=$(tput setaf 1)
reset=$(tput sgr0)
datestamp=$(date +%Y%m%d)

if [[ $# -ne 2 ]]; then
    printf "%s: ${yellow}Error${reset}: Source and target directories are required.\n" \
        "${green}${script_name}${reset}"
    printf "%s: Usage: %s <source_dir> <target_dir>\n" \
        "${green}${script_name}${reset}" "${green}${script_name}${reset}"
    exit 1
fi

source_dir=$1
target_dir=$2

if [[ ! -d $source_dir ]]; then
    printf "%s: ${yellow}Error${reset}: Source directory %s does not exist.\n" \
        "${green}${script_name}${reset}" "${yellow}${source_dir}${reset}"
    exit 1
fi

if [[ ! -d $target_dir ]]; then
    printf "%s: ${yellow}Error${reset}: Target directory %s does not exist.\n" \
        "${green}${script_name}${reset}" "${yellow}${target_dir}${reset}"
    exit 1
fi

clean_name() {
    local original_name="$1"
    local cleaned_name

    cleaned_name=$(echo "$original_name" | sed 's/[^a-zA-Z0-9._-]/_/g' | sed 's/^_*\|_*$//g')

    if [[ ${#cleaned_name} -lt 3 ]]; then
        cleaned_name="file_$(date +%Y%m%d%H%M%S)"
    fi

    echo "$cleaned_name"
}

declare -A file_size_map

printf "%s: Scanning target directory %s for files...\n" \
    "${green}${script_name}${reset}" "${green}${target_dir}${reset}"

for file in "$target_dir"/**/*; do
    if [[ $file == *Zone.Identifier ]]; then
        rm "$file"
        continue
    fi

    if [[ -f $file ]]; then
        size=$(stat --printf="%s" "$file")
        file_size_map[$size]+="$file "
    fi
done

printf "%s: Processing files from source directory %s...\n" \
    "${green}${script_name}${reset}" "${green}${source_dir}${reset}"

for source_file in "$source_dir"/**/*; do
    if [[ -f $source_file ]]; then

        if [[ $source_file == *Zone.Identifier ]]; then
            rm "$source_file"
            continue
        fi

        size=$(stat --printf="%s" "$source_file")
        if [[ -n "${file_size_map[$size]}" ]]; then
            matched=false
            for target_file in ${file_size_map[$size]}; do
                if cmp -s "$source_file" "$target_file"; then
                    printf "%s: Skipped duplicate file. Source: %s, Target: %s\n" \
                        "${green}${script_name}${reset}" \
                        "${yellow}${source_file}${reset}" \
                        "${yellow}${target_file}${reset}"
                    matched=true
                    break
                fi
            done
            if $matched; then
                continue
            fi
        fi

        relative_path=$(realpath --relative-to="$source_dir" "$source_file" | sed 's|/./|/|')
        year=$(date -r "$source_file" +%Y)
        month=$(date -r "$source_file" +%m)
        dest_dir="$target_dir/import/$datestamp/$year/$month/$(dirname "$relative_path")"
        mkdir -p "$dest_dir"
        dest_file="$dest_dir/$(clean_name "$(basename "$source_file")")"
        cp "$source_file" "$dest_file"
        file_size_map[$size]+="$dest_file "

        
        printf "%s: Copied %s to %s\n" \
            "${green}${script_name}${reset}" \
            "${green}${source_file}${reset}" \
            "${green}${dest_file}${reset}"
    fi
done

printf "%s: File processing completed successfully.\n" "${green}${script_name}${reset}"
