#!/bin/sh

# Check for required tools
MISSING_TOOLS=""
for tool in cargo rustfmt cargo-clippy; do
    if ! command -v $tool >/dev/null 2>&1; then
        MISSING_TOOLS="$MISSING_TOOLS $tool"
    fi
done

if [ -n "$MISSING_TOOLS" ]; then
    echo "ERROR: The following required tools are not installed:$MISSING_TOOLS" >&2
    exit 1
fi

# Global variables
CLIPBOARD=0
DEPLOY=0
VERBOSE=0
TIME=0
EXIT_CODE=1

# Parse command line options
while getopts "cdvt" opt; do
  case $opt in
    c)
      CLIPBOARD=1
      ;;
    d)
      DEPLOY=1
      ;;
    v)
      VERBOSE=1
      ;;
    t)
      TIME=1
      ;;
    \?)
      echo "Invalid option: -$OPTARG" >&2
      exit 1
      ;;
  esac
done

if [ "$CLIPBOARD" -eq 1 ] && [ "$DEPLOY" -eq 1 ]; then
    echo "ERROR: Options -c and -d cannot be used together." >&2
    exit 2
fi

# execute_command function
# Parameters:
#   $1 - command (mandatory)
#   $2 - use_time (optional, default: 0)
#   $3 - exit_on_error (optional, default: 1)
#   $4 - capture_clipboard (optional, default: 0)
#   $5 - show_command (optional, default: 0)
#   $6 - additional_error_patterns (optional, default: "")
execute_command() {
    local cmd="$1"
    local use_time="${2:-0}"
    local exit_on_error="${3:-1}"
    local capture_clipboard="${4:-0}"
    local show_command="${5:-0}"
    local additional_patterns="$6"
    
    # Determine if we should show the command
    local should_show=$show_command
    
    # Build the full command with time if requested
    local full_cmd="$cmd"
    if [ "$use_time" -eq 1 ]; then
        full_cmd="time -p $cmd"
    fi
    
    # Show command if requested
    if [ "$should_show" -eq 1 ]; then
        echo "+ $full_cmd" >&2
    fi
    
    # Build error pattern
    local error_pattern="error|warning"
    if [ -n "$additional_patterns" ]; then
        error_pattern="$error_pattern|$additional_patterns"
    fi
    
    # Execute command
    local result
    local tmpfile=$(mktemp)
    
    if [ "$use_time" -eq 1 ]; then
        # When timing, combine stdout and stderr
        if [ "$capture_clipboard" -eq 1 ]; then
            eval $full_cmd 2>&1 | tee "$tmpfile" | xclip -selection clipboard
        else
            eval $full_cmd 2>&1 | tee "$tmpfile"
        fi
    else
        # Without timing, capture stderr separately for error checking
        if [ "$capture_clipboard" -eq 1 ]; then
            eval $full_cmd 3>&1 1>&2 2>&3 | tee "$tmpfile" | xclip -selection clipboard
        else
            eval $full_cmd 3>&1 1>&2 2>&3 | tee "$tmpfile"
        fi
    fi
    
    local cmd_exit_code=$?
    result=$(cat "$tmpfile")
    rm -f "$tmpfile"
    
    # Sanitize output by removing known rustc patterns that contain error/warning keywords
    # Use parameter expansion to avoid sed issues with special characters
    local sanitized_result="$result"
    sanitized_result="${sanitized_result//--warn=/--WARN=}"
    sanitized_result="${sanitized_result//--WARN=/--XWARN=}"
    sanitized_result="${sanitized_result//--allow=/--ALLOW=}"
    sanitized_result="${sanitized_result//\'--warn=/\'--WARN=}"
    sanitized_result="${sanitized_result//\'--WARN=/\'--XWARN=}"
    sanitized_result="${sanitized_result//clippy::/CLIPPY::}"
    sanitized_result="${sanitized_result//warning:/WARNING:}"
    sanitized_result="${sanitized_result//WARN=/XWARN=}"
    
    # Check for errors
    if [ "$cmd_exit_code" -ne 0 ] || echo "$sanitized_result" | grep -qiE "$error_pattern"; then
        if [ "$exit_on_error" -eq 1 ]; then
            if [ "$cmd_exit_code" -ne 0 ]; then
                echo "Command failed with exit code $cmd_exit_code: $cmd" >&2
            else
                echo "Command failed with error/warning in output: $cmd" >&2
                echo "$result" | grep -iE "$error_pattern" >&2
            fi
            echo "To reproduce: cd $PWD ; $cmd" >&2
            cd - > /dev/null
            exit $EXIT_CODE
        fi
        return 1
    fi
    
    return 0
}

# Setup environment
GEN=$HOME/git/jpw3gen
SOURCE=$HOME/git/jpw3
TARGET=$HOME/git/vercel
export CARGO_TARGET_DIR=/tmp/cargo
export RUST_BACKTRACE=full
cd $GEN

rm -rf "$TARGET"/*

# Set up execution flags based on -v and -t
USE_TIME=$TIME
EXIT_ON_ERROR=1
CAPTURE_TO_CLIPBOARD=0
SHOW_COMMAND=$VERBOSE
ADDITIONAL_ERROR_PATTERNS=""

# cargo update
execute_command "cargo update -v" "$USE_TIME" "$EXIT_ON_ERROR" "$CAPTURE_TO_CLIPBOARD" "$SHOW_COMMAND" "$ADDITIONAL_ERROR_PATTERNS"
EXIT_CODE=$((EXIT_CODE + 1))

# cargo check (first attempt)
execute_command "cargo check -v --workspace --all-features" "$USE_TIME" "$EXIT_ON_ERROR" "$CAPTURE_TO_CLIPBOARD" "$SHOW_COMMAND" "$ADDITIONAL_ERROR_PATTERNS"
check_result=$?
EXIT_CODE=$((EXIT_CODE + 1))

# cargo check retry if needed
if [ $check_result -ne 0 ]; then
    execute_command "cargo check -v --workspace --all-features" "$USE_TIME" "$EXIT_ON_ERROR" "$CAPTURE_TO_CLIPBOARD" "$SHOW_COMMAND" "$ADDITIONAL_ERROR_PATTERNS"
    EXIT_CODE=$((EXIT_CODE + 1))
fi

# rustfmt
execute_command "rustfmt -l -v ./src/*.rs" "$USE_TIME" "$EXIT_ON_ERROR" "$CAPTURE_TO_CLIPBOARD" "$SHOW_COMMAND" "$ADDITIONAL_ERROR_PATTERNS"
EXIT_CODE=$((EXIT_CODE + 1))

# List Rust files
for f in $(find . -iname \*.rs -print); do
    ls -l $f
    wc -clw $f
done

# cargo-clippy (don't exit on error)
EXIT_ON_ERROR=0
execute_command "cargo-clippy -v" "$USE_TIME" "$EXIT_ON_ERROR" "$CAPTURE_TO_CLIPBOARD" "$SHOW_COMMAND" "$ADDITIONAL_ERROR_PATTERNS"
EXIT_CODE=$((EXIT_CODE + 1))

# cargo build
EXIT_ON_ERROR=1
if [ "$CLIPBOARD" -eq 1 ]; then
    CAPTURE_TO_CLIPBOARD=1
fi
execute_command "cargo build -v" "$USE_TIME" "$EXIT_ON_ERROR" "$CAPTURE_TO_CLIPBOARD" "$SHOW_COMMAND" "$ADDITIONAL_ERROR_PATTERNS"
EXIT_CODE=$((EXIT_CODE + 1))

# Run generator
execute_command "$CARGO_TARGET_DIR/debug/jpw3gen --source $SOURCE --target $TARGET" "$USE_TIME" "$EXIT_ON_ERROR" "$CAPTURE_TO_CLIPBOARD" "$SHOW_COMMAND" "$ADDITIONAL_ERROR_PATTERNS"
EXIT_CODE=$((EXIT_CODE + 1))

# Copy files
execute_command "cp $SOURCE/styles.css $SOURCE/favicon.ico $TARGET" "$USE_TIME" "$EXIT_ON_ERROR" "$CAPTURE_TO_CLIPBOARD" "$SHOW_COMMAND" "$ADDITIONAL_ERROR_PATTERNS"
EXIT_CODE=$((EXIT_CODE + 1))

execute_command "cp $SOURCE/template.html $SOURCE/styles.css $GEN" "$USE_TIME" "$EXIT_ON_ERROR" "$CAPTURE_TO_CLIPBOARD" "$SHOW_COMMAND" "$ADDITIONAL_ERROR_PATTERNS"
EXIT_CODE=$((EXIT_CODE + 1))

# Deploy/cleanup if requested
if [ $DEPLOY -eq 1 ]; then
    cd $TARGET
    execute_command "git checkout --orphan latest_branch" "$USE_TIME" "$EXIT_ON_ERROR" "$CAPTURE_TO_CLIPBOARD" "$SHOW_COMMAND" "$ADDITIONAL_ERROR_PATTERNS"
    EXIT_CODE=$((EXIT_CODE + 1))
    
    execute_command "git add -A" "$USE_TIME" "$EXIT_ON_ERROR" "$CAPTURE_TO_CLIPBOARD" "$SHOW_COMMAND" "$ADDITIONAL_ERROR_PATTERNS"
    EXIT_CODE=$((EXIT_CODE + 1))
    
    execute_command "git commit -am 'history of generated files truncated to save storage space'" "$USE_TIME" "$EXIT_ON_ERROR" "$CAPTURE_TO_CLIPBOARD" "$SHOW_COMMAND" "$ADDITIONAL_ERROR_PATTERNS"
    EXIT_CODE=$((EXIT_CODE + 1))
    
    # Check if main branch exists before trying to delete it
    if git show-ref --verify --quiet refs/heads/main; then
        execute_command "git branch -D main" "$USE_TIME" "$EXIT_ON_ERROR" "$CAPTURE_TO_CLIPBOARD" "$SHOW_COMMAND" "$ADDITIONAL_ERROR_PATTERNS"
        EXIT_CODE=$((EXIT_CODE + 1))
    fi
    
    execute_command "git branch -m main" "$USE_TIME" "$EXIT_ON_ERROR" "$CAPTURE_TO_CLIPBOARD" "$SHOW_COMMAND" "$ADDITIONAL_ERROR_PATTERNS"
    EXIT_CODE=$((EXIT_CODE + 1))
    
    execute_command "git push -f origin main" "$USE_TIME" "$EXIT_ON_ERROR" "$CAPTURE_TO_CLIPBOARD" "$SHOW_COMMAND" "$ADDITIONAL_ERROR_PATTERNS"
    EXIT_CODE=$((EXIT_CODE + 1))
fi