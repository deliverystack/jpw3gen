#!/usr/bin/bash
# https://github.com/deliverystack/wslbin/blob/main/runzip.bash.md

shopt -s globstar

script_name=$(basename "$0")
green=$(tput setaf 2)
yellow=$(tput setaf 3)
reset=$(tput sgr0)

usage() {
    echo -e "${green}${script_name}${reset}: ${yellow}Usage: $script_name <directory>${reset}"
    exit 1
}

clean_name() {
    echo "$1" | sed 's/[^a-zA-Z0-9._-]/_/g'
}

source_dir="${1:-.}"

if [[ ! -d "$source_dir" ]]; then
    echo -e "${green}${script_name}${reset}: ${yellow}Error: Directory $source_dir does not exist.${reset}"
    usage
fi

echo -e "${green}${script_name}${reset}: Starting to search for compressed files in ${green}$source_dir${reset} and extract them..."

for file in "$source_dir"/**/*.{tar,tar.gz,tar.bz2,zip,gz,rar,7z}; do
    [[ -e "$file" ]] || continue
    start_time=$(date +%s)

    dir=$(dirname "$file")
    base=$(clean_name "$(basename "$file" .tar)")
    base=$(clean_name "$(basename "$base" .tar.gz)")
    base=$(clean_name "$(basename "$base" .tar.bz2)")
    base=$(clean_name "$(basename "$base" .zip)")
    base=$(clean_name "$(basename "$base" .gz)")
    base=$(clean_name "$(basename "$base" .rar)")
    base=$(clean_name "$(basename "$base" .7z)")

    target_dir="${dir}/${base}"
    mkdir -p "$target_dir"

    echo -e "${green}${script_name}${reset}: Extracting ${green}$file${reset} to ${green}$target_dir${reset}..."

    if [[ "$file" == *.tar.gz ]]; then
        if tar -xzf "$file" -C "$target_dir"; then
            rm "$file"
        else
            echo -e "${green}${script_name}${reset}: ${yellow}Failed to extract: $file${reset}"
        fi
    elif [[ "$file" == *.tar.bz2 ]]; then
        if tar -xjf "$file" -C "$target_dir"; then
            rm "$file"
        else
            echo -e "${green}${script_name}${reset}: ${yellow}Failed to extract: $file${reset}"
        fi
    elif [[ "$file" == *.tar ]]; then
        if tar -xf "$file" -C "$target_dir"; then
            rm "$file"
        else
            echo -e "${green}${script_name}${reset}: ${yellow}Failed to extract: $file${reset}"
        fi
    elif [[ "$file" == *.zip ]]; then
        if unzip -o "$file" -d "$target_dir"; then
            rm "$file"
        else
            echo -e "${green}${script_name}${reset}: ${yellow}Failed to extract: $file${reset}"
        fi
    elif [[ "$file" == *.gz ]]; then
        if gunzip -c "$file" > "${target_dir}/${base}"; then
            rm "$file"
        else
            echo -e "${green}${script_name}${reset}: ${yellow}Failed to extract: $file${reset}"
        fi
    elif [[ "$file" == *.7z ]]; then
        if 7z x "$file" -o"$target_dir"; then
            rm "$file"
        else
            echo -e "${green}${script_name}${reset}: ${yellow}Failed to extract: $file${reset}"
        fi
    elif [[ "$file" == *.rar ]]; then
        if unrar x "$file" "$target_dir"; then
            rm "$file"
        else
            echo -e "${green}${script_name}${reset}: ${yellow}Failed to extract: $file${reset}"
        fi
    else
        echo -e "${green}${script_name}${reset}: ${yellow}Unsupported file type for: $file${reset}"
        continue
    fi

    end_time=$(date +%s)
    elapsed=$((end_time - start_time))
    echo -e "${green}${script_name}${reset}: Finished extracting ${green}$file${reset} in ${elapsed} seconds."
done

echo -e "${green}${script_name}${reset}: All files processed in ${green}$source_dir${reset}."
echo -e "${green}${script_name}${reset}: Run ${script_name} again to confirm no remaining volumes."