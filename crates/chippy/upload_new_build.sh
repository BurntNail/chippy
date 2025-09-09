#!/bin/bash
set -euxo pipefail

trunk build --release --minify --filehash false


rm -v $HOME/Documents/Markdown/katashift/static/wasms/chippy/chippy.js
rm -v $HOME/Documents/Markdown/katashift/static/wasms/chippy/chippy_bg.wasm
cp -v dist/chippy.js $HOME/Documents/Markdown/katashift/static/wasms/chippy/
cp -v dist/chippy_bg.wasm $HOME/Documents/Markdown/katashift/static/wasms/chippy/

cd $HOME/Documents/Markdown/katashift/
zola build --minify
RUST_LOG=info shove upload public
