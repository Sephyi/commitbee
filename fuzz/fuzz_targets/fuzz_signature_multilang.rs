// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

#![no_main]

use libfuzzer_sys::fuzz_target;

// Dispatches the remaining input to a language-specific signature extractor
// based on the first byte (`data[0] % 10`). Each `extract_*_signature`
// function must never panic on any input — this fuzzer only asserts that.
//
// Language map (matches CommitBee's supported grammars):
//   0 -> Rust
//   1 -> TypeScript
//   2 -> JavaScript
//   3 -> Python
//   4 -> Go
//   5 -> Java
//   6 -> C
//   7 -> C++
//   8 -> Ruby
//   9 -> C#
fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let selector = data[0] % 10;
    let Ok(source) = std::str::from_utf8(&data[1..]) else {
        return;
    };
    match selector {
        0 => {
            let _ = commitbee::extract_rust_signature(source);
        }
        1 => {
            let _ = commitbee::extract_typescript_signature(source);
        }
        2 => {
            let _ = commitbee::extract_javascript_signature(source);
        }
        3 => {
            let _ = commitbee::extract_python_signature(source);
        }
        4 => {
            let _ = commitbee::extract_go_signature(source);
        }
        5 => {
            let _ = commitbee::extract_java_signature(source);
        }
        6 => {
            let _ = commitbee::extract_c_signature(source);
        }
        7 => {
            let _ = commitbee::extract_cpp_signature(source);
        }
        8 => {
            let _ = commitbee::extract_ruby_signature(source);
        }
        9 => {
            let _ = commitbee::extract_csharp_signature(source);
        }
        _ => unreachable!("selector is `% 10`"),
    }
});
