#!/usr/bin/bash


shopt -s globstar

# Set script name
script_name="$(basename "$0")"

# Temporary file for invalid files
temp_file=$(mktemp)

# Cleanup on exit
trap "rm -f ${temp_file}" EXIT

# Define color functions
green() { tput setaf 2; printf "%s" "$1"; tput sgr0; }
yellow() { tput setaf 3; printf "%s" "$1"; tput sgr0; }
red() { tput setaf 1; printf "%s" "$1"; tput sgr0; }

# Logging functions
log() {
    printf "%s: %s\n" "$(green "${script_name}")" "$1"
}

warn() {
#    printf "%s: %s\a\n" "$(yellow "${script_name}")" "$1"
    printf "%s: %s\n" "$(yellow "${script_name}")" "$1"
}

error() {
    printf "%s: %s\n" "$(red "${script_name}")" "$1"
    exit 1
}

debug() {
    if [ "${verbose:-0}" -eq 1 ]; then
        printf "%s: %s\n" "$(green "${script_name}")" "$1"
    fi
}

declare -A renamed_dirs  # Track renamed directories globally

resolve_renamed_path() {
    local original_path="$1"
    local relative_path="${original_path#"${source_dir}/"}"
    local renamed_path="${target_dir}"

    debug "Resolving renamed path for: ${original_path}"
    debug "Initial relative path: ${relative_path}"

    # Split the relative path into its components
    IFS='/' read -ra path_parts <<< "${relative_path}"

    for part in "${path_parts[@]}"; do
        # Check if the part has already been renamed
        if [[ -v "renamed_dirs[${part}]" ]]; then
            renamed_part="${renamed_dirs[${part}]}"
        else
            clean_name "${part}"
            renamed_part="${REPLY}"

            # Track the renamed part to avoid inconsistent random renames
            renamed_dirs["${part}"]="${renamed_part}"
        fi

        debug "Original part: ${part}, Renamed part: ${renamed_part}"
        renamed_path="${renamed_path}/${renamed_part}"
    done

    debug "Final resolved path: ${renamed_path}"
    REPLY="${renamed_path}"
}


clean_name() {
    local original="$1"
    local base
    local ext=""
    local cleaned

    debug "Processing file: ${original}"

    # Split the filename into base and extension
    if [ -d "$original" ]; then
        base="$original"
        debug "Detected directory. Using base: ${base}"
    else
        base="${original%.*}"
        ext="${original##*.}"

        if [[ "$base" == "$original" ]]; then
            ext=""
        fi
        debug "Split name into base: '${base}', extension: '${ext}'"
    fi

    # Skip renaming for ignored extensions
    for ignored_ext in "${ignore_ext_array[@]}"; do
        if [[ "${ext,,}" == "${ignored_ext,,}" ]]; then
            debug "Skipping file due to ignored extension: ${ext}"
            REPLY="$original"
            return
        fi
    done

    # Sanitize the name
    cleaned=$(echo "$base" | sed -E "
        s|[^a-zA-Z0-9.-]|-|g;  # Replace invalid characters (excluding underscores)
        s|-{2,}|-|g;           # Remove repeated dashes
        s|[-]$||g;             # Remove trailing dashes
    " | tr '[:upper:]' '[:lower:]')

    # Apply minimum length check unless disabled
    if [ "${disable_length_check}" -eq 0 ] && [ "${#cleaned}" -lt 3 ]; then
        # Use a consistent name for directories
        if [[ -v "renamed_dirs[${original}]" ]]; then
            cleaned="${renamed_dirs[${original}]}"
        else
            cleaned="dir-${RANDOM}"
            renamed_dirs["${original}"]="${cleaned}"
        fi
    fi

    # Reattach the extension for files
    if [[ -n "$ext" && ! -d "$original" ]]; then
        cleaned="${cleaned}.${ext}"
    fi

    REPLY="$cleaned"
    return
}

clean_name() {
    local original="$1"
    local base
    local ext=""
    local cleaned

    debug "Processing file: ${original}"

    # Split the filename into base and extension
    if [ -d "$original" ]; then
        base="$original"
        debug "Detected directory. Using base: ${base}"
    else
        base="${original%.*}"
        ext="${original##*.}"

        if [[ "$base" == "$original" ]]; then
            ext=""
        fi
        debug "Split name into base: '${base}', extension: '${ext}'"
    fi

    # Skip renaming for ignored extensions
    for ignored_ext in "${ignore_ext_array[@]}"; do
        if [[ "${ext,,}" == "${ignored_ext,,}" ]]; then
            debug "Skipping file due to ignored extension: ${ext}"
            REPLY="$original"
            return
        fi
    done

    cleaned=$(echo "$base" | sed -E "
        s|[^a-zA-Z0-9._-]|-|g;
        s|-{2,}|-|g;
        s|[-]$||g;
    " | tr '[:upper:]' '[:lower:]')

    # Apply minimum length check unless disabled
    if [ "${disable_length_check}" -eq 0 ] && [ "${#cleaned}" -lt 3 ]; then
        cleaned="file-${RANDOM}"
    fi

    if [[ -n "$ext" && ! -d "$original" ]]; then
        cleaned="${cleaned}.${ext}"
    fi

    REPLY="$cleaned"
    return
}

usage() {
    cat <<EOF
Usage: $(green "${script_name}") [options]
  -d Disable renaming directories
  -f Force renaming without prompting
  -h Show this help message
  -i <extensions> Comma-separated extensions to ignore
  -l Disable minimum length requirement for filenames
  -r Generate a report instead of renaming/copying
  -s <dir> Source directory (default: .)
  -t <dir> Target directory (copy files instead of renaming)
  -v Enable verbose output
EOF
    exit 0
}

source_dir="."
verbose=0
target_dir=""
generate_report=0
force_rename=0
disable_rename_directories=0
disable_length_check=0

while getopts "dfhi:lrs:t:v" opt; do
    case ${opt} in
        d) disable_rename_directories=1 ;;
        f) force_rename=1 ;;
        h) usage ;;
        i) ignore_extensions="${OPTARG}" ;;
        r) generate_report=1 ;;
        s) source_dir="${OPTARG}" ;;
        t) target_dir="${OPTARG}" ;;
        v) verbose=1 ;;
        *) usage ;;
    esac
done

IFS=',' read -r -a ignore_ext_array <<< "${ignore_extensions}"

if [[ ${#ignore_ext_array[@]} -eq 0 ]]; then
    log "No ignored extensions specified or array is empty."
else
    log "Ignored extensions array: $(green "${ignore_ext_array[*]}")"
fi

# Validate conflicting options
if [[ "${generate_report}" -eq 1 && -n "${target_dir}" ]]; then
    error "Options -r (generate report) and -t (specify target directory) cannot be used together."
fi

# Validate source directory
if [ ! -d "${source_dir}" ]; then
    error "Source directory does not exist: $(yellow "${source_dir}")"
fi

if [ -n "${target_dir}" ] && [ "${source_dir}" == "${target_dir}" ]; then
    error "Source directory $(yellow "${source_dir}") and target directory $(yellow "${target_dir}") cannot be the same."
fi

log "Starting script with source_dir=$(green "${source_dir}")"
if [ -n "${target_dir}" ]; then
    log "Target directory is set: $(green "${target_dir}")"
    if [ ! -d "${target_dir}" ]; then
        warn "Target directory does not exist: $(yellow "${target_dir}")"
        exit 1
    fi
fi

for file in "${source_dir}"/**/*; do
    if [ ! -e "${file}" ]; then
        warn "Skipping unknown type: ${file}"
        continue
    fi

    clean_name "$(basename "${file}")"
    new_name="$REPLY"

    if [[ "$(basename "${file}")" != "${new_name}" ]]; then
        echo "${file}:${new_name}" >> "${temp_file}"
        warn "Invalid file name: $(yellow "${file}") -> $(green "${new_name}")"
    fi
done

log "Found $(green "$(wc -l < "${temp_file}")") invalid files."

if [ "${generate_report}" -eq 1 ]; then
    log "Generating report..."
    while IFS=: read -r old_name new_name; do

        if [ -f "${old_name}" ]; then
            debug "File detected: ${old_name}"
        elif [ -d "${old_name}" ]; then
            debug "Directory detected: ${old_name}"
        else
            warn "Skipping unknown type: ${entry}"
            continue
        fi

        printf "%s: %s -> %s\n" \
            "$(green "${script_name}")" \
            "$(yellow "${old_name}")" \
            "$(green "${new_name}")"
    done < "${temp_file}"
    exit 0
fi

declare -A renamed_dirs  # Track renamed directories

while IFS= read -r -d '' entry; do
    clean_name "$(basename "${entry}")"
    new_name="$REPLY"

    debug "Processing entry: ${entry}"
    debug "Cleaned name: ${new_name}"

    if [ -d "${entry}" ]; then
        # Process directories
        debug "Detected directory: ${entry}"

        if [ "${disable_rename_directories}" -eq 1 ]; then
            debug "Skipping directory renaming for: $(yellow "${entry}")"
            renamed_dirs["${entry}"]="${entry}"  # Track directory as-is
            continue
        fi

        if [ -n "${target_dir}" ]; then
            # Create new path for directory in target_dir
            resolve_renamed_path "${entry}" "${target_dir}"
            new_dir_path="${REPLY}"

            debug "Resolved directory path: ${new_dir_path}"

            mkdir -p "${new_dir_path}" || error "Failed to create directory: $(yellow "${new_dir_path}")"
            renamed_dirs["${entry}"]="${new_dir_path}"
            log "Processed directory: $(green "${entry}") to $(green "${new_dir_path}")"
        else
            # Rename directory in place
            if [ "${entry}" != "$(dirname "${entry}")/${new_name}" ]; then
                if [ "${force_rename}" -eq 1 ]; then
                    mv "${entry}" "$(dirname "${entry}")/${new_name}" || error "Failed to rename directory: $(yellow "${entry}")"
                    renamed_dirs["${entry}"]="$(dirname "${entry}")/${new_name}"
                    log "Renamed directory: $(green "${entry}") to $(green "${renamed_dirs["${entry}"]}")"
                    printf "\a"
                else
                    printf "%s: Invalid directory name detected: %s\n" \
                        "$(green "${script_name}")" "$(yellow "${entry}")"
                    read -r -p "Enter new name [default: $new_name, skip to move on]: " user_input
                    if [ -z "${user_input}" ]; then
                        user_input="${new_name}"
                    elif [ "${user_input}" = "skip" ]; then
                        warn "Skipping: $(yellow "${entry}")"
                        continue
                    fi
                    mv "${entry}" "$(dirname "${entry}")/${user_input}" || {
                        warn "Failed to rename: $(yellow "${entry}")"
                        continue
                    }
                    renamed_dirs["${entry}"]="$(dirname "${entry}")/${user_input}"
                    log "Renamed directory: $(green "${entry}") to $(green "${renamed_dirs["${entry}"]}")"
                    printf "\a"
                fi
            else
                renamed_dirs["${entry}"]="${entry}"
            fi
        fi
    elif [ -f "${entry}" ]; then
        # Process files
        debug "Detected file: ${entry}"

        if [ -n "${target_dir}" ]; then
            # Determine new parent directory if renamed
            parent_dir="$(dirname "${entry}")"
            new_parent_dir="${renamed_dirs["${parent_dir}"]:-${target_dir}/${parent_dir#"${source_dir}"/}}"
            target_path="${new_parent_dir}/${new_name}"

            debug "Resolved file target path: ${target_path}"

            mkdir -p "$(dirname "${target_path}")" || error "Failed to create parent directory: $(yellow "$(dirname "${target_path}")")"
            cp "${entry}" "${target_path}" || error "Failed to copy file: $(yellow "${entry}")"
            log "Copied file: $(green "${entry}") to $(green "${target_path}")"
            printf "\a"
        else
            # Rename file in place
            if [ "${entry}" != "$(dirname "${entry}")/${new_name}" ]; then
                if [ "${force_rename}" -eq 1 ]; then
                    mv "${entry}" "$(dirname "${entry}")/${new_name}" || error "Failed to rename file: $(yellow "${entry}")"
                    log "Renamed file: $(green "${entry}") to $(green "$(dirname "${entry}")/${new_name}")"
                    printf "\a"
                else
                    printf "%s: Invalid file name detected: %s\n" \
                        "$(green "${script_name}")" "$(yellow "${entry}")"
                    read -r -p "Enter new name [default: $new_name, skip to move on]: " user_input
                    if [ -z "${user_input}" ]; then
                        user_input="${new_name}"
                    elif [ "${user_input}" = "skip" ]; then
                        warn "Skipping: $(yellow "${entry}")"
                        continue
                    fi
                    mv "${entry}" "$(dirname "${entry}")/${user_input}" || {
                        warn "Failed to rename: $(yellow "${entry}")"
                        continue
                    }
                    log "Renamed file: $(green "${entry}") to $(green "$(dirname "${entry}")/${user_input}")"
                    printf "\a"
                fi
            fi
        fi
    else
        warn "Skipping unknown type: $(yellow "${entry}")"
    fi
done < <(find "${source_dir}" -mindepth 1 -print0)

log "Processing complete."