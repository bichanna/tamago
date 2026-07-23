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

//! C attributes, in both GNU (`__attribute__((...))`) and C23 (`[[...]]`) form
//!
//! An [`Attribute`] is a name, an optional namespace (e.g. `gnu`), and a list of
//! already-rendered argument strings. The named constructors cover the
//! attributes a code generator reaches for most often — `packed`, `aligned`,
//! `noreturn`, `always_inline`, `weak`, `section`, `visibility`, `deprecated`,
//! `unused`, `cleanup`, and `format` — and each produces a GNU attribute (so it
//! renders as `[[gnu::...]]` in C23 mode).
//!
//! The output style is chosen by the [`AttrStyle`](crate::AttrStyle) carried by
//! the [`Formatter`](crate::Formatter), and attributes attached to functions,
//! structs, fields, parameters, and variables are rendered in whichever style
//! the surrounding render uses.
//!
//! Placement is position-sensitive in C, so each item has one canonical slot:
//! functions and parameters and variables take a *leading* attribute list,
//! structs place theirs right after the `struct`/`union` keyword, and fields
//! place theirs *after* the declarator (and any bitfield width).

use std::fmt::{self, Write};

use crate::escape::escape_c_str;
use crate::{AttrStyle, Format, Formatter};
use tamacro::DisplayFromFormat;

/// A single C attribute, such as `packed`, `aligned(8)`, or `format(printf, 1, 2)`.
#[derive(Debug, Clone, PartialEq, Eq, DisplayFromFormat)]
pub struct Attribute {
    /// An optional namespace, shown only in C23 form (e.g. `gnu` renders as
    /// `[[gnu::name]]`). GNU form never prints a namespace
    pub namespace: Option<String>,

    /// The attribute name.
    pub name: String,

    /// The argument list, each already rendered as it should appear inside the
    /// parentheses (string arguments include their own quotes).
    pub args: Vec<String>,
}

impl Attribute {
    /// Creates a plain, namespace-less attribute with the given name and
    /// arguments. Useful for standard C23 attributes (`nodiscard`,
    /// `maybe_unused`, ...) or as a general escape hatch.
    pub fn new(name: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            namespace: None,
            name: name.into(),
            args,
        }
    }

    /// Creates a GNU attribute (namespace `gnu`), which renders as
    /// `__attribute__((name(args)))` in GNU form and `[[gnu::name(args)]]` in
    /// C23 form.
    pub fn gnu(name: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            namespace: Some("gnu".to_string()),
            name: name.into(),
            args,
        }
    }

    /// `packed` — remove padding between struct members.
    pub fn packed() -> Self {
        Self::gnu("packed", vec![])
    }

    /// `aligned(n)` — align to an `n`-byte boundary.
    pub fn aligned(n: u64) -> Self {
        Self::gnu("aligned", vec![n.to_string()])
    }

    /// `noreturn` — the function does not return (panic/abort/`unreachable`).
    pub fn noreturn() -> Self {
        Self::gnu("noreturn", vec![])
    }

    /// `always_inline` — force inlining.
    pub fn always_inline() -> Self {
        Self::gnu("always_inline", vec![])
    }

    /// `weak` — emit as a weak symbol.
    pub fn weak() -> Self {
        Self::gnu("weak", vec![])
    }

    /// `section("name")` — place the symbol in a named section.
    pub fn section(name: &str) -> Self {
        Self::gnu("section", vec![quote(name)])
    }

    /// `visibility("kind")` — e.g. `"default"`, `"hidden"`, `"protected"`.
    pub fn visibility(kind: &str) -> Self {
        Self::gnu("visibility", vec![quote(kind)])
    }

    /// `deprecated` — warn on use.
    pub fn deprecated() -> Self {
        Self::gnu("deprecated", vec![])
    }

    /// `deprecated("message")` — warn on use, with a message.
    pub fn deprecated_msg(message: &str) -> Self {
        Self::gnu("deprecated", vec![quote(message)])
    }

    /// `unused` — suppress unused warnings (the C23 spelling is `maybe_unused`).
    pub fn unused() -> Self {
        Self::gnu("unused", vec![])
    }

    /// `used` — keep the symbol even if it appears unused.
    pub fn used() -> Self {
        Self::gnu("used", vec![])
    }

    /// `cleanup(func)` — run `func` when the variable goes out of scope.
    pub fn cleanup(func: &str) -> Self {
        Self::gnu("cleanup", vec![func.to_string()])
    }

    /// `format(archetype, string_index, first_to_check)` — printf/scanf-style
    /// checking, e.g. `format(printf, 1, 2)`.
    pub fn format(archetype: &str, string_index: u64, first_to_check: u64) -> Self {
        Self::gnu(
            "format",
            vec![
                archetype.to_string(),
                string_index.to_string(),
                first_to_check.to_string(),
            ],
        )
    }

    /// Renders this attribute as a single entry (without the enclosing
    /// `__attribute__(())` or `[[]]`), applying the namespace only in C23 form.
    fn entry(&self, style: AttrStyle) -> String {
        let mut out = String::new();
        if style == AttrStyle::C23
            && let Some(ns) = &self.namespace {
                out.push_str(ns);
                out.push_str("::");
            }
        out.push_str(&self.name);
        if !self.args.is_empty() {
            out.push('(');
            out.push_str(&self.args.join(", "));
            out.push(')');
        }
        out
    }
}

impl Format for Attribute {
    /// Formats this single attribute as a complete, wrapped group in the
    /// [`Formatter`]'s ambient style — `__attribute__((name(args)))` in GNU form
    /// or `[[ns::name(args)]]` in C23 form. To emit several attributes as one
    /// group, use [`write_attrs`].
    fn format(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        let s = format_attrs(std::slice::from_ref(self), fmt.attr_style());
        write!(fmt, "{s}")
    }
}

/// Wraps a value in double quotes, escaping it for use as a string-literal
/// attribute argument.
fn quote(s: &str) -> String {
    format!("\"{}\"", escape_c_str(s))
}

/// Renders a list of attributes in the given style, or the empty string if the
/// list is empty.
///
/// - GNU: `__attribute__((packed, aligned(8)))`
/// - C23: `[[gnu::packed, gnu::aligned(8)]]`
///
/// # Examples
///
/// ```rust
/// let attrs = vec![Attribute::packed(), Attribute::aligned(8)];
/// assert_eq!(
///     format_attrs(&attrs, AttrStyle::Gnu),
///     "__attribute__((packed, aligned(8)))"
/// );
/// assert_eq!(
///     format_attrs(&attrs, AttrStyle::C23),
///     "[[gnu::packed, gnu::aligned(8)]]"
/// );
/// ```
pub fn format_attrs(attrs: &[Attribute], style: AttrStyle) -> String {
    if attrs.is_empty() {
        return String::new();
    }

    let entries = attrs
        .iter()
        .map(|a| a.entry(style))
        .collect::<Vec<_>>()
        .join(", ");

    match style {
        AttrStyle::Gnu => format!("__attribute__(({entries}))"),
        AttrStyle::C23 => format!("[[{entries}]]"),
    }
}

/// Renders raw macro/specifier tokens followed by the typed attribute group, or
/// the empty string if both are empty
///
/// This is the combined form used at each item's annotation slot: the raw tokens
/// (verbatim macros like `MYLANG_EXPORT` or `__declspec(dllexport)`) come first,
/// then the typed [`Attribute`]s grouped into one `__attribute__((...))` / `[[...]]`.
///
/// # Examples
///
/// ```rust
/// let raw = vec!["MYLANG_EXPORT".to_string()];
/// let typed = vec![Attribute::noreturn()];
/// assert_eq!(
///     format_annotations(&raw, &typed, AttrStyle::Gnu),
///     "MYLANG_EXPORT __attribute__((noreturn))"
/// );
/// ```
pub fn format_annotations(raw: &[String], typed: &[Attribute], style: AttrStyle) -> String {
    let typed = format_attrs(typed, style);
    match (raw.is_empty(), typed.is_empty()) {
        (true, true) => String::new(),
        (false, true) => raw.join(" "),
        (true, false) => typed,
        (false, false) => format!("{} {typed}", raw.join(" ")),
    }
}

/// Returns whether an annotation slot would render to anything — i.e. there is at
/// least one raw specifier token or one typed attribute. Handy for deciding
/// separating whitespace around [`write_annotations`].
pub fn has_annotations(raw: &[String], typed: &[Attribute]) -> bool {
    !raw.is_empty() || !typed.is_empty()
}

/// Writes a group of attributes into `fmt` as a single `__attribute__((...))` /
/// `[[...]]` group, in the formatter's ambient [`AttrStyle`], or nothing if the
/// list is empty.
///
/// This is the [`Formatter`]-based counterpart to [`format_attrs`]; prefer it
/// wherever a live formatter is available so the style is taken from the
/// formatter instead of being threaded by hand.
pub fn write_attrs(fmt: &mut Formatter<'_>, attrs: &[Attribute]) -> fmt::Result {
    let s = format_attrs(attrs, fmt.attr_style());
    if !s.is_empty() {
        write!(fmt, "{s}")?;
    }
    Ok(())
}

/// Writes the combined annotation slot (raw specifier tokens followed by the
/// grouped typed attributes) into `fmt` in the ambient [`AttrStyle`], or nothing
/// if both are empty.
///
/// This is the [`Formatter`]-based counterpart to [`format_annotations`]. It does
/// not emit any surrounding whitespace — callers place the leading or trailing
/// space appropriate to the item's annotation slot (guarded by
/// [`has_annotations`]).
pub fn write_annotations(
    fmt: &mut Formatter<'_>,
    raw: &[String],
    typed: &[Attribute],
) -> fmt::Result {
    let s = format_annotations(raw, typed, fmt.attr_style());
    if !s.is_empty() {
        write!(fmt, "{s}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gnu_and_c23_spelling() {
        let attrs = vec![Attribute::packed(), Attribute::aligned(16)];
        assert_eq!(
            format_attrs(&attrs, AttrStyle::Gnu),
            "__attribute__((packed, aligned(16)))"
        );
        assert_eq!(
            format_attrs(&attrs, AttrStyle::C23),
            "[[gnu::packed, gnu::aligned(16)]]"
        );
    }

    #[test]
    fn string_and_numeric_args() {
        assert_eq!(
            format_attrs(&[Attribute::section(".boot")], AttrStyle::Gnu),
            "__attribute__((section(\".boot\")))"
        );
        assert_eq!(
            format_attrs(&[Attribute::format("printf", 1, 2)], AttrStyle::Gnu),
            "__attribute__((format(printf, 1, 2)))"
        );
        assert_eq!(
            format_attrs(&[Attribute::deprecated_msg("use bar")], AttrStyle::C23),
            "[[gnu::deprecated(\"use bar\")]]"
        );
    }

    #[test]
    fn plain_and_empty() {
        assert_eq!(
            format_attrs(&[Attribute::new("nodiscard", vec![])], AttrStyle::C23),
            "[[nodiscard]]"
        );
        assert_eq!(format_attrs(&[], AttrStyle::Gnu), "");
    }

    #[test]
    fn format_impl_uses_ambient_style() {
        use crate::{RenderOptions, render};
        let a = Attribute::packed();
        assert_eq!(a.to_string(), "__attribute__((packed))");
        let c23 = render(
            &a,
            RenderOptions {
                attr_style: AttrStyle::C23,
                ..Default::default()
            },
        );
        assert_eq!(c23, "[[gnu::packed]]");
    }

    #[test]
    fn formatter_writers_match_string_form() {
        use crate::{Formatter, RenderOptions};
        let raw = vec!["MYLANG_EXPORT".to_string()];
        let typed = vec![Attribute::noreturn()];

        assert!(has_annotations(&raw, &typed));
        assert!(!has_annotations(&[], &[]));

        let mut buf = String::new();
        {
            let mut fmt = Formatter::new(&mut buf);
            write_annotations(&mut fmt, &raw, &typed).unwrap();
        }
        assert_eq!(buf, format_annotations(&raw, &typed, AttrStyle::Gnu));
        assert_eq!(buf, "MYLANG_EXPORT __attribute__((noreturn))");

        let mut buf = String::new();
        {
            let mut fmt = Formatter::with_options(
                &mut buf,
                RenderOptions {
                    attr_style: AttrStyle::C23,
                    ..Default::default()
                },
            );
            write_attrs(&mut fmt, &typed).unwrap();
        }
        assert_eq!(buf, "[[gnu::noreturn]]");

        // empty slots write nothing
        let mut buf = String::new();
        {
            let mut fmt = Formatter::new(&mut buf);
            write_annotations(&mut fmt, &[], &[]).unwrap();
        }
        assert_eq!(buf, "");
    }
}
