#!/bin/sh

# interactive delete all non-hidden files 
# with a given extension
# from non-hidden directories 

# find . -path '*/.*' -prune -o -name "*.THM" -type f -exec rm -i {} +
# find . -path '*/.*' -prune -o -name "*Thumbs.db" -type f -exec rm {} +

find . -path '*/.*' -prune -o -type f -print0 |
awk -v RS='\0' '
{
    path = $0
    name = path
    sub(/^.*\//, "", name)

    if (name ~ /\./) {
        sub(/^.*\./, "", name)
        ext[name]++
    } else {
        noext[++n] = path
    }
}
END {
    for (e in ext)
        printf "%d\t%s\n", ext[e], e
    print "---"
    for (i = 1; i <= n; i++)
        print noext[i]
}' |
awk '
$0=="---"{mode=1; next}
mode==0{print | "sort -nr"}
mode==1{print}
'
