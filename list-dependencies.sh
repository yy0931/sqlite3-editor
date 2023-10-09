#!/bin/bash
cargo license --authors --do-not-bundle | sed '/^db-driver-rs:/d' > vendor.LICENSE.rust.txt
