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

//! Identifier hygiene helpers for generated C.
//!
//! When names are derived from another language's source (a symbol table, a
//! type name, a user string), they may not be valid C identifiers or may clash
//! with C keywords. This module provides:
//!
//! - [`sanitize_ident`], which turns an arbitrary string into a valid C
//!   identifier;
//! - [`is_c_keyword`] and [`C_KEYWORDS`], to test against the reserved words;
//! - [`Gensym`], a small counter for minting fresh, unique identifiers (for
//!   temporaries, labels, and the like).
//!
//! These are standalone utilities: nothing in Tamago rewrites your names behind
//! your back, so call [`sanitize_ident`] yourself when you need the guarantee :)

/// The reserved keywords of C, covering C11 (including the `_`-prefixed
/// keywords) and the additions introduced by C23.
///
/// Used by [`is_c_keyword`] and [`sanitize_ident`].
pub const C_KEYWORDS: &[&str] = &[
    // C11
    "auto",
    "break",
    "case",
    "char",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extern",
    "float",
    "for",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "register",
    "restrict",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "struct",
    "switch",
    "typedef",
    "union",
    "unsigned",
    "void",
    "volatile",
    "while",
    "_Alignas",
    "_Alignof",
    "_Atomic",
    "_Bool",
    "_Complex",
    "_Generic",
    "_Imaginary",
    "_Noreturn",
    "_Static_assert",
    "_Thread_local",
    // C23
    "alignas",
    "alignof",
    "bool",
    "constexpr",
    "false",
    "nullptr",
    "static_assert",
    "thread_local",
    "true",
    "typeof",
    "typeof_unqual",
    "_BitInt",
    "_Decimal128",
    "_Decimal32",
    "_Decimal64",
];

/// Returns `true` if `s` is a reserved C keyword (see [`C_KEYWORDS`])
///
/// # Example
///
/// ```rust
/// assert!(is_c_keyword("int"));
/// assert!(is_c_keyword("_Bool"));
/// assert!(!is_c_keyword("integer"));
/// ```
pub fn is_c_keyword(s: &str) -> bool {
    C_KEYWORDS.contains(&s)
}

/// Rewrites an arbitrary string into a valid C identifier.
///
/// The transformation is:
/// 1. every character that is not an ASCII letter, digit, or underscore becomes
///    an underscore
/// 2. an empty result, or one starting with a digit, is prefixed with an
///    underscore so the identifier is well-formed
/// 3. if the result collides with a C keyword, an underscore is appended
///
/// The result is always a non-empty, valid, non-keyword C identifier. Note that
/// distinct inputs can map to the same output (e.g. `"a b"` and `"a-b"` both
/// become `"a_b"`); use [`Gensym`] when you need guaranteed uniqueness.
///
/// # Examples
///
/// ```rust
/// assert_eq!(sanitize_ident("hello world"), "hello_world");
/// assert_eq!(sanitize_ident("123abc"), "_123abc");
/// assert_eq!(sanitize_ident("struct"), "struct_");
/// assert_eq!(sanitize_ident(""), "_");
/// ```
pub fn sanitize_ident(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }

    if out.is_empty() {
        out.push('_');
    } else if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }

    if is_c_keyword(&out) {
        out.push('_');
    }

    out
}

/// A generator of fresh, unique identifiers.
///
/// Each call to [`Gensym::fresh`] returns a new name of the form
/// `{prefix}{n}` with a monotonically increasing counter, so the names never
/// repeat for the lifetime of the generator. This is handy for compiler-emitted
/// temporaries, labels, and helper variables.
///
/// The default prefix is `__tmp`. the leading double underscore intentionally
/// lands in C's implementation-reserved identifier space so generated names do
/// not collide with ordinary user identifiers. Override it with
/// [`Gensym::with_prefix`] if you prefer a different convention.
///
/// # Examples
///
/// ```rust
/// let mut g = Gensym::new();
/// assert_eq!(g.fresh(), "__tmp0");
/// assert_eq!(g.fresh(), "__tmp1");
///
/// let mut labels = Gensym::with_prefix("L");
/// assert_eq!(labels.fresh(), "L0");
/// ```
#[derive(Debug, Clone)]
pub struct Gensym {
    prefix: String,
    counter: u64,
}

impl Gensym {
    /// Creates a generator with the default `__tmp` prefix.
    pub fn new() -> Self {
        Self::with_prefix("__tmp")
    }

    /// Creates a generator with a custom prefix.
    pub fn with_prefix(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
            counter: 0,
        }
    }

    /// Returns the next fresh identifier, `{prefix}{n}`.
    pub fn fresh(&mut self) -> String {
        let name = format!("{}{}", self.prefix, self.counter);
        self.counter += 1;
        name
    }

    /// Returns a fresh identifier that incorporates a (sanitized) base name,
    /// `{prefix}_{base}_{n}`. Useful for keeping generated temporaries readable.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let mut g = Gensym::new();
    /// assert_eq!(g.fresh_named("index"), "__tmp_index_0");
    /// ```
    pub fn fresh_named(&mut self, base: &str) -> String {
        let name = format!("{}_{}_{}", self.prefix, sanitize_ident(base), self.counter);
        self.counter += 1;
        name
    }

    /// Resets the counter back to zero.
    pub fn reset(&mut self) {
        self.counter = 0;
    }
}

impl Default for Gensym {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_invalid_characters() {
        assert_eq!(sanitize_ident("hello world"), "hello_world");
        assert_eq!(sanitize_ident("my-var!"), "my_var_");
        assert_eq!(sanitize_ident("a.b.c"), "a_b_c");
        assert_eq!(sanitize_ident("ns::name"), "ns__name");
    }

    #[test]
    fn sanitizes_leading_digit_and_empty() {
        assert_eq!(sanitize_ident("123abc"), "_123abc");
        assert_eq!(sanitize_ident("0"), "_0");
        assert_eq!(sanitize_ident(""), "_");
    }

    #[test]
    fn sanitizes_keywords() {
        assert_eq!(sanitize_ident("int"), "int_");
        assert_eq!(sanitize_ident("struct"), "struct_");
        assert_eq!(sanitize_ident("_Bool"), "_Bool_");
        // not a keyword, left untouched
        assert_eq!(sanitize_ident("count"), "count");
    }

    #[test]
    fn keyword_lookup() {
        assert!(is_c_keyword("return"));
        assert!(is_c_keyword("bool"));
        assert!(is_c_keyword("_Atomic"));
        assert!(!is_c_keyword("Return"));
        assert!(!is_c_keyword("counter"));
    }

    #[test]
    fn gensym_is_unique_and_prefixed() {
        let mut g = Gensym::new();
        assert_eq!(g.fresh(), "__tmp0");
        assert_eq!(g.fresh(), "__tmp1");
        assert_eq!(g.fresh_named("index"), "__tmp_index_2");

        g.reset();
        assert_eq!(g.fresh(), "__tmp0");

        let mut labels = Gensym::with_prefix("L");
        assert_eq!(labels.fresh(), "L0");
        assert_eq!(labels.fresh(), "L1");
    }
}
