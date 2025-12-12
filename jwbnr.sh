#!/bin/sh

# show commands
set -x

# exit on error
set -e


SRC=/home/jw/git/jpw3gen
SOURCE=/home/jw/git/jpw3
TARGET=/home/jw/git/vercel
export CARGO_TARGET_DIR=/tmp/cargo
rm -r $TARGET/*
cd $SRC
cargo build
#read -p 'Enter to continue; CTRL+C to quit'
/tmp/cargo/debug/jpw3gen --source $SOURCE --target $TARGET
#read -p 'Enter to continue; CTRL+C to quit'
#kill $(lsof -t -i :8000) 2> /dev/null
#python3 -m http.server 8000 --directory $TARGET &
#xdg-open http://localhost:8000