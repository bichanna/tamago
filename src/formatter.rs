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

//! This module provides a formatter implementation that emits a string of C code with the right
//! indentation.
//!
//! Most code is rendered simply via `to_string()`, but [`render`] with
//! [`RenderOptions`] enables opt-in features: the [`AttrStyle`] for attributes
//! (GNU vs C23) and `#line` directives driven by [`SourceLoc`]-tagged items

use std::fmt::{self, Write};

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

/// Options that control how code is rendered. Use with [`render`] or
/// [`Formatter::with_options`].
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderOptions {
    /// When `true`, located items and statements emit `#line N "file"`
    /// directives whenever the source location changes.
    pub line_directives: bool,

    /// The attribute spelling style.
    pub attr_style: AttrStyle,
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

/// Formatter for a scope
#[derive(Debug)]
pub struct Formatter<'a> {
    /// The buffer for the formatter
    dst: &'a mut String,

    /// The current identation level
    pub spaces: usize,

    /// The current scope
    scope: Vec<String>,

    /// Indentation level
    pub indent: usize,

    /// Rendering options (attribute style, whether to emit `#line`).
    opts: RenderOptions,

    /// The most recently emitted source location, for `#line` de-duplication.
    last_loc: Option<SourceLoc>,
}

impl<'a> Formatter<'a> {
    pub fn new(dst: &'a mut String) -> Self {
        Self {
            dst,
            spaces: 0,
            scope: vec![],
            indent: DEFAULT_INDENT,
            opts: RenderOptions::default(),
            last_loc: None,
        }
    }

    /// Creates a formatter with explicit [`RenderOptions`].
    pub fn with_options(dst: &'a mut String, opts: RenderOptions) -> Self {
        Self {
            dst,
            spaces: 0,
            scope: vec![],
            indent: DEFAULT_INDENT,
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
        if !self.is_start_of_line() {
            write!(self, " ")?;
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
        self.dst.is_empty() || self.dst.ends_with('\n')
    }

    fn push_spaces(&mut self) {
        self.dst.push_str(&" ".repeat(self.spaces))
    }
}

impl Write for Formatter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut should_indent = self.is_start_of_line();

        for (idx, line) in s.lines().enumerate() {
            if idx != 0 {
                self.dst.push('\n');
            }

            if should_indent && !line.is_empty() && line.as_bytes()[0] != b'\n' {
                self.push_spaces();
            }

            should_indent = true;

            self.dst.push_str(line);
        }

        if s.as_bytes().last() == Some(&b'\n') {
            self.dst.push('\n');
        }

        Ok(())
    }
}
