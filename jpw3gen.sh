#!/bin/sh

# show commands
set -x

# exit on error
set -e

CLIPBOARD=0
CLEANUP=0

while getopts "cd" opt; do
  case $opt in
    c)
      CLIPBOARD=1
      ;;
    d)
      CLEANUP=1
      ;;
    \?)
      echo "Invalid option: -$OPTARG" >&2
      exit 1
      ;;
  esac
done

if [ "$CLIPBOARD" -eq 1 ] && [ "$CLEANUP" -eq 1 ]; then
    echo "ERROR: Options -c and -d cannot be used together." >&2
    exit 2 # Use a unique exit code for this specific error
fi

GEN=/home/jw/git/jpw3gen
SOURCE=/home/jw/git/jpw3
TARGET=/home/jw/git/vercel
export CARGO_TARGET_DIR=/tmp/cargo

rm -rf "$TARGET"/*

cd "$GEN"

if [ $CLIPBOARD -eq 1 ]; then
    cargo build 2>&1 | tee >(xclip -selection clipboard)
else
    cargo build
fi

GEN_CMD="$CARGO_TARGET_DIR/debug/jpw3gen --source $SOURCE --target $TARGET"
$GEN_CMD # --verbose 

if [ $CLEANUP -eq 1 ]; then
    $GEN/jpw3hist.sh 
fi
