#!/usr/bin/bash
#https://chatgpt.com/share/673f7c19-0684-8005-a79e-4aade4484100

script_name=$(basename "$0")

# Default number of threads
threads=10

# Color functions
info() { echo -e "\033[0;32m${script_name}:\033[0m $1"; }
error() { echo -e "\033[0;31m${script_name}:\033[0m $1"; }

# Parse command-line arguments
while getopts ":s:t:" opt; do
    case ${opt} in
        s)
            source_dir="${OPTARG}"
            ;;
        t)
            target_dir="${OPTARG}"
            ;;
        *)
            error "Invalid option: -${OPTARG}"
            echo "Usage: ${script_name} -s <source_dir> -t <target_dir>"
            exit 1
            ;;
    esac
done

# Ensure mandatory parameters are provided
if [[ -z "${source_dir}" || -z "${target_dir}" ]]; then
    error "Both -s <source_dir> and -t <target_dir> are required."
    echo "Usage: ${script_name} -s <source_dir> -t <target_dir>"
    exit 1
fi

# Ensure the target directory exists
mkdir -p "${target_dir}"

info "Source directory: ${source_dir}"
info "Target directory: ${target_dir}"
info "Using ${threads} parallel threads."

# FFmpeg options (explicit mapping for audio and video streams)
ffmpeg_options="-map 0:v -map 0:a -c:v libx264 -preset fast -crf 23 -c:a aac -b:a 128k"

# Create an array of video files to process
mapfile -t video_files < <(find "${source_dir}" -type f ! -name "*.mp4")

# Function to convert a single file
convert_file() {
    local file="$1"
    local relative_path="${file#${source_dir}/}"
    local output_file="${target_dir}/$(dirname "${relative_path}")/$(basename "${relative_path}" .${file##*.}).mp4"
    
    mkdir -p "$(dirname "${output_file}")"
    info "Converting '${file}' to '${output_file}'..."
    ffmpeg -i "${file}" ${ffmpeg_options} "${output_file}" && \
    info "Completed: ${output_file}" || \
    error "Failed: ${file}"
}

# Process files in parallel using background jobs
count=0
for file in "${video_files[@]}"; do
    convert_file "${file}" &
    ((count++))

    # Wait for jobs to finish when reaching the thread limit
    if (( count % threads == 0 )); then
        wait
    fi
done

# Wait for any remaining jobs to finish
wait

info "All conversions completed."
