#!/bin/bash
script_name=$(basename "$0")
set -u

log_output="both"  # Options: screen, file, both
log_file="$HOME/${script_name}.log"
debug=true  # Debug mode off by default
verbose=true  # Verbose mode off by default

red_color=$(tput setaf 1)
green_color=$(tput setaf 2)
orange_color=$(tput setaf 3)
yellow_color=$(tput setaf 3)
cyan_color=$(tput setaf 6)
reset_color=$(tput sgr0)

red() { printf "${red_color}%s${reset_color}" "$1"; }
green() { printf "${green_color}%s${reset_color}" "$1"; }
orange() { printf "${orange_color}%s${reset_color}" "$1"; }
yellow() { printf "${yellow_color}%s${reset_color}" "$1"; }
cyan() { printf "${cyan_color}%s${reset_color}" "$1"; }

log_message() {
    local level="$1"
    local level_color="$2"
    shift 2 
    local timestamp
    timestamp=$(date "+%Y-%m-%d %H:%M:%S")

    for message in "$@"; do
        local entry="${level_color}${script_name}: ${level}: ${reset_color}${message}"

        if [[ "$log_output" == "screen" || "$log_output" == "both" ]]; then
            printf "%s\n" "$entry" > /dev/tty
        fi

        if [[ "$log_output" == "file" || "$log_output" == "both" ]]; then
            printf "%s %s\n" "$timestamp" "$entry" >> "$log_file"
        fi
    done
}

info() {
    local level="INFO"
    local color="$green_color"
    log_message "$level" "$color" "$@"
}

warn() {
    local level="WARN"
    local color="$orange_color"
    log_message "$level" "$color" "$@"
}

error() {
    local level="ERROR"
    local color="$red_color"
    log_message "$level" "$color" "$@"
}

progress() {
    local level="PROGRESS"
    local color="$cyan_color"
    log_message "$level" "$color" "$@"
}

debug() {
    local level="DEBUG"
    local color="$yellow_color"
    [[ "$debug" == true ]] && log_message "$level" "$color" "$@"
}

sanitize_name_part() {
    local part="$1"
    echo "$part" | sed -E "
        s|[^a-zA-Z0-9.-]|-|g;  # Replace invalid characters
        s|-{2,}|-|g;           # Remove repeated dashes
        s|^-||g;               # Remove leading dashes
        s|-$||g;               # Remove trailing dashes
    " | tr '[:upper:]' '[:lower:]'
}

validate_dependencies() {
    local exit_on_fail=0
    local dependencies=()
    local missing_deps=()

    # Parse options
    while getopts ":x" opt; do
        case ${opt} in
            x)
                exit_on_fail=1
                ;;
            \?)
                error "Invalid option: -${OPTARG}"
                return 1
                ;;
        esac
    done
    shift $((OPTIND -1))
    
    # Remaining arguments are dependencies
    dependencies=("$@")

    # Check each dependency
    for dep in "${dependencies[@]}"; do
        if ! command -v "${dep}" &> /dev/null; then
            missing_deps+=("${dep}")
        fi
    done

    if [[ ${#missing_deps[@]} -gt 0 ]]; then
        warn "Missing dependencies detected: ${missing_deps[*]}"
        warn "To install the missing dependencies, try:"
        warn "  sudo apt update && sudo apt install -y ${missing_deps[*]}"
        
        if [[ ${exit_on_fail} -eq 1 ]]; then
            exit 1
        else
            return 1
        fi
    fi

    return 0
}

clean_name() {
    local original="$1"
    local base
    local ext=""
    local cleaned_base
    local cleaned_ext

    debug "Processing name: ${original}"

    # Split the name into base and extension
    base="${original%.*}"
    ext="${original##*.}"

    # Handle cases where there's no extension
    if [[ "$base" == "$original" ]]; then
        ext=""
    fi
    debug "Split name into base: '${base}', extension: '${ext}'"

    # Sanitize the base and extension
    cleaned_base=$(sanitize_name_part "$base")
    if [[ -n "$ext" ]]; then
        cleaned_ext=$(sanitize_name_part "$ext")
    else
        cleaned_ext=""
    fi

    # Combine base and extension
    if [[ -n "$cleaned_ext" ]]; then
        cleaned_base="${cleaned_base}.${cleaned_ext}"
    fi

    # Handle edge cases for short or invalid names
    if [[ -z "$cleaned_base" || "$cleaned_base" =~ ^-+$ ]]; then
        cleaned_base="file-${RANDOM}"
    fi

    debug "Sanitized name: ${cleaned_base}"
    echo "$cleaned_base"
}

run_command() {
    local args=("$@")
    local output_file=""
    local debug=false
    local verbose=false
    local run_flags=()
    local use_time=false

    local exit_on_error=false
    local calling_script
    local result

    # Function for handling errors with exit logic
    handle_error() {
        local message="$1"
        error "$message"
        if $exit_on_error; then
            error "Exiting script due to -x/--exit-on-error"
            exit 1
        else
            return 1
        fi
    }

    while [[ "$#" -gt 0 ]]; do
        case "$1" in
            -o|--output-file)
                if [[ -z "$2" || "$2" == -* ]]; then
                    handle_error "The -o/--output-file flag requires a valid file name as an argument." || return 1
                fi
                output_file="$2"
                shift
                ;;
            -t|--time)
                use_time=true
                ;;
            -d|--debug)
                debug=true
                ;;
            -v|--verbose)
                verbose=true
                ;;
            -x|--exit-on-error)
                exit_on_error=true
                ;;
            --) shift; break ;;  # End of run_command flags; break the loop
            -*)
                handle_error "Invalid or unsupported flag: $1" || return 1
                ;;
        esac
        shift
    done

    debug "Received ${cyan_color}${args[*]}${reset_color}"
    cmd=("$@")

    # Validate the presence of a command
    if [[ ${#cmd[@]} -eq 0 ]]; then
        handle_error "No command provided to run_command" || return 1
    fi

    # Validate -t usage
    if $use_time && [[ -z "$output_file" && $verbose == false ]]; then
        handle_error "The -t/--time flag requires output to be directed to the screen or a file." || return 1
    fi

    calling_script="$(realpath "${BASH_SOURCE[1]}")"
    info "${cyan_color}$calling_script${reset_color} invoking ${cyan_color}${cmd[*]}${reset_color}"

    # Prefix the command with 'time' if -t is specified
    if $use_time; then
        cmd=("time" "${cmd[@]}")
    fi

    if [[ -n "$output_file" ]]; then
        debug "Saving command output to ${cyan_color}$output_file${reset_color}"
        if $use_time; then
            { time "${cmd[@]}" > >(tee -a "$output_file") 2>&1; } 2> >(tee -a "$output_file" >&2)
        else
            "${cmd[@]}" 2>&1 | tee -- "$output_file"
        fi
    else
        if $verbose; then
            debug "No output file specified; command output will be displayed on screen."
            if $use_time; then
                time "${cmd[@]}"
            else
                "${cmd[@]}"
            fi
        else
            debug "No output file specified; command output will not be displayed."
            if $use_time; then
                time "${cmd[@]}" &>/dev/null
            else
                "${cmd[@]}" &>/dev/null
            fi
        fi
    fi
    
    result=${PIPESTATUS[0]}

    # Handle command execution errors
    if [[ "$result" -ne 0 ]]; then
        handle_error "Command failed with exit code $result" || return 1
    fi

    return "$result"
}

self_check() {
    local calling_script
    calling_script=$(realpath "${BASH_SOURCE[0]}")

    # Create a temporary file for ShellCheck output
    local temp_file
    temp_file=$(mktemp)

    # Execute ShellCheck and capture output in the temporary file
    run_command -o "$temp_file" -x -t -d -v shellcheck "$calling_script"
    local result=$?

    # Check the temp file for errors or warnings
    if [[ "$result" -ne 0 || $(grep -qiE 'error|warning' "$temp_file"; echo $?) -eq 0 ]]; then
        warn "ShellCheck detected issues:"
        cat "$temp_file"  # Display the output
    else
        info "ShellCheck passed successfully with no warnings or errors."
    fi

    # Clean up the temporary file
    rm -f "$temp_file"
}

test() {
    if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
        debug=true 
        verbose=true
        info "Starting script self-check using ShellCheck..."
        self_check

        # Prepare flags for run_command
    flags=()
    [[ "$debug" == true ]] && flags+=("-d")
    [[ "$verbose" == true ]] && flags+=("-v")
        # Execute invalid command
        run_command "${flags[@]}" --invalid
        result=$?

        if [[ "$result" -ne 0 ]]; then
            error "Invalid command failed as expected (Exit code: $result)."
        fi

        # Execute command with a nonexistent directory
        run_command "${flags[@]}" ls /nonexistent_directory
        result=$?

        if [[ "$result" -ne 0 ]]; then
            error "Command failed as expected (Exit code: $result)."
        fi

        info "Sanitizing a sample filename..."
        sanitized_name=$(clean_name " Example--File!@Name  ")
        info "Sanitized filename: $(cyan "${sanitized_name}")"
    fi
}