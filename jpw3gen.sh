#!/bin/sh

# show commands
#set -x

# exit on error
#set -e

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
export RUST_BACKTRACE=full  # or 1
cd $GEN                     # avoid specifying project path for all commands

rm -rf "$TARGET"/*

cd "$GEN"

cmd="cargo update -v "
result=$( time $cmd 3>&1 1>&2 2>&3 | tee /dev/fd/2 )

if [ "$?" -ne "0" ] || echo $result | grep -qiE "error|warning"; then
    echo cargo update -v failed
    echo To reproduce: cd `pwd` \; $cmd # and look for error/warning
    cd - > /dev/null # revert to the previous directory
    exit 3
fi

cmd="time cargo check -v --workspace --all-features"
result=$( $cmd 3>&1 1>&2 2>&3 | tee /dev/fd/2 )

if [ "$?" -ne "0" ]; then
    echo cargo check failed with non-zero exit code
    echo To reproduce: cd `pwd` \; $cmd
    cd - > /dev/null # revert to the previous directory
    exit 2
fi


# sometimes cargo check fails the first time; in which case, run it again.

if echo $result | grep -qiE "error|warning"; then
    result=$( time $cmd 3>&1 1>&2 2>&3 | tee /dev/fd/2 )
    
    if [ "$?" -ne "0" ]; then
        echo cargo check failed the second run with a non-zero exit code
        echo To reproduce: cd `pwd` \; $cmd # and look for error/warning
        cd - > /dev/null # revert to the previous directory
        exit 2
    fi

    if echo $result | grep -qiE "error|warning"; then
        echo cargo check failed the second run with an error or warning
        echo $result | grep -iE "error|warning"
        echo To reproduce: cd `pwd` \; $cmd # and look for error/warning
        cd - > /dev/null # revert to the previous directory
        exit 2
    fi
fi

cmd="time rustfmt -l -v ./src/*.rs"
result=$( time $cmd 3>&1 1>&2 2>&3 | tee /dev/fd/2 )

if [ "$?" -ne "0" ] || echo $result | grep -Eqi "error|warning"; then
    echo rustfmt all failed
    echo To reproduce: cd `pwd` \; $cmd # and look for error/warning
    cd - > /dev/null # revert to the previous directory
    exit 3
fi

for f in "$(find . -iname \*.rs -print)"; do
    ls -l $f
    wc -clw $f
done

cmd='time cargo-clippy -v'
result=$( $cmd 3>&1 1>&2 2>&3 | tee /dev/fd/2 )

if [ "$?" -ne "0" ] || echo $result | grep -qiE "error|warning"; then
    echo cargo-clippy failed
    echo To reproduce: cd `pwd` ; $cmd
    cd - > /dev/null # revert to the previous directory
#    exit 4
fi






#exit 0

# show commands
#set -x

# exit on error
#set -e

set -e

# add --release but that will build to a different path so invocation will be wrong...

cmd="time cargo build -v"

if [ "$CLIPBOARD" -eq 1 ]; then
    result=$($cmd 3>&1 1>&2 2>&3 | tee /dev/fd/2 >(xclip -selection clipboard))
else
    result=$($cmd 3>&1 1>&2 2>&3 | tee /dev/fd/2)
fi

#if [ "$?" -ne "0" ] || echo "$result" | sed -e s'/error.format/3rror.format/g' | grep -qiE "error|warning"; then
#    echo cargo build failed
#    echo "To reproduce: cd `pwd` \; $cmd"
#    cd - > /dev/null 
#    exit 6
#fi





#if [ $CLIPBOARD -eq 1 ]; then
#    cargo build 2>&1 | tee >(xclip -selection clipboard)
#else
#    cargo build
#fi

GEN_CMD="$CARGO_TARGET_DIR/debug/jpw3gen --source $SOURCE --target $TARGET"
$GEN_CMD # --verbose 
cp $SOURCE/styles.css $SOURCE/favicon.ico $TARGET
cp $SOURCE/template.html $SOURCE/styles.css $GEN

if [ $CLEANUP -eq 1 ]; then
    cd $TARGET
    git checkout --orphan latest_branch
    git add -A
    git commit -am "history of generated files truncated to save storage space"
    git branch -D main
    git branch -m main
    git push -f origin main
fi
