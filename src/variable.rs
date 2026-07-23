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

//! This module provides tools for creating and managing C variables in Rust.
//!
//! It allows for programmatic generation of C variable declarations with support
//! for various attributes such as `static` and `extern`, initial values, types,
//! and documentation. This is particularly useful for generating C code or bindings
//! from Rust.

use std::fmt::{self, Write};

use crate::{
    Attribute, DocComment, Expr, Format, Formatter, StorageClass, Type, declare_with,
    has_annotations, write_annotations,
};
use tamacro::DisplayFromFormat;

/// Represents a C variable with its properties and attributes.
///
/// The `Variable` struct enables the creation of complete C variable declarations,
/// supporting type definitions, initial values, static and extern modifiers, and
/// documentation comments.
///
/// # Examples
///
/// Basic variable declaration:
/// ```c
/// int counter;
/// ```
///
/// Initialized static variable:
/// ```c
/// static float value = 3.14;
/// ```
///
/// External variable declaration:
/// ```c
/// extern char *message;
/// ```
#[derive(Debug, Clone, DisplayFromFormat)]
pub struct Variable {
    /// The name of the variable
    pub name: String,

    /// The type of the variable
    pub t: Type,

    /// The optional initial value of the variable
    pub value: Option<Expr>,

    /// The storage class (`static`, `extern`, or none). `static` and `extern`
    /// are mutually exclusive, so they share one field.
    pub storage: StorageClass,

    /// The attributes applied to the variable (e.g. `aligned`, `section`, `used`)
    pub attrs: Vec<Attribute>,

    /// Raw macro/specifier tokens emitted verbatim at the leading annotation slot.
    pub raw_attrs: Vec<String>,

    /// The optional documentation comment for the variable
    pub doc: Option<DocComment>,
}

impl Variable {
    /// Creates and returns a new `VariableBuilder` to construct a `Variable` using the builder pattern.
    ///
    /// This method simplifies the process of creating complex variable declarations
    /// by providing an interface for defining all variable properties.
    ///
    /// # Parameters
    ///
    /// * `name` - The name of the variable
    /// * `t` - The type of the variable
    ///
    /// # Returns
    ///
    /// A `VariableBuilder` instance that can be used to configure and build a `Variable`
    ///
    /// # Examples
    ///
    /// ```rust
    /// let var = Variable::new("message".to_string(), Type::new(BaseType::Char).make_pointer().build())
    ///     .value(Expr::Str("Hello".to_string()))
    ///     .build();
    /// ```
    pub fn new(name: String, t: Type) -> VariableBuilder {
        VariableBuilder::new(name, t)
    }

    /// Returns the type of the variable.
    ///
    /// # Returns
    ///
    /// A clone of the variable's `Type`
    ///
    /// # Examples
    ///
    /// ```rust
    /// let var = Variable::new("x".to_string(), Type::new(BaseType::Int).build()).build();
    /// let t = var.to_type();
    /// assert_eq!(t.to_string(), "int");
    /// ```
    pub fn to_type(&self) -> Type {
        self.t.clone()
    }
}

impl Format for Variable {
    fn format(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        if let Some(doc) = &self.doc {
            doc.format(fmt)?;
        }

        if has_annotations(&self.raw_attrs, &self.attrs) {
            write_annotations(fmt, &self.raw_attrs, &self.attrs)?;
            write!(fmt, " ")?;
        }

        if let Some(kw) = self.storage.keyword() {
            write!(fmt, "{kw} ")?;
        }

        write!(fmt, "{}", declare_with(&self.t, &self.name, fmt.options()))?;

        if !self.storage.is_extern()
            && let Some(value) = &self.value {
                write!(fmt, " = ")?;
                value.format(fmt)?;
            }

        Ok(())
    }
}

/// A builder for constructing a `Variable` instance with a fluent interface.
///
/// The `VariableBuilder` provides a step-by-step approach to creating complex C variables
/// by allowing incremental configuration of all variable attributes and properties.
pub struct VariableBuilder {
    name: String,
    t: Type,
    value: Option<Expr>,
    storage: StorageClass,
    attrs: Vec<Attribute>,
    raw_attrs: Vec<String>,
    doc: Option<DocComment>,
}

impl VariableBuilder {
    /// Creates and returns a new `VariableBuilder` with the specified name and type.
    ///
    /// # Parameters
    ///
    /// * `name` - The name of the variable as a String
    /// * `t` - The type of the variable
    ///
    /// # Returns
    ///
    /// A new `VariableBuilder` instance with default values for other properties
    ///
    /// # Examples
    ///
    /// ```rust
    /// let builder = VariableBuilder::new("count".to_string(), Type::new(BaseType::Int).build());
    /// ```
    pub fn new(name: String, t: Type) -> Self {
        Self {
            name,
            t,
            value: None,
            storage: StorageClass::None,
            attrs: vec![],
            raw_attrs: vec![],
            doc: None,
        }
    }

    /// Creates and returns a new `VariableBuilder` using a string slice for the name.
    ///
    /// This is a convenience method that converts the provided string slice to a String
    /// before delegating to the standard `new` method.
    ///
    /// # Parameters
    ///
    /// * `name` - The name of the variable as a string slice
    /// * `t` - The type of the variable
    ///
    /// # Returns
    ///
    /// A new `VariableBuilder` instance
    ///
    /// # Examples
    ///
    /// ```rust
    /// let builder = VariableBuilder::new_with_str("flag", Type::new(BaseType::Bool).build());
    /// ```
    pub fn new_with_str(name: &str, t: Type) -> Self {
        Self::new(name.to_string(), t)
    }

    /// Sets the initial value for the variable being built.
    ///
    /// # Parameters
    ///
    /// * `value` - The expression to initialize the variable with
    ///
    /// # Returns
    ///
    /// The builder instance for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// let builder = VariableBuilder::new_with_str("pi", Type::new(BaseType::Float).build())
    ///     .value(Expr::Float(3.14));
    /// ```
    pub fn value(mut self, value: Expr) -> Self {
        self.value = Some(value);
        self
    }

    /// Sets the documentation comment for the variable being built.
    ///
    /// # Parameters
    ///
    /// * `doc` - The documentation comment to attach to the variable
    ///
    /// # Returns
    ///
    /// The builder instance for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// let builder = VariableBuilder::new_with_str("size", Type::new(BaseType::Int).build())
    ///     .doc(DocComment::new("The size of the buffer").build());
    /// ```
    pub fn doc(mut self, doc: DocComment) -> Self {
        self.doc = Some(doc);
        self
    }

    /// Makes the variable static.
    ///
    /// In C, the `static` keyword limits the variable's scope to the file it is defined in
    /// and preserves its value between function calls if local.
    ///
    /// # Returns
    ///
    /// The builder instance for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// let builder = VariableBuilder::new_with_str("counter", Type::new(BaseType::Int).build())
    ///     .make_static();
    /// ```
    pub fn make_static(mut self) -> Self {
        // `static` and `extern` are mutually exclusive; the enum enforces that.
        self.storage = StorageClass::Static;
        self
    }

    /// Makes the variable extern.
    ///
    /// In C, the `extern` keyword indicates that the variable is defined elsewhere,
    /// typically in another compilation unit.
    ///
    /// # Returns
    ///
    /// The builder instance for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// let builder = VariableBuilder::new_with_str("global_flag", Type::new(BaseType::Bool).build())
    ///     .make_extern();
    /// ```
    pub fn make_extern(mut self) -> Self {
        self.storage = StorageClass::Extern;
        self
    }

    /// Sets the storage class explicitly.
    pub fn storage(mut self, storage: StorageClass) -> Self {
        self.storage = storage;
        self
    }

    /// Sets the initial value of the variable using a raw string.
    ///
    /// This is a convenience method for setting the value without constructing an `Expr` manually.
    ///
    /// # Parameters
    ///
    /// * `value` - The raw string value to initialize the variable with
    ///
    /// # Returns
    ///
    /// The builder instance for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// let builder = VariableBuilder::new_with_str("name", Type::new(BaseType::Char).make_pointer().build())
    ///     .raw_value("\"John\"".to_string());
    /// ```
    pub fn raw_value(mut self, value: String) -> Self {
        self.value = Some(Expr::Raw(value));
        self
    }

    /// Finalizes the variable definition and returns a fully constructed `Variable`.
    ///
    /// # Returns
    ///
    /// A fully constructed `Variable` instance
    ///
    /// # Examples
    ///
    /// ```rust
    /// let var = VariableBuilder::new_with_str("id", Type::new(BaseType::Int).build())
    ///     .value(Expr::Int(42))
    ///     .build();
    /// assert_eq!(var.to_string(), "int id = 42");
    /// ```
    /// Adds a single attribute (e.g. [`Attribute::aligned`]) to the variable.
    ///
    /// Variable attributes are emitted at the front of the declaration, e.g.
    /// `__attribute__((aligned(16))) static int buf;`.
    pub fn attr(mut self, attr: Attribute) -> Self {
        self.attrs.push(attr);
        self
    }

    /// Replaces the variable's attribute list.
    pub fn attrs(mut self, attrs: Vec<Attribute>) -> Self {
        self.attrs = attrs;
        self
    }

    /// Adds a raw macro/specifier token emitted verbatim at the leading
    /// annotation slot (like a thread-local or export macro)
    pub fn raw_attr(mut self, token: &str) -> Self {
        self.raw_attrs.push(token.to_string());
        self
    }

    /// Replaces the variable's raw annotation tokens.
    pub fn raw_attrs(mut self, tokens: Vec<String>) -> Self {
        self.raw_attrs = tokens;
        self
    }

    pub fn build(self) -> Variable {
        Variable {
            name: self.name,
            t: self.t,
            value: self.value,
            storage: self.storage,
            attrs: self.attrs,
            raw_attrs: self.raw_attrs,
            doc: self.doc,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn var() {
        let var = VariableBuilder::new_with_str(
            "some_var",
            TypeBuilder::new(BaseType::Char)
                .make_pointer()
                .make_const()
                .build(),
        )
        .value(Expr::Str("Hello, world".to_string()))
        .build();

        let res = "const char *some_var = \"Hello, world\"";

        assert_eq!(var.to_string(), res);

        let another_var =
            VariableBuilder::new_with_str("another_var", TypeBuilder::new(BaseType::Bool).build())
                .make_static()
                .build();

        let another_res = "static bool another_var";

        assert_eq!(another_var.to_string(), another_res);
    }

    #[test]
    fn extern_omits_initializer_and_storage_is_exclusive() {
        let ext = VariableBuilder::new_with_str("g", TypeBuilder::new(BaseType::Int).build())
            .value(Expr::Int(5))
            .make_extern()
            .build();
        assert_eq!(ext.storage, StorageClass::Extern);
        assert_eq!(ext.to_string(), "extern int g");

        let stat = VariableBuilder::new_with_str("g", TypeBuilder::new(BaseType::Int).build())
            .value(Expr::Int(5))
            .make_extern()
            .make_static()
            .build();
        assert_eq!(stat.storage, StorageClass::Static);
        assert_eq!(stat.to_string(), "static int g = 5");
    }
}
