#!/bin/bash

rm -r docs

cd fishnet
cargo clean --doc
RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc -p fishnet --lib --no-deps --all-features
cp -r $CARGO_TARGET_DIR/doc ../docs
echo "<meta http-equiv=\"refresh\" content=\"0; url=fishnet/index.html\">" > ../docs/index.html

