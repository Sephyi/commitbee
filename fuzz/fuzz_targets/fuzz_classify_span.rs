// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // classify_diff_span must never panic on any input
    if data.len() < 16 {
        return;
    }
    // Use first 16 bytes for line range parameters, rest as diff
    let new_start = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize % 1000;
    let new_end = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize % 1000;
    let old_start = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize % 1000;
    let old_end = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize % 1000;
    if let Ok(diff) = std::str::from_utf8(&data[16..]) {
        let _ = commitbee::classify_diff_span(diff, new_start, new_end, old_start, old_end);
    }
});
