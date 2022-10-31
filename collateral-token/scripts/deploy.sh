#!/bin/bash

cd ..

CONTRACT_FILE=./res/collateral_token.wasm

near dev-deploy --wasmFile $CONTRACT_FILE