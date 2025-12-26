#!/usr/bin/bash 

script_name=$(basename "$0")
trap cleanup INT TERM
color_reset=$(tput sgr0)
color_info=$(tput setaf 2)  # Green
color_warn=$(tput setaf 3)  # Yellow
color_error=$(tput setaf 1) # Red
color_var=$(tput setaf 4)   # Blue
img_time=5
video_duration=0
shuffle=false
verbose=false
reverse=false
sort_alpha=false
sort_date=false
loop=false
include_videos=false
include_images=false
source_dir="."
audio_files=()
include_info=false
pause_audio=false

log() {
    if ${verbose}; then
        echo -e "${color_info}${script_name}:${color_reset} $*"
    fi
}

warn() {
    echo -e "${color_warn}${script_name}:${color_reset} $*"
}

error() {
    echo -e "${color_error}${script_name}:${color_reset} $*"
}

usage() {
    cat <<EOF
${color_info}${script_name}:${color_reset} Display a slideshow of images, GIFs, and optionally videos.
Usage: $script_name [options]
  -a           Sort images alphabetically (mutually exclusive with -r).
  -d <dir>     Specify the directory containing files (default: ${color_var}${source_dir}${color_reset}).
  -h           Display this help message.
  -i           Include file information in slideshow (via feh's --info option).
  -I           Include images.
  -l           Loop slideshow (reshuffles or resorts after completion).
  -m <files>   Comma-separated list of FLAC or MP3 files or directories. If directories are specified, adds all .flac and .mp3 files under those directories to the list.
  -n <sec>     Display time for each image (default: ${color_var}${img_time}${color_reset} seconds).
  -p           Pause background music during video playback (requires -m).
  -r           Randomize file order (mutually exclusive with -a, -x).
  -R           Recurse subdirectories when gathering image files.
  -t <seconds> Maximum playback time for videos (default: ${color_var}${video_duration}${color_reset} seconds).
  -v           Enable verbose mode.
  -V           Include video files in the slideshow.
  -x           Sort files by modification date (mutually exclusive with -r).
  -z           Reverse file order (requires -a or -x).
EOF
    exit 1
}

display_parsed_options() {
    log "Parsed options:"
    log "  Image time: ${color_var}${img_time}${color_reset} seconds"
    log "  Video duration: ${color_var}${video_duration}${color_reset} seconds"
    log "  Shuffle: ${color_var}${shuffle}${color_reset}"
    log "  Verbose mode: ${color_var}${verbose}${color_reset}"
    log "  Reverse order: ${color_var}${reverse}${color_reset}"
    log "  Sort alphabetically: ${color_var}${sort_alpha}${color_reset}"
    log "  Sort by date: ${color_var}${sort_date}${color_reset}"
    log "  Loop: ${color_var}${loop}${color_reset}"
    log "  Include videos: ${color_var}${include_videos}${color_reset}"
    log "  Include images: ${color_var}${include_images}${color_reset}"
    log "  Source directory: ${color_var}${source_dir}${color_reset}"
    log "  Audio files: ${color_var}${audio_files[*]}${color_reset}"
    log "  Include info: ${color_var}${include_info}${color_reset}"
    log "  Pause audio: ${color_var}${pause_audio}${color_reset}"

    log "Based on the provided options:"
    if ${include_images}; then
        log "  - Images will be included in the slideshow."
    fi
    if ${include_videos}; then
        log "  - Videos will be included in the slideshow."
    fi
    if ${sort_alpha}; then
        log "  - Files will be sorted alphabetically."
    elif ${sort_date}; then
        log "  - Files will be sorted by modification date."
    elif ${shuffle}; then
        log "  - Files will be randomized."
    fi
    if ${reverse}; then
        log "  - File order will be reversed."
    fi
    if ${loop}; then
        log "  - The slideshow will loop after completion."
    fi
    if [[ ${#audio_files[@]} -gt 0 ]]; then
        log "  - Background audio will play during the slideshow."
        if ${pause_audio}; then
            log "  - Audio playback will pause during video playback."
        fi
    fi
    log "Starting slideshow setup..."
}

declare -A seen_options
options_with_values="d m n t"

while getopts ":ad:hiIlm:n:prRt:vVxz" opt; do
    if [[ "${opt}" == ":" ]]; then
        error "Option -${OPTARG} requires an argument"
        exit 1
    fi

    # Check for duplicate options
    if [[ -n "${seen_options[${opt}]}" ]]; then
        error "Option -${opt} specified multiple times"
        exit 1
    fi
    seen_options[${opt}]=1  # Mark the option as seen

    # List of options that require values

    # Validate options that require values
    if [[ "${options_with_values}" == *"${opt}"* ]]; then
        # If OPTARG is empty or begins with a dash, it's invalid
        if [[ -z "${OPTARG}" || "${OPTARG}" == -* ]]; then
            error "Error: Missing or invalid value for -${opt} option"
            exit 1
        fi
    fi

    case ${opt} in
        a) sort_alpha=true ;;
        d) source_dir=$OPTARG ;;  
        h) usage ;;
        i) include_info=true ;;
        I) include_images=true ;;
        l) loop=true ;;
        m) IFS=',' read -r -a audio_files <<< "$OPTARG" ;;
        n) img_time=$OPTARG ;;
        p) pause_audio=true ;;
        r) shuffle=true ;;
        R) recurse=true ;;
        t) video_duration=$OPTARG ;;
        v) verbose=true ;;
        V) include_videos=true ;;
        x) sort_date=true ;;
        z) reverse=true ;;
        \?) warn "Invalid option: -$OPTARG" && usage ;;
    esac
done

if [[ "$include_images" != "true" && "$include_videos" != "true" ]]; then
    error "Either -I (include images) or -V (include videos) must be specified." >&2
    exit 1
fi

if ${pause_audio} && [[ ${#audio_files[@]} -eq 0 ]]; then
    error "The -p option requires background audio files to be specified with -m."
    exit
fi

if ${shuffle} && { ${sort_alpha} || ${sort_date}; }; then
    error "The -r (shuffle) option cannot be used with -a (sort alphabetically) or -x (sort by date)."
    exit
fi

if ${reverse} && ! { ${sort_alpha} || ${sort_date}; }; then
    error "The -z (reverse) option requires either -a (sort alphabetically) or -x (sort by date)."
    exit
fi

if [[ ! -d "${source_dir}" ]]; then
    error "The specified directory ${color_var}${source_dir}${color_reset} does not exist or is not a directory."
    exit
fi

if ${verbose}; then
    display_parsed_options
fi

log "Starting slideshow setup..."

for dir in "${audio_files[@]}"; do
    if [[ -d "$dir" ]]; then
        log "Adding audio files from directory: ${color_var}$dir${color_reset}"
        mapfile -t new_audio_files < <(find "$dir" -type f \( -iname "*.flac" -o -iname "*.mp3" \))
        audio_files+=("${new_audio_files[@]}")
    fi
done

cleanup() {
    log "Cleaning up and exiting..."
    stop_audio
    pkill -P $$  # Kill all child processes of the current script
    stty sane
    exit 0
}

validate_dependencies() {
    local missing_deps=()

    if ! command -v feh &> /dev/null; then
        missing_deps+=("feh")
    fi

    if ! command -v mpv &> /dev/null; then
        missing_deps+=("mpv")
    fi

    if [[ ${#missing_deps[@]} -gt 0 ]]; then
        log "Missing dependencies detected: ${color_var}${missing_deps[*]}${color_reset}"
        echo "To install the missing dependencies, run:"
        echo "  sudo apt update && sudo apt install -y ${missing_deps[*]}"
        exit 1
    fi
}

gather_files() {
    files=()
    local find_cmd="find \"${source_dir}\""

    if [[ "${recurse}" != "true" ]]; then
        find_cmd+=" -maxdepth 1"
    fi

    find_cmd+=" -type f \\( -iname "

    if [[ "${include_images}" == "true" ]]; then
        find_cmd+=" '*.jpg' -o -iname '*.jpeg' -o -iname '*.png' -o -iname '*.bmp' -o -iname '*.gif'"
    fi

    if [[ "${include_videos}" == "true" ]]; then
        if [[ "${include_images}" == "true" ]]; then
            find_cmd+=" -o -iname"
        fi

        find_cmd+=" '*.mp4' -o -iname '*.mkv' -o -iname '*.avi' -o -iname '*.webm'"
    fi

    find_cmd+=" \\)"
    log "Gathering files..."
    mapfile -t files < <(eval "${find_cmd}")

    if [[ ${#files[@]} -eq 0 ]]; then
        error "No relevant files found in ${source_dir}."
        exit 1
    fi
}

sort_and_shuffle_files() {
    if ${sort_alpha}; then
        mapfile -t files < <(printf "%s\n" "${files[@]}" | sort)
    elif ${sort_date}; then
        mapfile -t files < <(printf "%s\n" "${files[@]}" | xargs -I {} stat -c "%Y {}" {} | sort -rn | awk '{print $2}')
    fi

    if ${reverse}; then
        mapfile -t files < <(printf "%s\n" "${files[@]}" | tac)
    fi

    if ${shuffle}; then
        mapfile -t files < <(printf "%s\n" "${files[@]}" | shuf)
    fi
}

countdown_timer() {
    local duration="$1"
    while (( duration > 0 )); do
        echo -ne "${color_info}Time remaining: ${color_var}${duration}${color_reset} seconds...\r"
        sleep 1
        (( duration-- ))
    done
    echo -ne "\r${color_reset}                          \r"  # Clear the line
}

start_audio() {
    if [[ ${#audio_files[@]} -gt 0 ]]; then
        log "Starting background audio playback."
        mpv --loop=inf --no-video --audio-buffer=1.0 "${audio_files[@]}" &
        audio_pid=$!
    fi
}

stop_audio() {
    if [[ -n ${audio_pid} ]]; then
        log "Stopping background audio playback."
        kill "${audio_pid}" &>/dev/null
    fi
}

monitor_exit() {
    while :; do
        read -r -t 1 -n 1 key
        if [[ $key == "q" ]]; then
            cleanup
        fi
    done
}

run_slideshow() {
    log "Starting slideshow..."
    start_audio
    trap cleanup INT TERM 

    while :; do
        for file in "${files[@]}"; do
            if [[ "${file}" =~ \.(jpg|jpeg|png|bmp|gif)$ ]]; then
                local display_file="$file"
                log "Displaying image ${color_var}${display_file}${color_reset} for ${color_var}${img_time}${color_reset} second(s)."
                feh --auto-zoom --auto-rotate --fullscreen --borderless --image-bg black -D "${img_time}" --on-last-slide quit \
                    --info "bash -c 'realpath \"$file\"'" "$file" &
                feh_pid=$!
                countdown_timer "${img_time}"
                wait "${feh_pid}"
            elif [[ "${file}" =~ \.(mp4|mkv|avi|webm)$ ]]; then
                log "Playing video ${color_var}${file}${color_reset} for up to ${color_var}${video_duration}${color_reset} seconds."
                mpv_cmd="mpv --fs --audio-device=pulse --hwdec=auto-safe --msg-level=vo/gpu=warn --fs --hwdec=no --gpu-api=opengl"

                if [[ ${video_duration} -ne 0 ]]; then
                    mpv_cmd+=" --length=${video_duration}"
                fi

                if ${include_info}; then
                    subtitle_file=$(mktemp "/tmp/$(basename "${file%.*}").XXXXXX.srt")
                    echo "1" > "${subtitle_file}"  
                    echo "00:00:00,000 --> 99:59:59,999" >> "${subtitle_file}"  
                    realpath --relative-to="${source_dir}" "$file" >> "${subtitle_file}"  
                    mpv_cmd+=" --sub-file=\"${subtitle_file}\""
                    log "Subtitle file ${color_var}${subtitle_file}${color_reset} created."
                fi

                mpv_cmd+=" \"$(realpath "$file")\""
                if ${pause_audio} && [[ -n ${audio_pid} ]]; then
                    log "Pausing background music."
                    kill -SIGSTOP "${audio_pid}" 
                fi

                eval "$mpv_cmd" 

                if ${include_info}; then
                    rm -f "${subtitle_file}"
                fi

                if ${pause_audio} && [[ -n ${audio_pid} ]]; then
                    log "Resuming background music."
                    kill -SIGCONT "${audio_pid}" 
                fi
            fi
        done

        if ! ${loop}; then
            break
        fi

        log "Restarting slideshow..."
        sort_and_shuffle_files  # Re-sort or shuffle files as needed
    done
}

validate_dependencies
gather_files
sort_and_shuffle_files
log "Files to display: ${color_var}${files[*]}${color_reset}"
run_slideshow