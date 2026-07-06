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

//! This module provides a formatter implementation that emits C code with the
//! right indentation.
//!
//! Output can be built into a `String` via `to_string()` / [`render`], or
//! *streamed* into any [`fmt::Write`] or [`io::Write`] sink via [`render_to`] /
//! [`render_to_io`], which avoids materializing one large `String` for big
//! translation units. [`RenderOptions`] controls opt-in behavior: the
//! [`AttrStyle`] for attributes (GNU vs C23), the [`IndentStyle`] (spaces or
//! tabs) and [`BraceStyle`] (K&R vs Allman), and `#line` directives driven by
//! [`SourceLoc`]-tagged items.

use std::fmt::{self, Write};
use std::io;

const DEFAULT_INDENT: usize = 2;

pub trait Format {
    fn format(&self, fmt: &mut Formatter<'_>) -> fmt::Result;
}

/// A source location used for `#line` mapping: a file path and a 1-based line
/// number in the *original* (pre-generation) source
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLoc {
    /// The originating source file.
    pub file: String,

    /// The 1-based line number within that file.
    pub line: u64,
}

impl SourceLoc {
    /// Creates a new source location.
    pub fn new(file: impl Into<String>, line: u64) -> Self {
        Self {
            file: file.into(),
            line,
        }
    }
}

/// The spelling style used when emitting attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AttrStyle {
    /// GNU style, e.g. `__attribute__((packed))`. This is the default for nwo.
    #[default]
    Gnu,

    /// C23 bracket style, e.g. `[[gnu::packed]]`.
    C23,
}

/// How each level of indentation is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    /// Indent with the given number of spaces per level (the default is 2).
    Spaces(usize),

    /// Indent with a single tab per level.
    Tabs,
}

impl Default for IndentStyle {
    fn default() -> Self {
        IndentStyle::Spaces(DEFAULT_INDENT)
    }
}

impl IndentStyle {
    /// The number of indent units added per nesting level.
    fn step(self) -> usize {
        match self {
            IndentStyle::Spaces(n) => n,
            IndentStyle::Tabs => 1,
        }
    }

    /// The character emitted for each indent unit.
    fn unit(self) -> char {
        match self {
            IndentStyle::Spaces(_) => ' ',
            IndentStyle::Tabs => '\t',
        }
    }
}

/// Where the opening brace of a block is placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BraceStyle {
    /// K&R / "one true brace" style: the `{` stays on the same line, e.g.
    /// `int main(void) {`. This is the default.
    #[default]
    Attach,

    /// Allman style: the `{` goes on its own line, e.g.
    /// ```c
    /// int main(void)
    /// {
    /// ```
    NextLine,
}

/// Options that control how code is rendered. Use with [`render`],
/// [`render_to`], [`render_to_io`], or [`Formatter::with_options`].
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderOptions {
    /// When `true`, located items and statements emit `#line N "file"`
    /// directives whenever the source location changes.
    pub line_directives: bool,

    /// The attribute spelling style.
    pub attr_style: AttrStyle,

    /// How indentation is rendered (spaces or tabs).
    pub indent: IndentStyle,

    /// Where opening braces are placed.
    pub brace_style: BraceStyle,
}

/// Renders any [`Format`] value to a `String` using the given [`RenderOptions`].
///
/// This is the entry point for opt-in features such as `#line` directives and
/// the C23 attribute style; the plain `to_string()` path always uses the
/// defaults (GNU attributes, no line directives).
pub fn render<T: Format>(item: &T, opts: RenderOptions) -> String {
    let mut dst = String::new();
    let mut fmt = Formatter::with_options(&mut dst, opts);
    // Writing into a String is infallible, so the result is safe to discard.
    let _ = item.format(&mut fmt);
    dst
}

/// Streams a [`Format`] value into any [`fmt::Write`] sink instead of building
/// one big `String` first. This matters for large translation units, where the
/// intermediate `String` would be sizable.
///
/// # Examples
///
/// ```rust
/// let mut out = String::new();
/// render_to(&item, &mut out, RenderOptions::default())?;
/// ```
pub fn render_to<T: Format, W: fmt::Write>(
    item: &T,
    dst: &mut W,
    opts: RenderOptions,
) -> fmt::Result {
    let mut fmt = Formatter::with_options(dst, opts);
    item.format(&mut fmt)
}

/// Streams a [`Format`] value directly into an [`io::Write`] sink (a file, a
/// socket, `stdout`, ...) without an intermediate `String` :)
///
/// I/O errors from the sink are surfaced as [`io::Error`].
///
/// # Examples
///
/// ```rust
/// let file = std::fs::File::create("out.c")?;
/// let mut writer = std::io::BufWriter::new(file);
/// render_to_io(&translation_unit, &mut writer, RenderOptions::default())?;
/// ```
pub fn render_to_io<T: Format, W: io::Write>(
    item: &T,
    dst: &mut W,
    opts: RenderOptions,
) -> io::Result<()> {
    /// Bridges an [`io::Write`] to [`fmt::Write`], capturing the first I/O error.
    struct IoAdapter<'w, W: io::Write> {
        w: &'w mut W,
        err: Option<io::Error>,
    }

    impl<W: io::Write> fmt::Write for IoAdapter<'_, W> {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            self.w.write_all(s.as_bytes()).map_err(|e| {
                self.err = Some(e);
                fmt::Error
            })
        }
    }

    let mut adapter = IoAdapter { w: dst, err: None };
    let res = {
        let mut fmt = Formatter::with_options(&mut adapter, opts);
        item.format(&mut fmt)
    };

    if let Some(e) = adapter.err {
        return Err(e);
    }
    res.map_err(|_| io::Error::new(io::ErrorKind::Other, "formatting failed"))
}

/// Escapes a string for inclusion inside a C string literal (used for the file
/// path in `#line` directives).
fn escape_c_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out
}

/// Formatter for a scope.
///
/// Writes into any [`fmt::Write`] sink (a `String`, or an [`io::Write`] adapter),
/// so output can be streamed rather than accumulated into one large `String`.
pub struct Formatter<'a> {
    /// The sink the formatted code is written to.
    dst: &'a mut dyn fmt::Write,

    /// Whether the cursor is at the start of a line (indentation pending).
    at_line_start: bool,

    /// The current indentation width, in indent units (spaces or tabs).
    pub spaces: usize,

    /// The current scope
    scope: Vec<String>,

    /// The number of indent units added per nesting level.
    pub indent: usize,

    /// The character emitted per indent unit (`' '` or `'\t'`).
    indent_unit: char,

    /// Rendering options (attribute style, indent/brace style, `#line`).
    opts: RenderOptions,

    /// The most recently emitted source location, for `#line` de-duplication.
    last_loc: Option<SourceLoc>,
}

impl fmt::Debug for Formatter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Formatter")
            .field("spaces", &self.spaces)
            .field("indent", &self.indent)
            .field("opts", &self.opts)
            .finish_non_exhaustive()
    }
}

impl<'a> Formatter<'a> {
    pub fn new<W: fmt::Write>(dst: &'a mut W) -> Self {
        Self::with_options(dst, RenderOptions::default())
    }

    /// Creates a formatter with explicit [`RenderOptions`], over any sink.
    pub fn with_options<W: fmt::Write>(dst: &'a mut W, opts: RenderOptions) -> Self {
        Self {
            dst,
            at_line_start: true,
            spaces: 0,
            scope: vec![],
            indent: opts.indent.step(),
            indent_unit: opts.indent.unit(),
            opts,
            last_loc: None,
        }
    }

    /// Returns the attribute spelling style in effect.
    pub fn attr_style(&self) -> AttrStyle {
        self.opts.attr_style
    }

    /// Returns whether `#line` directives are being emitted
    pub fn line_directives_enabled(&self) -> bool {
        self.opts.line_directives
    }

    /// Emits a `#line N "file"` directive if line directives are enabled and the
    /// location differs from the last one emitted, and otherwise, it does nothing.
    ///
    /// Located items and statements call this before emitting their content, so
    /// that the C compiler and any debugger map the following code back to the
    /// original source.
    pub fn sync_line(&mut self, loc: &SourceLoc) -> fmt::Result {
        if !self.opts.line_directives {
            return Ok(());
        }
        if self.last_loc.as_ref() == Some(loc) {
            return Ok(());
        }
        if !self.is_start_of_line() {
            writeln!(self)?;
        }
        writeln!(
            self,
            "#line {} \"{}\"",
            loc.line,
            escape_c_string(&loc.file)
        )?;
        self.last_loc = Some(loc.clone());
        Ok(())
    }

    pub fn scope<F, R>(&mut self, name: &str, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.scope.push(name.to_string());
        let res = f(self);
        self.scope.pop();
        res
    }

    pub fn block<F>(&mut self, f: F) -> fmt::Result
    where
        F: FnOnce(&mut Self) -> fmt::Result,
    {
        match self.opts.brace_style {
            BraceStyle::Attach => {
                if !self.is_start_of_line() {
                    write!(self, " ")?;
                }
            }
            BraceStyle::NextLine => {
                if !self.is_start_of_line() {
                    writeln!(self)?;
                }
            }
        }

        writeln!(self, "{{")?;
        self.indent(f)?;
        write!(self, "}}")?;

        Ok(())
    }

    pub fn indent<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.spaces += self.indent;
        let res = f(self);
        self.spaces -= self.indent;
        res
    }

    pub fn is_start_of_line(&self) -> bool {
        self.at_line_start
    }
}

/// Writes `n` indent units of the given character to the sink, batching to avoid
/// a virtual call per character.
fn write_indent(dst: &mut dyn fmt::Write, unit: char, n: usize) -> fmt::Result {
    const SPACES: &str = "                                "; // 32 spaces
    const TABS: &str = "\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t"; // 16 tabs
    let filler = if unit == '\t' { TABS } else { SPACES };
    let mut remaining = n;
    while remaining > 0 {
        let take = remaining.min(filler.len());
        dst.write_str(&filler[..take])?;
        remaining -= take;
    }
    Ok(())
}

impl Write for Formatter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for (idx, line) in s.split('\n').enumerate() {
            if idx != 0 {
                self.dst.write_char('\n')?;
                self.at_line_start = true;
            }

            if line.is_empty() {
                continue;
            }

            if self.at_line_start {
                write_indent(self.dst, self.indent_unit, self.spaces)?;
                self.at_line_start = false;
            }

            self.dst.write_str(line)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BaseType, BlockBuilder, Function, FunctionBuilder, Statement, Type};

    fn sample() -> Function {
        FunctionBuilder::new_with_str("f", Type::base(BaseType::Void))
            .body(
                BlockBuilder::new()
                    .statement(Statement::Return(None))
                    .build(),
            )
            .build()
    }

    #[test]
    fn stream_to_fmt_write_matches_to_string() {
        let f = sample();
        let mut streamed = String::new();
        render_to(&f, &mut streamed, RenderOptions::default()).unwrap();
        assert_eq!(streamed, f.to_string());
    }

    #[test]
    fn stream_to_io_write() {
        let f = sample();
        let mut bytes: Vec<u8> = Vec::new();
        render_to_io(&f, &mut bytes, RenderOptions::default()).unwrap();
        assert_eq!(String::from_utf8(bytes).unwrap(), f.to_string());
    }

    #[test]
    fn indent_spaces_width() {
        let f = sample();
        let out = render(
            &f,
            RenderOptions {
                indent: IndentStyle::Spaces(4),
                ..Default::default()
            },
        );
        assert_eq!(out, "void f(void) {\n    return;\n}\n");
    }

    #[test]
    fn indent_tabs() {
        let f = sample();
        let out = render(
            &f,
            RenderOptions {
                indent: IndentStyle::Tabs,
                ..Default::default()
            },
        );
        assert_eq!(out, "void f(void) {\n\treturn;\n}\n");
    }

    #[test]
    fn brace_style_next_line() {
        let f = sample();
        let out = render(
            &f,
            RenderOptions {
                brace_style: BraceStyle::NextLine,
                ..Default::default()
            },
        );
        assert_eq!(out, "void f(void)\n{\n  return;\n}\n");
    }
}
