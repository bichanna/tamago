// Copyright (c) 2025 Nobuharu Shimazu
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! Shared helpers for escaping text into C string and character literals.
//!
//! Every place that emits a quoted C literal — string expressions, character
//! constants, `#error`/`#warning`/`#include` payloads, and `#line` file paths —
//! routes through here so the escaping rules stay identical instead of drifting
//! between three near-duplicate implementations.

/// Escapes a Rust string into the body of a C string literal (no surrounding
/// quotes).
pub(crate) fn escape_c_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for &b in s.as_bytes() {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\t' => out.push_str("\\t"),
            b'\r' => out.push_str("\\r"),
            0x07 => out.push_str("\\a"),
            0x08 => out.push_str("\\b"),
            0x0b => out.push_str("\\v"),
            0x0c => out.push_str("\\f"),
            0x20..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\{b:03o}")),
        }
    }
    out
}

/// Escapes a Rust `char` into the body of a C character constant (no surrounding
/// quotes).
pub(crate) fn escape_c_char(c: char) -> String {
    match c {
        '\'' => "\\'".to_string(),
        '\\' => "\\\\".to_string(),
        '\n' => "\\n".to_string(),
        '\t' => "\\t".to_string(),
        '\r' => "\\r".to_string(),
        '\0' => "\\0".to_string(),
        '\u{07}' => "\\a".to_string(),
        '\u{08}' => "\\b".to_string(),
        '\u{0b}' => "\\v".to_string(),
        '\u{0c}' => "\\f".to_string(),
        c if (' '..='~').contains(&c) => c.to_string(),
        c if (c as u32) <= 0xff => format!("\\{:03o}", c as u32),
        c => format!("\\x{:x}", c as u32),
    }
}
