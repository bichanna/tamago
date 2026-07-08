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

//! This module provides a translation-unit abstraction that splits generated C
//! into a header (`.h`) and a source (`.c`) file.
//!
//! A [`Module`] holds two streams of [`GlobalStatement`]s — one destined for the
//! header, one for the source — plus a header guard and an optional self-include
//! in the source. The [`ModuleBuilder`] routing helpers put things where they
//! belong automatically: a function definition contributes a *prototype* to the
//! header and its *body* to the source, a public global variable becomes an
//! `extern` declaration in the header and a definition in the source, and shared
//! type declarations (typedefs, structs, enums, unions) go to the header.
//!
//! ```rust
//! let m = Module::new("counter")
//!     .header_include_system("stdint.h")
//!     .function(
//!         FunctionBuilder::new_with_str("counter_next", Type::new(BaseType::UInt64).build())
//!             .body(Block::new().build())
//!             .build(),
//!     )
//!     .build();
//!
//! let header = m.header(); // #ifndef COUNTER_H ... uint64_t counter_next(void); ... #endif
//! let source = m.source(); // #include "counter.h" ... uint64_t counter_next(void) { }
//! ```

use crate::{
    Enum, Function, GlobalStatement, Include, IncludeBuilder, RenderOptions, ScopeBuilder, Struct,
    TypeDef, Union, Variable, VariableBuilder,
};

/// Controls how a module's header protects itself against multiple inclusion
#[derive(Debug, Clone)]
pub enum HeaderGuard {
    /// Emit `#pragma once`.
    PragmaOnce,

    /// Emit a classic `#ifndef`/`#define` ... `#endif` guard using the given
    /// macro name.
    Ifndef(String),
}

/// A C translation unit: a header (`.h`) and a source (`.c`) generated together.
///
/// Build one with [`Module::new`] and the [`ModuleBuilder`] routing helpers, then
/// render each file with [`Module::header`] and [`Module::source`].
#[derive(Debug, Clone)]
pub struct Module {
    name: String,
    guard: HeaderGuard,
    self_include: bool,
    header: Vec<GlobalStatement>,
    source: Vec<GlobalStatement>,
}

impl Module {
    /// Creates a [`ModuleBuilder`] for a module with the given base name (used
    /// for the header guard and the source's self-include, e.g. `"widget"` for
    /// `widget.h`/`widget.c`).
    pub fn new(name: &str) -> ModuleBuilder {
        ModuleBuilder::new(name)
    }

    /// The module's base name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The statements destined for the header.
    pub fn header_stmts(&self) -> &[GlobalStatement] {
        &self.header
    }

    /// The statements destined for the source.
    pub fn source_stmts(&self) -> &[GlobalStatement] {
        &self.source
    }

    /// Renders the header file, wrapped in the configured include guard, using
    /// default [`RenderOptions`].
    pub fn header(&self) -> String {
        self.header_with(RenderOptions::default())
    }

    /// Renders the header file with explicit [`RenderOptions`] (so `#line`
    /// directives, tab indentation, C23 spellings, brace style, etc. all apply).
    pub fn header_with(&self, opts: RenderOptions) -> String {
        let body = render_with(&self.header, opts);
        match &self.guard {
            HeaderGuard::PragmaOnce => {
                if body.is_empty() {
                    "#pragma once\n".to_string()
                } else {
                    format!("#pragma once\n\n{body}")
                }
            }
            HeaderGuard::Ifndef(g) => {
                format!("#ifndef {g}\n#define {g}\n\n{body}\n#endif /* {g} */\n")
            }
        }
    }

    /// Renders the source file, prefixed (unless disabled) with an include of
    /// this module's own header, using default [`RenderOptions`].
    pub fn source(&self) -> String {
        self.source_with(RenderOptions::default())
    }

    /// Renders the source file with explicit [`RenderOptions`].
    pub fn source_with(&self, opts: RenderOptions) -> String {
        let body = render_with(&self.source, opts);
        if self.self_include {
            if body.is_empty() {
                format!("#include \"{}.h\"\n", self.name)
            } else {
                format!("#include \"{}.h\"\n\n{body}", self.name)
            }
        } else {
            body
        }
    }
}

/// Renders a slice of global statements to a string via a [`Scope`], with the
/// given options.
fn render_with(stmts: &[GlobalStatement], opts: RenderOptions) -> String {
    crate::render(
        &ScopeBuilder::new()
            .global_statements(stmts.to_vec())
            .build(),
        opts,
    )
}

/// Derives a default `#ifndef` guard macro from a module name, for example, `"gfx/canvas"`
/// becomes `CANVAS_H`.
fn default_guard(name: &str) -> String {
    let stem = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let stem = stem.split('.').next().unwrap_or(stem);

    let mut out = String::new();
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }

    if out.is_empty() {
        out.push_str("MODULE");
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out.push_str("_H");
    out
}

/// A builder for [`Module`] with helpers that route declarations and definitions
/// to the header or the source automatically.
pub struct ModuleBuilder {
    name: String,
    guard: HeaderGuard,
    self_include: bool,
    header: Vec<GlobalStatement>,
    source: Vec<GlobalStatement>,
}

impl ModuleBuilder {
    /// Creates a new builder for a module with the given base name. By default
    /// the header uses an `#ifndef` guard derived from the name and the source
    /// includes its own header.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            guard: HeaderGuard::Ifndef(default_guard(name)),
            self_include: true,
            header: vec![],
            source: vec![],
        }
    }

    /// Uses `#pragma once` for the header guard instead of `#ifndef`.
    pub fn pragma_once(mut self) -> Self {
        self.guard = HeaderGuard::PragmaOnce;
        self
    }

    /// Overrides the header guard macro (implies an `#ifndef` guard).
    pub fn include_guard(mut self, macro_name: &str) -> Self {
        self.guard = HeaderGuard::Ifndef(macro_name.to_string());
        self
    }

    /// Stops the source file from including its own header.
    pub fn no_self_include(mut self) -> Self {
        self.self_include = false;
        self
    }

    /// Pushes a raw statement into the header
    pub fn header_stmt(mut self, stmt: GlobalStatement) -> Self {
        self.header.push(stmt);
        self
    }

    /// Pushes a raw statement into the source.
    pub fn source_stmt(mut self, stmt: GlobalStatement) -> Self {
        self.source.push(stmt);
        self
    }

    /// Adds a system `#include <...>` to the header.
    pub fn header_include_system(self, path: &str) -> Self {
        self.header_include_impl(IncludeBuilder::new_system_with_str(path).build())
    }

    /// Adds a local `#include "..."` to the header.
    pub fn header_include(self, path: &str) -> Self {
        self.header_include_impl(IncludeBuilder::new_with_str(path).build())
    }

    /// Adds a system `#include <...>` to the source.
    pub fn source_include_system(self, path: &str) -> Self {
        self.source_include_impl(IncludeBuilder::new_system_with_str(path).build())
    }

    /// Adds a local `#include "..."` to the source.
    pub fn source_include(self, path: &str) -> Self {
        self.source_include_impl(IncludeBuilder::new_with_str(path).build())
    }

    fn header_include_impl(mut self, inc: Include) -> Self {
        self.header.push(GlobalStatement::Include(inc));
        self
    }

    fn source_include_impl(mut self, inc: Include) -> Self {
        self.source.push(GlobalStatement::Include(inc));
        self
    }

    /// Adds a typedef to the header.
    pub fn typedef(mut self, td: TypeDef) -> Self {
        self.header.push(GlobalStatement::TypeDef(td));
        self
    }

    /// Adds a struct declaration to the header.
    pub fn struct_(mut self, s: Struct) -> Self {
        self.header.push(GlobalStatement::Struct(s));
        self
    }

    /// Adds an enum declaration to the header.
    pub fn enum_(mut self, e: Enum) -> Self {
        self.header.push(GlobalStatement::Enum(e));
        self
    }

    /// Adds a union declaration to the header.
    pub fn union_(mut self, u: Union) -> Self {
        self.header.push(GlobalStatement::Union(u));
        self
    }

    /// Adds a function, routing it automatically:
    /// - a prototype (no body) goes to the header
    /// - a `static` definition goes only to the source
    /// - any other definition contributes a prototype to the header and its body
    ///   to the source.
    pub fn function(mut self, f: Function) -> Self {
        if f.is_prototype() {
            self.header.push(GlobalStatement::Function(f));
        } else if f.storage.is_static() {
            self.source.push(GlobalStatement::Function(f));
        } else {
            self.header
                .push(GlobalStatement::Function(f.to_prototype()));
            self.source.push(GlobalStatement::Function(f));
        }
        self
    }

    /// Adds a global variable, routing it automatically:
    /// - an `extern` variable is a declaration and goes to the header;
    /// - a `static` variable is translation-unit-local and goes to the source;
    /// - any other variable becomes an `extern` declaration in the header and a
    ///   definition in the source.
    pub fn global(mut self, v: Variable) -> Self {
        if v.storage.is_extern() {
            self.header.push(GlobalStatement::Variable(v));
        } else if v.storage.is_static() {
            self.source.push(GlobalStatement::Variable(v));
        } else {
            let mut decl = VariableBuilder::new(v.name.clone(), v.t.clone()).make_extern();
            if let Some(doc) = &v.doc {
                decl = decl.doc(doc.clone());
            }
            self.header.push(GlobalStatement::Variable(decl.build()));
            self.source.push(GlobalStatement::Variable(v));
        }
        self
    }

    /// Adds a blank line to the header (for spacing).
    pub fn header_newline(mut self) -> Self {
        self.header.push(GlobalStatement::NewLine);
        self
    }

    /// Adds a blank line to the source (for spacing).
    pub fn source_newline(mut self) -> Self {
        self.source.push(GlobalStatement::NewLine);
        self
    }

    /// Inserts raw text into the header.
    pub fn header_raw(mut self, raw: &str) -> Self {
        self.header.push(GlobalStatement::Raw(raw.to_string()));
        self
    }

    /// Inserts raw text into the source.
    pub fn source_raw(mut self, raw: &str) -> Self {
        self.source.push(GlobalStatement::Raw(raw.to_string()));
        self
    }

    /// Finalizes and returns the [`Module`]
    pub fn build(self) -> Module {
        Module {
            name: self.name,
            guard: self.guard,
            self_include: self.self_include,
            header: self.header,
            source: self.source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    fn defn(name: &str) -> Function {
        FunctionBuilder::new_with_str(name, Type::new(BaseType::Int).build())
            .param(ParameterBuilder::new_with_str("x", Type::new(BaseType::Int).build()).build())
            .body(
                BlockBuilder::new()
                    .statement(Statement::Return(Some(Expr::Ident("x".to_string()))))
                    .build(),
            )
            .build()
    }

    #[test]
    fn header_and_source_split() {
        let m = Module::new("math_utils")
            .header_include_system("stdint.h")
            .function(defn("identity"))
            .build();

        let header = m.header();
        assert_eq!(
            header,
            "#ifndef MATH_UTILS_H\n#define MATH_UTILS_H\n\n#include <stdint.h>\nint identity(int x);\n\n#endif /* MATH_UTILS_H */\n"
        );

        let source = m.source();
        assert_eq!(
            source,
            "#include \"math_utils.h\"\n\nint identity(int x) {\n  return x;\n}\n"
        );
    }

    #[test]
    fn static_function_stays_in_source() {
        let f = FunctionBuilder::new_with_str("helper", Type::new(BaseType::Void).build())
            .make_static()
            .body(BlockBuilder::new().build())
            .build();
        let m = Module::new("m").function(f).build();

        // header has no prototype for a static function
        assert!(!m.header().contains("helper"));
        assert!(m.source().contains("static void helper(void)"));
    }

    #[test]
    fn public_global_becomes_extern_in_header() {
        let v = VariableBuilder::new_with_str("counter", Type::new(BaseType::Int).build())
            .value(Expr::Int(0))
            .build();
        let m = Module::new("state").pragma_once().global(v).build();

        let header = m.header();
        assert_eq!(header, "#pragma once\n\nextern int counter;\n");

        let source = m.source();
        assert_eq!(source, "#include \"state.h\"\n\nint counter = 0;\n");
    }

    #[test]
    fn prototype_goes_to_header_only() {
        let proto = FunctionBuilder::new_with_str("f", Type::new(BaseType::Int).build()).build();
        let m = Module::new("m").no_self_include().function(proto).build();
        assert!(m.header().contains("int f(void);"));
        assert_eq!(m.source(), "");
    }
}
