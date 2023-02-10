#!/usr/bin/bash

mkdir -p target/near/near_messenger
pushd contract
cargo near build --no-abi
popd
