#!/bin/sh
# Downloads the CommonMark and GFM specs beside this script, each only if it
# is not already there. The conformance test and both benches read them.
#
#   crates/markdown/tests/spec/fetch.sh
set -eu
here=$(cd "$(dirname "$0")" && pwd)

fetch() {
  [ -s "$here/$1" ] && return
  curl --fail --silent --show-error --location "$2" --output "$here/$1.part"
  mv "$here/$1.part" "$here/$1"
}

fetch commonmark.txt https://raw.githubusercontent.com/commonmark/commonmark-spec/0.31.2/spec.txt
fetch gfm.txt https://raw.githubusercontent.com/github/cmark-gfm/0.29.0.gfm.13/test/spec.txt
