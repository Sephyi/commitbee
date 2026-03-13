// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    // scan_full_diff_for_secrets must never panic on any input
    let _ = commitbee::scan_full_diff_for_secrets(data);
});
