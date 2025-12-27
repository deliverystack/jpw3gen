#!/bin/sh

# Check for required tools
MISSING_TOOLS=""
for tool in cargo rustfmt cargo-clippy xclip time; do
    if ! command -v $tool >/dev/null 2>&1; then
        MISSING_TOOLS="$MISSING_TOOLS $tool"
    fi
done

if [ -n "$MISSING_TOOLS" ]; then
    echo "ERROR: The following required tools are not installed:$MISSING_TOOLS" >&2
    exit 1
fi

# Global variables
EXIT_CODE=1

# prompt_user function
# Parameters:
#   $1 - prompt message
# Returns: 0 for yes, 1 for no
prompt_user() {
    local prompt="$1"
    printf "%s " "$prompt" >&2
    read -r answer
    case "$answer" in
        [Yy]|[Yy][Ee][Ss])
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

# execute_command function
# Parameters:
#   $1 - command (mandatory)
#   $2 - check_errors (1 = check for errors, 0 = ignore errors)
#   $3 - prompt_on_fail (1 = prompt on failure, 0 = auto-retry and exit)
execute_command() {
    local cmd="$1"
    local check_errors="${2:-1}"
    local prompt_on_fail="${3:-0}"
    
    # Always show the command
    echo "===================================" >&2
    echo "Executing: $cmd" >&2
    echo "===================================" >&2
    
    # Execute command with time - no capture on first run
    eval "time -p $cmd"
    local cmd_exit_code=$?
    
    # Only check for errors if requested
    if [ "$check_errors" -eq 1 ]; then
        # Check for errors (non-zero exit code only)
        if [ "$cmd_exit_code" -ne 0 ]; then
            echo "" >&2
            echo "!!! COMMAND FAILED !!!" >&2
            echo "Failed command: $cmd" >&2
            echo "Exit code: $cmd_exit_code" >&2
            echo "" >&2
            
            if [ "$prompt_on_fail" -eq 1 ]; then
                # Prompt for clipboard and continuation
                local capture_clipboard=0
                if prompt_user "Retry with clipboard capture? (y/n)"; then
                    capture_clipboard=1
                    echo "Retrying WITH clipboard capture..." >&2
                else
                    echo "Retrying WITHOUT clipboard capture..." >&2
                fi
                
                # Retry the command
                echo "===================================" >&2
                echo "RETRY: $cmd" >&2
                echo "===================================" >&2
                
                if [ "$capture_clipboard" -eq 1 ]; then
                    eval "time -p $cmd" 2>&1 | xclip -selection clipboard
                    local retry_exit_code=$?
                    echo "Output captured to clipboard." >&2
                else
                    eval "time -p $cmd"
                    local retry_exit_code=$?
                fi
                
                # Check retry result
                if [ "$retry_exit_code" -ne 0 ]; then
                    echo "" >&2
                    echo "!!! RETRY ALSO FAILED !!!" >&2
                    echo "Failed command: $cmd" >&2
                    echo "Retry exit code: $retry_exit_code" >&2
                    echo "" >&2
                else
                    echo "" >&2
                    echo "Retry succeeded!" >&2
                    echo "" >&2
                fi
                
                # Ask whether to continue
                if ! prompt_user "Continue with script? (y/n)"; then
                    echo "Script terminated by user." >&2
                    exit $EXIT_CODE
                fi
            else
                # Auto-retry once with clipboard, then exit
                echo "Retrying WITH clipboard capture..." >&2
                echo "===================================" >&2
                echo "RETRY: $cmd" >&2
                echo "===================================" >&2
                
                eval "time -p $cmd" 2>&1 | xclip -selection clipboard
                local retry_exit_code=$?
                echo "Output captured to clipboard." >&2
                
                if [ "$retry_exit_code" -ne 0 ]; then
                    echo "" >&2
                    echo "!!! RETRY ALSO FAILED !!!" >&2
                    echo "Script terminated." >&2
                    exit $EXIT_CODE
                else
                    echo "" >&2
                    echo "Retry succeeded!" >&2
                    echo "" >&2
                fi
            fi
            
            return 1
        fi
    fi
    
    echo "Command succeeded." >&2
    echo "" >&2
    return 0
}

# Setup environment
GEN=$HOME/git/jpw3gen
SOURCE=$HOME/git/jpw3
TARGET=$HOME/git/vercel
export CARGO_TARGET_DIR=/tmp/cargo
export RUST_BACKTRACE=full

echo "===================================" >&2
echo "Starting build script" >&2
echo "GEN: $GEN" >&2
echo "SOURCE: $SOURCE" >&2
echo "TARGET: $TARGET" >&2
echo "CARGO_TARGET_DIR: $CARGO_TARGET_DIR" >&2
echo "===================================" >&2
echo "" >&2

cd $GEN

rm -rf "$TARGET"/*

# cargo update - don't check for errors (warnings are ok)
execute_command "cargo update -v" 0 0
EXIT_CODE=$((EXIT_CODE + 1))

# cargo check (first attempt) - check errors, no prompt
execute_command "cargo check -v --workspace --all-features" 1 0
check_result=$?
EXIT_CODE=$((EXIT_CODE + 1))

# cargo check retry if needed
if [ $check_result -ne 0 ]; then
    execute_command "cargo check -v --workspace --all-features" 1 0
    EXIT_CODE=$((EXIT_CODE + 1))
fi

# rustfmt - don't check for errors
execute_command "rustfmt -l -v ./src/*.rs" 0 0
EXIT_CODE=$((EXIT_CODE + 1))

# List Rust files
echo "===================================" >&2
echo "Listing Rust files" >&2
echo "===================================" >&2
for f in $(find . -iname \*.rs -print); do
    ls -l $f
    wc -clw $f
done
echo "" >&2

# cargo-clippy - don't check for errors (warnings are informational)
execute_command "cargo-clippy -v" 0 0
EXIT_CODE=$((EXIT_CODE + 1))

# cargo build - check errors WITH prompting
execute_command "cargo build -v" 1 1
EXIT_CODE=$((EXIT_CODE + 1))

# Run generator - check errors, no prompt
execute_command "$CARGO_TARGET_DIR/debug/jpw3gen --source $SOURCE --target $TARGET" 1 0
EXIT_CODE=$((EXIT_CODE + 1))

# Copy files - check errors, no prompt
execute_command "cp $SOURCE/styles.css $SOURCE/favicon.ico $TARGET" 1 0
EXIT_CODE=$((EXIT_CODE + 1))

execute_command "cp $SOURCE/template.html $SOURCE/styles.css $GEN" 1 0
EXIT_CODE=$((EXIT_CODE + 1))

# Ask about git deployment
echo "" >&2
if prompt_user "Run git deployment commands? (y/n)"; then
    cd $TARGET
    
    execute_command "git checkout --orphan latest_branch" 1 0
    EXIT_CODE=$((EXIT_CODE + 1))
    
    execute_command "git add -A" 1 0
    EXIT_CODE=$((EXIT_CODE + 1))
    
    execute_command "git commit -am 'history of generated files truncated to save storage space'" 1 0
    EXIT_CODE=$((EXIT_CODE + 1))
    
    # Check if main branch exists before trying to delete it
    if git show-ref --verify --quiet refs/heads/main; then
        execute_command "git branch -D main" 1 0
        EXIT_CODE=$((EXIT_CODE + 1))
    fi
    
    execute_command "git branch -m main" 1 0
    EXIT_CODE=$((EXIT_CODE + 1))
    
    execute_command "git push -f origin main" 1 0
    EXIT_CODE=$((EXIT_CODE + 1))
    
    echo "===================================" >&2
    echo "Git deployment completed" >&2
    echo "===================================" >&2
else
    echo "Skipping git deployment." >&2
fi

echo "" >&2
echo "===================================" >&2
echo "Build script completed successfully" >&2
echo "===================================" >&2