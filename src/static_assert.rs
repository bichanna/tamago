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

//! Compile-time assertions (`_Static_assert` / `static_assert`).
//!
//! These are invaluable for a C backend that wants to lock down ABI and layout
//! invariants — e.g. asserting `sizeof(Value) == 16` — so mismatches fail at
//! C-compile time rather than at runtime on a foreign platform. A
//! [`StaticAssert`] can be placed both at file scope (via
//! [`GlobalStatement::StaticAssert`](crate::GlobalStatement)) and inside a
//! function body (via [`Statement::StaticAssert`](crate::Statement)).

use std::fmt::{self, Write};

use crate::{Expr, Format, Formatter};
use tamacro::DisplayFromFormat;

/// A `_Static_assert(cond, "message")` compile-time assertion.
///
/// Renders with the C11 keyword `_Static_assert` by default, or the C23 spelling
/// `static_assert` when the formatter has
/// [`c23_keywords`](crate::RenderOptions::c23_keywords) enabled. The trailing
/// semicolon is added by the enclosing statement, matching how variable
/// declarations behave.
#[derive(Debug, Clone, DisplayFromFormat)]
pub struct StaticAssert {
    /// The constant expression that must be non-zero.
    pub cond: Expr,

    /// The diagnostic message emitted when the assertion fails.
    pub message: String,
}

impl StaticAssert {
    /// Creates a new static assertion.
    pub fn new(cond: Expr, message: String) -> Self {
        Self { cond, message }
    }

    /// Creates a new static assertion from a string-slice message.
    pub fn new_with_str(cond: Expr, message: &str) -> Self {
        Self::new(cond, message.to_string())
    }
}

impl Format for StaticAssert {
    fn format(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        let keyword = if fmt.c23_keywords() {
            "static_assert"
        } else {
            "_Static_assert"
        };
        write!(fmt, "{keyword}(")?;
        self.cond.format(fmt)?;
        write!(fmt, ", \"{}\")", crate::escape::escape_c_str(&self.message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    fn size_check() -> StaticAssert {
        StaticAssert::new_with_str(
            Expr::new_binary(
                Expr::new_sizeof(Type::base(BaseType::TypeDef("Value".to_string()))),
                BinOp::Eq,
                Expr::Int(16),
            ),
            "ABI: Value must be 16 bytes",
        )
    }

    #[test]
    fn c11_and_c23_spelling() {
        let sa = size_check();
        assert_eq!(
            sa.to_string(),
            "_Static_assert(sizeof(Value) == 16, \"ABI: Value must be 16 bytes\")"
        );
        let c23 = render(
            &sa,
            RenderOptions {
                c23_keywords: true,
                ..Default::default()
            },
        );
        assert_eq!(
            c23,
            "static_assert(sizeof(Value) == 16, \"ABI: Value must be 16 bytes\")"
        );
    }

    #[test]
    fn as_global_and_statement() {
        let global = GlobalStatement::StaticAssert(size_check());
        let scope = ScopeBuilder::new().global_statement(global).build();
        assert_eq!(
            scope.to_string(),
            "_Static_assert(sizeof(Value) == 16, \"ABI: Value must be 16 bytes\");\n"
        );

        let block = BlockBuilder::new()
            .statement(Statement::StaticAssert(size_check()))
            .build();
        assert_eq!(
            block.to_string(),
            "_Static_assert(sizeof(Value) == 16, \"ABI: Value must be 16 bytes\");\n"
        );
    }
}
