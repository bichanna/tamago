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

//! This module provides means to create C structs programmatically in Rust.
//!
//! It offers a builder pattern approach for defining C-compatible struct types
//! with their fields, documentation, and other attributes. This is particularly
//! useful for generating C header files or FFI bindings.
//!
//! Fields may be ordinary typed members, bitfields, or inline C11 anonymous
//! `struct`/`union` members (see [`Field::anonymous_union`] and
//! [`Field::anonymous_struct`]) — the latter being how tagged unions are
//! modeled. Structs and fields can also carry [`Attribute`]s such as `packed`
//! and `aligned`.

use std::fmt::{self, Write};

use crate::{
    Attribute, BaseType, DocComment, Format, Formatter, Type, declare_with, has_annotations,
    write_annotations,
};
use tamacro::DisplayFromFormat;

/// Whether an inline anonymous aggregate member is a `struct` or a `union`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateKind {
    /// An inline `struct { ... }`.
    Struct,

    /// An inline `union { ... }`.
    Union,
}

impl AggregateKind {
    fn keyword(self) -> &'static str {
        match self {
            AggregateKind::Struct => "struct",
            AggregateKind::Union => "union",
        }
    }
}

/// An inline anonymous aggregate used as the "type" of a [`Field`].
///
/// This is how C11 anonymous structs and unions are modeled: a field whose
/// contents are an inline aggregate rather than a named type. When the field
/// also has no name, its members are injected into the enclosing aggregate's
/// scope — exactly what you want for lowering tagged unions:
///
/// ```c
/// struct Value {
///   Tag tag;
///   union {   // anonymous: write v.i, not v.payload.i
///     int i;
///     double d;
///   };
/// };
/// ```
#[derive(Debug, Clone)]
pub struct AnonAggregate {
    /// Whether this is a `struct` or a `union`.
    pub kind: AggregateKind,

    /// The members of the inline aggregate.
    pub fields: Vec<Field>,
}

/// Represents a struct in C.
///
/// This struct holds all the information needed to represent a C struct,
/// including its name, fields, and documentation.
///
/// # Examples
///
/// A C struct representation:
/// ```c
/// struct Person {
///   char *name;
///   int age;
/// };
/// ```
///
/// Creating this struct using the builder pattern:
/// ```rust
/// use crate::{Struct, Field, Type, BaseType, DocComment};
///
/// let person = Struct::new("Person".to_string())
///     .field(Field::new("name".to_string(), Type::new(BaseType::Char).make_pointer().build()))
///     .field(Field::new("age".to_string(), Type::new(BaseType::Int).build()))
///     .build();
/// ```
#[derive(Debug, Clone, DisplayFromFormat)]
pub struct Struct {
    /// The name of the struct
    name: String,

    /// The struct's body: `None` is a forward declaration (`struct Foo;`),
    /// `Some(fields)` is a definition (`struct Foo { ... };`) — including
    /// `Some(vec![])` for an explicitly empty definition. This disambiguates an
    /// opaque/forward declaration from a (possibly empty) definition, which a
    /// bare `Vec<Field>` could not.
    body: Option<Vec<Field>>,

    /// The attributes applied to the struct (e.g. `packed`, `aligned`)
    attrs: Vec<Attribute>,

    /// Raw macro/specifier tokens emitted verbatim after the `struct` keyword.
    raw_attrs: Vec<String>,

    /// The doc comment of the struct
    doc: Option<DocComment>,
}

impl Struct {
    /// Creates and returns a new `StructBuilder` to construct a `Struct` using the builder
    /// pattern.
    ///
    /// # Parameters
    /// * `name` - The name to be given to the struct
    ///
    /// # Returns
    /// A new `StructBuilder` instance initialized with the given name
    ///
    /// # Examples
    /// ```rust
    /// let person_struct = Struct::new("Person".to_string())
    ///     .field(Field::new("name".to_string(), Type::new(BaseType::Char).make_pointer().build()))
    ///     .field(Field::new("age".to_string(), Type::new(BaseType::Int).build()))
    ///     .build();
    ///
    /// println!("{}", person_struct);
    /// // Outputs:
    /// // struct Person {
    /// //   char *name;
    /// //   int age;
    /// // };
    /// ```
    pub fn new(name: String) -> StructBuilder {
        StructBuilder::new(name)
    }

    /// Returns the type representation of the struct.
    ///
    /// This allows using a struct definition as a type for fields or function parameters.
    ///
    /// # Returns
    /// A `Type` instance representing this struct type
    ///
    /// # Examples
    /// ```rust
    /// let person_struct = Struct::new("Person".to_string())
    ///     .field(Field::new("name".to_string(), Type::new(BaseType::Char).make_pointer().build()))
    ///     .build();
    ///
    /// // Now use this struct as a type for another field
    /// let person_field = Field::new("person".to_string(), person_struct.to_type())
    ///     .build();
    /// ```
    pub fn to_type(&self) -> Type {
        Type::new(BaseType::Struct(self.name.clone())).build()
    }
}

impl Format for Struct {
    fn format(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        if let Some(doc) = &self.doc {
            doc.format(fmt)?;
        }

        write!(fmt, "struct")?;

        if has_annotations(&self.raw_attrs, &self.attrs) {
            write!(fmt, " ")?;
            write_annotations(fmt, &self.raw_attrs, &self.attrs)?;
        }

        write!(fmt, " {}", self.name)?;

        if let Some(fields) = &self.body {
            fmt.block(|fmt| {
                for field in fields {
                    field.format(fmt)?;
                }
                Ok(())
            })?;
        }

        writeln!(fmt, ";")
    }
}

/// A builder for constructing a `Struct` instance.
///
/// This builder implements the builder pattern for creating struct
/// definitions with a fluent interface.
pub struct StructBuilder {
    name: String,
    fields: Vec<Field>,
    force_forward: bool,
    force_body: bool,
    attrs: Vec<Attribute>,
    raw_attrs: Vec<String>,
    doc: Option<DocComment>,
}

impl StructBuilder {
    /// Creates and returns a new `StructBuilder` to construct a `Struct` using the builder
    /// pattern.
    ///
    /// # Parameters
    /// * `name` - The name to be given to the struct
    ///
    /// # Returns
    /// A new `StructBuilder` instance initialized with the given name
    ///
    /// # Examples
    /// ```rust
    /// let builder = StructBuilder::new("Person".to_string());
    /// let person_struct = builder
    ///     .field(Field::new("name".to_string(), Type::new(BaseType::Char).make_pointer().build()))
    ///     .field(Field::new("age".to_string(), Type::new(BaseType::Int).build()))
    ///     .build();
    /// ```
    pub fn new(name: String) -> Self {
        Self {
            name,
            fields: vec![],
            force_forward: false,
            force_body: false,
            attrs: vec![],
            raw_attrs: vec![],
            doc: None,
        }
    }

    /// Creates and returns a new `StructBuilder` construct a `Struct` with the given name string
    /// slice using the builder pattern.
    ///
    /// This is a convenience method that converts a string slice to a `String`.
    ///
    /// # Parameters
    /// * `name` - The name of the struct as a string slice
    ///
    /// # Returns
    /// A new `StructBuilder` instance initialized with the given name
    ///
    /// # Examples
    /// ```rust
    /// let person_struct = StructBuilder::new_with_str("Person")
    ///     .field(Field::new_with_str("name", Type::new(BaseType::Char).make_pointer().build()))
    ///     .field(Field::new_with_str("age", Type::new(BaseType::Int).build()))
    ///     .build();
    /// ```
    pub fn new_with_str(name: &str) -> Self {
        Self::new(name.to_string())
    }

    /// Sets the optional doc comment for the struct and returns the builder for more chaining.
    ///
    /// # Parameters
    /// * `doc` - A `DocComment` instance to be associated with the struct
    ///
    /// # Returns
    /// The builder instance for method chaining
    pub fn doc(mut self, doc: DocComment) -> Self {
        self.doc = Some(doc);
        self
    }

    /// Appends a struct field to the struct being built and returns the builder for more chaining.
    ///
    /// # Parameters
    /// * `field` - A `Field` instance to be added to the struct
    ///
    /// # Returns
    /// The builder instance for method chaining
    ///
    /// # Examples
    /// ```rust
    /// let name_field = Field::new_with_str("name", Type::new(BaseType::Char).make_pointer().build());
    /// let age_field = Field::new_with_str("age", Type::new(BaseType::Int).build());
    ///
    /// let person_struct = StructBuilder::new_with_str("Person")
    ///     .field(name_field)
    ///     .field(age_field)
    ///     .build();
    /// ```
    pub fn field(mut self, field: Field) -> Self {
        self.fields.push(field);
        self
    }

    /// Sets the struct fields of the struct being built and returns the builder for more chaining.
    ///
    /// This method replaces any existing fields with the provided vector of fields.
    ///
    /// # Parameters
    /// * `fields` - A vector of `Field` instances to be added to the struct
    ///
    /// # Returns
    /// The builder instance for method chaining
    ///
    /// # Examples
    /// ```rust
    /// let fields = vec![
    ///     Field::new_with_str("name", Type::new(BaseType::Char).make_pointer().build()),
    ///     Field::new_with_str("age", Type::new(BaseType::Int).build())
    /// ];
    ///
    /// let person_struct = StructBuilder::new_with_str("Person")
    ///     .fields(fields)
    ///     .build();
    /// ```
    pub fn fields(mut self, fields: Vec<Field>) -> Self {
        self.fields = fields;
        self
    }

    /// Marks this as a forward declaration/opaque type, emitting `struct Foo;`
    /// with no body even if fields were added.
    ///
    /// ```rust
    /// let fwd = StructBuilder::new_with_str("Opaque").forward_declaration().build();
    /// assert_eq!(fwd.to_string(), "struct Opaque;\n");
    /// ```
    pub fn forward_declaration(mut self) -> Self {
        self.force_forward = true;
        self
    }

    /// Forces a (possibly empty) definition rather than a forward declaration.
    ///
    /// A struct with no fields normally renders as a forward declaration
    /// (`struct Foo;`). Call this to instead emit an empty definition
    /// (`struct Foo {\n};`) — note that an empty struct body is a GNU/C++
    /// extension and is not valid ISO C.
    pub fn define(mut self) -> Self {
        self.force_body = true;
        self
    }

    /// Adds a single attribute (e.g. [`Attribute::packed`]) to the struct.
    ///
    /// Struct attributes are emitted right after the `struct` keyword, e.g.
    /// `struct __attribute__((packed)) Name { ... };`.
    pub fn attr(mut self, attr: Attribute) -> Self {
        self.attrs.push(attr);
        self
    }

    /// Replaces the struct's attribute list.
    pub fn attrs(mut self, attrs: Vec<Attribute>) -> Self {
        self.attrs = attrs;
        self
    }

    /// Adds a raw macro/specifier token emitted verbatim after the `struct`
    /// keyword (e.g. a `MYLANG_PACKED` macro).
    pub fn raw_attr(mut self, token: &str) -> Self {
        self.raw_attrs.push(token.to_string());
        self
    }

    /// Replaces the struct's raw annotation tokens.
    pub fn raw_attrs(mut self, tokens: Vec<String>) -> Self {
        self.raw_attrs = tokens;
        self
    }

    /// Consumes the builder and returns a `Struct` containing all the fields.
    ///
    /// # Returns
    /// A fully constructed `Struct` instance
    pub fn build(self) -> Struct {
        let body = if self.force_forward {
            None
        } else if self.fields.is_empty() && !self.force_body {
            None
        } else {
            Some(self.fields)
        };

        Struct {
            name: self.name,
            body,
            attrs: self.attrs,
            raw_attrs: self.raw_attrs,
            doc: self.doc,
        }
    }
}

/// Represents a struct field in C.
///
/// This struct holds all the information needed to represent a field
/// within a C struct, including its name, type, bitfield width (if any),
/// and documentation.
#[derive(Debug, Clone, DisplayFromFormat)]
pub struct Field {
    /// The name of the field, or `None` for an anonymous member (an anonymous
    /// struct/union, or an unnamed bitfield used for padding).
    pub name: Option<String>,

    /// The type of the field. Ignored when [`anon`](Field::anon) is `Some`.
    pub t: Type,

    /// When `Some`, the field *is* an inline anonymous aggregate (C11) rather
    /// than a field of a named type; [`t`](Field::t) is then unused.
    pub anon: Option<AnonAggregate>,

    /// The number of bits in the bitfield, if this is a bitfield
    pub width: Option<u8>,

    /// The attributes applied to the field (e.g. `aligned`, `deprecated`)
    pub attrs: Vec<Attribute>,

    /// Raw macro/specifier tokens emitted verbatim after the declarator.
    pub raw_attrs: Vec<String>,

    /// The doc comment
    pub doc: Option<DocComment>,
}

impl Field {
    /// Creates and returns a new `FieldBuilder` to construct a `Field` using the builder pattern.
    ///
    /// # Parameters
    /// * `name` - The name of the field
    /// * `t` - The type of the field
    ///
    /// # Returns
    /// A new `FieldBuilder` instance initialized with the given name and type
    ///
    /// # Examples
    /// ```rust
    /// let name_field = Field::new("name".to_string(), Type::new(BaseType::Char).make_pointer().build())
    ///     .build();
    ///
    /// let age_field = Field::new("age".to_string(), Type::new(BaseType::Int).build())
    ///     .build();
    /// ```
    pub fn new(name: String, t: Type) -> FieldBuilder {
        FieldBuilder::new(name, t)
    }

    /// Creates a builder for an inline anonymous `struct` member (C11).
    ///
    /// With no name, the members are injected into the enclosing aggregate's
    /// scope. Give it a name with [`FieldBuilder::member_name`] to instead
    /// produce a named inline aggregate (`struct { ... } name;`).
    ///
    /// # Examples
    /// ```rust
    /// let anon = Field::anonymous_struct(vec![
    ///     FieldBuilder::new_with_str("x", Type::new(BaseType::Int).build()).build(),
    ///     FieldBuilder::new_with_str("y", Type::new(BaseType::Int).build()).build(),
    /// ])
    /// .build();
    /// ```
    pub fn anonymous_struct(fields: Vec<Field>) -> FieldBuilder {
        FieldBuilder::anonymous(AggregateKind::Struct, fields)
    }

    /// Creates a builder for an inline anonymous `union` member (C11).
    ///
    /// This is the workhorse for tagged-union / sum-type lowering.
    ///
    /// # Examples
    /// ```rust
    /// let payload = Field::anonymous_union(vec![
    ///     FieldBuilder::new_with_str("i", Type::new(BaseType::Int).build()).build(),
    ///     FieldBuilder::new_with_str("d", Type::new(BaseType::Double).build()).build(),
    /// ])
    /// .build();
    /// ```
    pub fn anonymous_union(fields: Vec<Field>) -> FieldBuilder {
        FieldBuilder::anonymous(AggregateKind::Union, fields)
    }

    /// Returns the type of the field.
    ///
    /// # Returns
    /// A clone of the field's type
    ///
    /// # Examples
    /// ```rust
    /// let field = Field::new("count".to_string(), Type::new(BaseType::Int).build())
    ///     .build();
    ///
    /// let field_type = field.to_type();
    /// assert_eq!(field_type.to_string(), "int");
    /// ```
    pub fn to_type(&self) -> Type {
        self.t.clone()
    }
}

impl Format for Field {
    fn format(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        if let Some(doc) = &self.doc {
            doc.format(fmt)?;
        }

        match &self.anon {
            None => {
                write!(
                    fmt,
                    "{}",
                    declare_with(&self.t, self.name.as_deref().unwrap_or(""), fmt.options())
                )?;

                if let Some(w) = self.width {
                    write!(fmt, " : {w}")?;
                }
            }
            Some(agg) => {
                write!(fmt, "{}", agg.kind.keyword())?;
                fmt.block(|fmt| {
                    for field in &agg.fields {
                        field.format(fmt)?;
                    }
                    Ok(())
                })?;

                if let Some(name) = &self.name {
                    write!(fmt, " {name}")?;
                }
            }
        }

        if has_annotations(&self.raw_attrs, &self.attrs) {
            write!(fmt, " ")?;
            write_annotations(fmt, &self.raw_attrs, &self.attrs)?;
        }

        writeln!(fmt, ";")
    }
}

/// A builder for constructing a `Field` instance.
///
/// This builder implements the builder pattern for creating struct
/// field definitions with a fluent interface.
pub struct FieldBuilder {
    name: Option<String>,
    t: Type,
    anon: Option<AnonAggregate>,
    width: Option<u8>,
    attrs: Vec<Attribute>,
    raw_attrs: Vec<String>,
    doc: Option<DocComment>,
}

impl FieldBuilder {
    /// Creates and returns a new `FieldBuilder` to construct a `Field` using the builder pattern.
    ///
    /// # Parameters
    /// * `name` - The name of the field
    /// * `t` - The type of the field
    ///
    /// # Returns
    /// A new `FieldBuilder` instance initialized with the given name and type
    ///
    /// # Examples
    /// ```rust
    /// let builder = FieldBuilder::new("name".to_string(), Type::new(BaseType::Char).make_pointer().build());
    /// let name_field = builder.build();
    ///
    /// // Or in a single chain:
    /// let age_field = FieldBuilder::new("age".to_string(), Type::new(BaseType::Int).build())
    ///     .build();
    /// ```
    pub fn new(name: String, t: Type) -> Self {
        Self {
            name: Some(name),
            t,
            anon: None,
            width: None,
            attrs: vec![],
            raw_attrs: vec![],
            doc: None,
        }
    }

    /// Creates a builder for an inline anonymous aggregate member. Prefer the
    /// [`Field::anonymous_struct`] / [`Field::anonymous_union`] entry points.
    pub fn anonymous(kind: AggregateKind, fields: Vec<Field>) -> Self {
        Self {
            name: None,
            t: Type::new(BaseType::Void).build(),
            anon: Some(AnonAggregate { kind, fields }),
            width: None,
            attrs: vec![],
            raw_attrs: vec![],
            doc: None,
        }
    }

    /// Creates and returns a new `FieldBuilder` to construct a `Field` with the given name string
    /// slice using the builder pattern.
    ///
    /// This is a convenience method that converts a string slice to a `String`.
    ///
    /// # Parameters
    /// * `name` - The name of the field as a string slice
    /// * `t` - The type of the field
    ///
    /// # Returns
    /// A new `FieldBuilder` instance initialized with the given name and type
    ///
    /// # Examples
    /// ```rust
    /// let name_field = FieldBuilder::new_with_str("name", Type::new(BaseType::Char).make_pointer().build())
    ///     .build();
    ///
    /// let age_field = FieldBuilder::new_with_str("age", Type::new(BaseType::Int).build())
    ///     .build();
    /// ```
    pub fn new_with_str(name: &str, t: Type) -> Self {
        Self::new(name.to_string(), t)
    }

    /// Sets the optional doc comment for the field and returns the builder for more chaining.
    ///
    /// # Parameters
    /// * `doc` - A `DocComment` instance to be associated with the field
    ///
    /// # Returns
    /// The builder instance for method chaining
    pub fn doc(mut self, doc: DocComment) -> Self {
        self.doc = Some(doc);
        self
    }

    /// Sets the optional bit width for the field and returns the builder for more chaining.
    ///
    /// When specified, this indicates that the field is a bitfield with the given width.
    ///
    /// # Parameters
    /// * `width` - The number of bits to allocate for this bitfield
    ///
    /// # Returns
    /// The builder instance for method chaining
    ///
    /// # Examples
    /// ```rust
    /// // Create a 1-bit flag field
    /// let flag_field = FieldBuilder::new_with_str("is_active", Type::new(BaseType::Bool).build())
    ///     .bitfield_width(1)
    ///     .build();
    ///
    /// // Create a 4-bit enum field that can store values 0-15
    /// let type_field = FieldBuilder::new_with_str("type", Type::new(BaseType::UInt8).build())
    ///     .bitfield_width(4)
    ///     .build();
    /// ```
    pub fn bitfield_width(mut self, width: u8) -> Self {
        self.width = Some(width);
        self
    }

    /// Gives an inline anonymous aggregate a member name, turning
    /// `struct { ... };` into `struct { ... } name;`. Also usable to (re)name any
    /// field.
    pub fn member_name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    /// Adds a single attribute (e.g. [`Attribute::aligned`]) to the field.
    ///
    /// Field attributes are emitted after the declarator (and bitfield width),
    /// e.g. `int x __attribute__((aligned(8)));`.
    pub fn attr(mut self, attr: Attribute) -> Self {
        self.attrs.push(attr);
        self
    }

    /// Replaces the field's attribute list.
    pub fn attrs(mut self, attrs: Vec<Attribute>) -> Self {
        self.attrs = attrs;
        self
    }

    /// Adds a raw macro/specifier token emitted verbatim after the declarator.
    pub fn raw_attr(mut self, token: &str) -> Self {
        self.raw_attrs.push(token.to_string());
        self
    }

    /// Replaces the field's raw annotation tokens.
    pub fn raw_attrs(mut self, tokens: Vec<String>) -> Self {
        self.raw_attrs = tokens;
        self
    }

    /// Consumes the builder and returns a `Field` containing all the information.
    ///
    /// # Returns
    /// A fully constructed `Field` instance
    ///
    /// # Examples
    /// ```rust
    /// let field = FieldBuilder::new_with_str("name", Type::new(BaseType::Char).make_pointer().build())
    ///     .doc(DocComment::new().line_str("The person's full name").build())
    ///     .build();
    ///
    /// println!("{}", field);
    /// // Output:
    /// // /// The person's full name
    /// // char *name;
    /// ```
    pub fn build(self) -> Field {
        Field {
            name: self.name,
            t: self.t,
            anon: self.anon,
            width: self.width,
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
    fn field() {
        let f = FieldBuilder::new_with_str("some_field", Type::new(BaseType::Char).build())
            .doc(DocComment::new().line_str("Hello").build())
            .build();
        let res = r#"/// Hello
char some_field;
"#;

        assert_eq!(f.to_string(), res);

        let f2 = FieldBuilder::new_with_str("another_field", Type::new(BaseType::Bool).build())
            .bitfield_width(1)
            .build();
        let res2 = "bool another_field : 1;\n";

        assert_eq!(f2.to_string(), res2);
    }

    #[test]
    fn structs() {
        let s = StructBuilder::new_with_str("Person")
            .fields(vec![
                FieldBuilder::new_with_str(
                    "name",
                    Type::new(BaseType::Char).make_pointer().build(),
                )
                .build(),
                FieldBuilder::new_with_str("age", Type::new(BaseType::UInt8).build()).build(),
            ])
            .build();
        let res = r#"struct Person {
  char *name;
  uint8_t age;
};
"#;

        assert_eq!(s.to_string(), res);
    }

    #[test]
    fn anonymous_union_member() {
        // tagged union
        let s = StructBuilder::new_with_str("Value")
            .field(FieldBuilder::new_with_str("tag", Type::new(BaseType::Int).build()).build())
            .field(
                Field::anonymous_union(vec![
                    FieldBuilder::new_with_str("i", Type::new(BaseType::Int).build()).build(),
                    FieldBuilder::new_with_str("d", Type::new(BaseType::Double).build()).build(),
                ])
                .build(),
            )
            .build();
        let res = r#"struct Value {
  int tag;
  union {
    int i;
    double d;
  };
};
"#;
        assert_eq!(s.to_string(), res);
    }

    #[test]
    fn named_inline_aggregate() {
        // struct { int x; int y; } point;
        let f = Field::anonymous_struct(vec![
            FieldBuilder::new_with_str("x", Type::new(BaseType::Int).build()).build(),
            FieldBuilder::new_with_str("y", Type::new(BaseType::Int).build()).build(),
        ])
        .member_name("point")
        .build();
        let res = r#"struct {
  int x;
  int y;
} point;
"#;
        assert_eq!(f.to_string(), res);
    }

    #[test]
    fn struct_and_field_attributes() {
        let s = StructBuilder::new_with_str("Header")
            .attr(Attribute::packed())
            .field(
                FieldBuilder::new_with_str("magic", Type::new(BaseType::UInt32).build())
                    .attr(Attribute::aligned(4))
                    .build(),
            )
            .build();
        let res = r#"struct __attribute__((packed)) Header {
  uint32_t magic __attribute__((aligned(4)));
};
"#;
        assert_eq!(s.to_string(), res);

        // same struct rendered in C23 attribute style
        let c23 = render(
            &s,
            RenderOptions {
                attr_style: AttrStyle::C23,
                ..Default::default()
            },
        );
        let expected = r#"struct [[gnu::packed]] Header {
  uint32_t magic [[gnu::aligned(4)]];
};
"#;
        assert_eq!(c23, expected);
    }

    #[test]
    fn raw_annotations() {
        let s = StructBuilder::new_with_str("Regs")
            .raw_attr("MYLANG_PACKED")
            .field(
                FieldBuilder::new_with_str("flags", Type::new(BaseType::UInt32).build())
                    .raw_attr("MYLANG_ALIGN4")
                    .build(),
            )
            .build();
        let res = r#"struct MYLANG_PACKED Regs {
  uint32_t flags MYLANG_ALIGN4;
};
"#;
        assert_eq!(s.to_string(), res);

        // raw tokens precede a typed attribute group in the same slot
        let s2 = StructBuilder::new_with_str("S")
            .raw_attr("MYLANG_PACKED")
            .attr(Attribute::aligned(8))
            .build();
        assert_eq!(
            s2.to_string(),
            "struct MYLANG_PACKED __attribute__((aligned(8))) S;\n"
        );
    }

    #[test]
    fn forward_declaration_vs_empty_definition() {
        // no fields defaults to a forward declaration (backwards compatible)
        let fwd = StructBuilder::new_with_str("Opaque").build();
        assert_eq!(fwd.to_string(), "struct Opaque;\n");

        // ...and can be requested explicitly, even with fields present
        let forced = StructBuilder::new_with_str("Opaque")
            .field(FieldBuilder::new_with_str("x", Type::new(BaseType::Int).build()).build())
            .forward_declaration()
            .build();
        assert_eq!(forced.to_string(), "struct Opaque;\n");

        // nn explicit (GNU/C++) empty definition is now expressible and distinct
        let empty = StructBuilder::new_with_str("Empty").define().build();
        assert_eq!(empty.to_string(), "struct Empty {\n};\n");

        // a normal definition is unaffected :)
        let normal = StructBuilder::new_with_str("P")
            .field(FieldBuilder::new_with_str("x", Type::new(BaseType::Int).build()).build())
            .build();
        assert_eq!(normal.to_string(), "struct P {\n  int x;\n};\n");
    }
}
