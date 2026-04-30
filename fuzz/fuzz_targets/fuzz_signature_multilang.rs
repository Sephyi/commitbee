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
    let source = String::from_utf8_lossy(&data[1..]);
    match selector {
        0 => {
            let _ = commitbee::extract_rust_signature(source.as_ref());
        }
        1 => {
            let _ = commitbee::extract_typescript_signature(source.as_ref());
        }
        2 => {
            let _ = commitbee::extract_javascript_signature(source.as_ref());
        }
        3 => {
            let _ = commitbee::extract_python_signature(source.as_ref());
        }
        4 => {
            let _ = commitbee::extract_go_signature(source.as_ref());
        }
        5 => {
            let _ = commitbee::extract_java_signature(source.as_ref());
        }
        6 => {
            let _ = commitbee::extract_c_signature(source.as_ref());
        }
        7 => {
            let _ = commitbee::extract_cpp_signature(source.as_ref());
        }
        8 => {
            let _ = commitbee::extract_ruby_signature(source.as_ref());
        }
        9 => {
            let _ = commitbee::extract_csharp_signature(source.as_ref());
        }
        _ => unreachable!("selector is `% 10`"),
    }
});
