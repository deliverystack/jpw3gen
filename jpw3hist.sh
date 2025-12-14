#!/bin/sh

# show commands
set -x

# exit on error
set -e

cd /home/jw/git/vercel
git checkout --orphan latest_branch
git add -A
git commit -am "history of generated files truncated to save storage space"
git branch -D main
git branch -m main
git push -f origin main

# exclude favicon