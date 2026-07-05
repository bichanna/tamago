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

//! This module provides tools for expressing and managing C types in Rust.
//!
//! It defines structures and utilities to represent C base types, type qualifiers,
//! and complex type constructions such as pointers, arrays, and function types.
//! This is particularly useful for generating C code or bindings programmatically
//! from Rust.
//!
//! # Type representation
//!
//! C declarations read "inside-out": in `int (*p)[10]`, `p` is a pointer to an
//! array of ten `int`s, not an array of pointers. To model this faithfully,
//! [`Type`] is a *recursive* description of a derived type, and rendering a
//! declaration threads the identifier through it with the classic declarator
//! algorithm (see [`declare`]). This is what makes pointer-to-array,
//! array-of-pointers, function pointers, and pointer-level `const` all
//! expressible.
//!
//! The [`TypeBuilder`] fluent API (`Type::new(base).make_pointer().make_array(n)`)
//! is retained as a thin facade over the recursive form for the common cases;
//! reach for the [`Type::ptr`], [`Type::array`], and [`Type::func`] constructors
//! (or the enum variants directly) when you need the full generality.

use std::fmt::{self, Write};

use crate::{Expr, Format, Formatter};
use tamacro::DisplayFromFormat;

/// Represents all base types used in C.
///
/// The `BaseType` enum encapsulates the fundamental types in C, including primitive
/// types like `int` and `float`, sized integer types from `stdint.h`, and aggregate
/// types like `enum`, `struct`, and `union`. It serves as the foundation for building
/// more complex types.
///
/// # Examples
///
/// Basic integer type:
/// ```c
/// int
/// ```
///
/// Sized unsigned integer:
/// ```c
/// uint32_t
/// ```
///
/// Struct type:
/// ```c
/// struct Point
/// ```
#[derive(Debug, Clone, DisplayFromFormat)]
pub enum BaseType {
    /// Represents the `void` type.
    Void,

    /// Represents the `double` type, double precision floating point number.
    Double,

    /// Represents the `float` type, single precision floating point number.
    Float,

    /// Represents the `char` type, a single byte character.
    Char,

    /// Represents the `int` type.
    Int,

    /// Represents the `uint8_t` type from `stdint.h`, an unsigned 8-bit integer.
    UInt8,

    /// Represents the `uint16_t` type from `stdint.h`, an unsigned 16-bit integer.
    UInt16,

    /// Represents the `uint32_t` type from `stdint.h`, an unsigned 32-bit integer.
    UInt32,

    /// Represents the `uint64_t` type from `stdint.h`, an unsigned 64-bit integer.
    UInt64,

    /// Represents the `int8_t` type from `stdint.h`, a signed 8-bit integer.
    Int8,

    /// Represents the `int16_t` type from `stdint.h`, a signed 16-bit integer.
    Int16,

    /// Represents the `int32_t` type from `stdint.h`, a signed 32-bit integer.
    Int32,

    /// Represents the `int64_t` type from `stdint.h`, a signed 64-bit integer.
    Int64,

    /// Represents the `size_t` type from `stddef.h`.
    Size,

    /// Represents the `uintptr_t` type from `stdint.h`.
    UIntPtr,

    /// Represents the `bool` type from `stdbool.h`.
    Bool,

    /// An enumeration type.
    Enum(String),

    /// A struct type.
    Struct(String),

    /// A union type.
    Union(String),

    /// `typedef`
    TypeDef(String),
}

impl BaseType {
    /// Creates a new unsigned integer with the given bit size.
    ///
    /// This method maps the provided bit size to the corresponding unsigned integer type:
    /// - 8 -> `UInt8`
    /// - 16 -> `UInt16`
    /// - 32 -> `UInt32`
    /// - 64 -> `UInt64`
    /// Any other value defaults to `UInt64`.
    ///
    /// # Parameters
    ///
    /// * `bits` - The size of the unsigned integer in bits
    ///
    /// # Returns
    ///
    /// A `BaseType` instance representing the specified unsigned integer type
    ///
    /// # Examples
    ///
    /// ```rust
    /// let uint_type = BaseType::new_uint(32);
    /// assert_eq!(uint_type.to_string(), "uint32_t");
    /// ```
    pub fn new_uint(bits: u8) -> Self {
        use BaseType::*;
        match bits {
            8 => UInt8,
            16 => UInt16,
            32 => UInt32,
            64 => UInt64,
            _ => UInt64,
        }
    }

    /// Creates a new signed integer with the given bit size.
    ///
    /// This method maps the provided bit size to the corresponding signed integer type:
    /// - 8 -> `Int8`
    /// - 16 -> `Int16`
    /// - 32 -> `Int32`
    /// - 64 -> `Int64`
    /// Any other value defaults to `Int64`.
    ///
    /// # Parameters
    ///
    /// * `bits` - The size of the signed integer in bits
    ///
    /// # Returns
    ///
    /// A `BaseType` instance representing the specified signed integer type
    ///
    /// # Examples
    ///
    /// ```rust
    /// let int_type = BaseType::new_int(16);
    /// assert_eq!(int_type.to_string(), "int16_t");
    /// ```
    pub fn new_int(bits: u8) -> Self {
        use BaseType::*;
        match bits {
            8 => Int8,
            16 => Int16,
            32 => Int32,
            64 => Int64,
            _ => Int64,
        }
    }

    /// Checks whether the type is an integer type.
    ///
    /// This includes all signed and unsigned integer types, `char`, `size_t`, `uintptr_t`, and `bool`.
    ///
    /// # Returns
    ///
    /// `true` if the type is an integer type, `false` otherwise
    ///
    /// # Examples
    ///
    /// ```rust
    /// let int_type = BaseType::Int;
    /// assert!(int_type.is_integer());
    /// let float_type = BaseType::Float;
    /// assert!(!float_type.is_integer());
    /// ```
    pub fn is_integer(&self) -> bool {
        use BaseType::*;
        matches!(
            self,
            Int | UInt8
                | UInt16
                | UInt32
                | UInt64
                | Int8
                | Int16
                | Int32
                | Int64
                | Size
                | UIntPtr
                | Bool
                | Char
        )
    }

    /// Checks whether the type is a tag type (`enum`, `struct`, `union`, or `typedef`).
    ///
    /// # Returns
    ///
    /// `true` if the type is a tag type, `false` otherwise
    ///
    /// # Examples
    ///
    /// ```rust
    /// let struct_type = BaseType::Struct("Point".to_string());
    /// assert!(struct_type.is_tag_type());
    /// let int_type = BaseType::Int;
    /// assert!(!int_type.is_tag_type());
    /// ```
    pub fn is_tag_type(&self) -> bool {
        use BaseType::*;
        matches!(self, Enum(_) | Struct(_) | Union(_) | TypeDef(_))
    }
}

impl Format for BaseType {
    fn format(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        use BaseType::*;
        match self {
            Void => write!(fmt, "void"),
            Double => write!(fmt, "double"),
            Float => write!(fmt, "float"),
            Char => write!(fmt, "char"),
            Int => write!(fmt, "int"),
            UInt8 => write!(fmt, "uint8_t"),
            UInt16 => write!(fmt, "uint16_t"),
            UInt32 => write!(fmt, "uint32_t"),
            UInt64 => write!(fmt, "uint64_t"),
            Int8 => write!(fmt, "int8_t"),
            Int16 => write!(fmt, "int16_t"),
            Int32 => write!(fmt, "int32_t"),
            Int64 => write!(fmt, "int64_t"),
            Size => write!(fmt, "size_t"),
            UIntPtr => write!(fmt, "uintptr_t"),
            Bool => write!(fmt, "bool"),
            Enum(s) => write!(fmt, "enum {s}"),
            Struct(s) => write!(fmt, "struct {s}"),
            Union(s) => write!(fmt, "union {s}"),
            TypeDef(s) => write!(fmt, "{s}"),
        }
    }
}

/// Represents type qualifiers in C.
///
/// The `TypeQualifier` enum encapsulates C's type qualifiers `const` and `volatile`,
/// which modify the behavior of variables or pointers in a C program.
///
/// # Examples
///
/// Const qualifier:
/// ```c
/// const int
/// ```
///
/// Volatile qualifier:
/// ```c
/// volatile char
/// ```
#[derive(Debug, Clone, Copy, DisplayFromFormat)]
pub enum TypeQualifier {
    /// The `volatile` keyword, indicating the variable may change unexpectedly.
    Volatile,

    /// The `const` keyword, indicating the variable's value cannot be modified after initialization.
    Const,
}

impl Format for TypeQualifier {
    fn format(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        use TypeQualifier::*;
        match self {
            Volatile => write!(fmt, "volatile"),
            Const => write!(fmt, "const"),
        }
    }
}

/// A recursive description of a C type, built out of a base type wrapped in any
/// number of pointer, array, and function derivations.
///
/// Because C declarations bind inside-out, a `Type` cannot be printed
/// independently of the identifier it declares — the name has to be threaded
/// through the derivations. Use [`declare`] (or [`Type::declarator`]) to render a
/// full declaration, and the [`Format`]/`Display` impls to render the *abstract*
/// declarator (the type with an empty name, as used in casts and `sizeof`).
///
/// # Examples
///
/// Pointer to const char (`const char *`):
/// ```rust
/// let t = Type::const_ptr(Type::base(BaseType::Char));
/// assert_eq!(t.to_string(), "const char *");
/// ```
///
/// Pointer to an array of ten ints (`int (*)[10]`):
/// ```rust
/// let t = Type::ptr(Type::array(
///     Type::base(BaseType::Int),
///     Some(Expr::Int(10)),
/// ));
/// assert_eq!(t.declarator("p"), "int (*p)[10]");
/// ```
///
/// A function pointer (`void (*)(int)`):
/// ```rust
/// let t = Type::ptr(Type::func(
///     Type::base(BaseType::Void),
///     vec![Type::base(BaseType::Int)],
///     false,
/// ));
/// assert_eq!(t.declarator("cb"), "void (*cb)(int)");
/// ```
#[derive(Debug, Clone, DisplayFromFormat)]
pub enum Type {
    /// A base type with its (base-level) qualifiers, e.g. `const int`.
    Base {
        /// The underlying base type.
        base: BaseType,
        /// Qualifiers applied to the base type (the pointee, when under a pointer).
        quals: Vec<TypeQualifier>,
    },

    /// A pointer to another type, with its own (pointer-level) qualifiers.
    ///
    /// The qualifiers here apply to the pointer itself, so `Pointer { quals:
    /// [Const], to: char }` is `char * const` (a const pointer), as opposed to
    /// `const char *` (a pointer to const char), which is a `Base` with a
    /// `const` qualifier under a `Pointer`.
    Pointer {
        /// Qualifiers applied to the pointer itself (e.g. `const` in `T * const`).
        quals: Vec<TypeQualifier>,
        /// The pointee type.
        to: Box<Type>,
    },

    /// An array of another type. `size` is `None` for an incomplete/flexible
    /// array (`[]`) and `Some(expr)` for a sized one (`[N]`, where `N` may be a
    /// constant expression such as a named constant).
    Array {
        /// The array length, or `None` for `[]`.
        size: Option<Box<Expr>>,
        /// The element type.
        of: Box<Type>,
    },

    /// A function type, used chiefly under a pointer to form function pointers
    /// (`void (*)(int)`) or in typedefs.
    Function {
        /// The return type.
        ret: Box<Type>,
        /// The parameter types (abstract, i.e. unnamed).
        params: Vec<Type>,
        /// Whether the function is variadic (a trailing `...`).
        variadic: bool,
    },

    /// A raw, verbatim type spelling used as an escape hatch for types Tamago
    /// does not model (e.g. `_Atomic(int)`, a vector type, or a compiler
    /// builtin). It behaves like a base type in the declarator: the identifier
    /// is placed after it.
    Raw(String),
}

/// Renders a full C declaration for `ty` naming the declarator `inner`.
///
/// `inner` is the declarator built up so far; pass the identifier being declared
/// (e.g. `"p"`), or the empty string for an *abstract* declarator (as in a cast
/// or `sizeof`). The routine walks the type outward, parenthesizing the
/// declarator whenever a pointer's pointee is an array or function (because `[]`
/// and `()` bind tighter than `*`).
///
/// # Examples
///
/// ```rust
/// // int *a[10]  — array of ten pointers to int
/// let t = Type::array(Type::ptr(Type::base(BaseType::Int)), Some(Expr::Int(10)));
/// assert_eq!(declare(&t, "a"), "int *a[10]");
///
/// // int (*p)[10]  — pointer to an array of ten ints
/// let t = Type::ptr(Type::array(Type::base(BaseType::Int), Some(Expr::Int(10))));
/// assert_eq!(declare(&t, "p"), "int (*p)[10]");
/// ```
pub fn declare(ty: &Type, inner: &str) -> String {
    match ty {
        Type::Base { base, quals } => {
            let q = quals_prefix(quals);
            if inner.is_empty() {
                format!("{q}{base}")
            } else if inner.starts_with('[') {
                // Arrays attach directly to the base: `int[10]`.
                format!("{q}{base}{inner}")
            } else {
                format!("{q}{base} {inner}")
            }
        }
        Type::Pointer { quals, to } => {
            let pq = quals_prefix(quals);
            let star = format!("*{pq}{inner}");
            // A pointer to an array or function needs its declarator parenthesized.
            let next = match to.as_ref() {
                Type::Array { .. } | Type::Function { .. } => format!("({star})"),
                _ => star,
            };
            declare(to, &next)
        }
        Type::Array { size, of } => {
            let sz = size.as_ref().map(|e| e.to_string()).unwrap_or_default();
            declare(of, &format!("{inner}[{sz}]"))
        }
        Type::Function {
            ret,
            params,
            variadic,
        } => {
            let mut parts: Vec<String> = params.iter().map(|p| declare(p, "")).collect();
            if *variadic {
                parts.push("...".to_string());
            }
            let params_str = if parts.is_empty() {
                "void".to_string()
            } else {
                parts.join(", ")
            };
            declare(ret, &format!("{inner}({params_str})"))
        }
        Type::Raw(spelling) => {
            if inner.is_empty() {
                spelling.clone()
            } else if inner.starts_with('[') {
                format!("{spelling}{inner}")
            } else {
                format!("{spelling} {inner}")
            }
        }
    }
}

/// Builds the `const `/`volatile ` prefix (each qualifier followed by a space),
/// in the order the qualifiers are stored.
fn quals_prefix(quals: &[TypeQualifier]) -> String {
    let mut out = String::new();
    for q in quals {
        out.push_str(&q.to_string());
        out.push(' ');
    }
    out
}

impl Type {
    /// Creates and returns a new [`TypeBuilder`] to construct a `Type` using the
    /// builder pattern.
    ///
    /// This is the backwards-compatible facade over the recursive representation
    /// and covers the common cases (qualifiers, a run of pointers, and a single
    /// array dimension). For anything richer — function pointers, multidimensional
    /// or unsized arrays, pointer-level `const` — use [`Type::ptr`],
    /// [`Type::array`], [`Type::func`], or the enum variants directly.
    ///
    /// # Parameters
    ///
    /// * `base` - The base type to start with
    ///
    /// # Returns
    ///
    /// A `TypeBuilder` instance for configuring and building a `Type`
    ///
    /// # Examples
    ///
    /// ```rust
    /// let t = Type::new(BaseType::Char)
    ///     .make_const()
    ///     .make_pointer()
    ///     .build();
    /// assert_eq!(t.to_string(), "const char *");
    /// ```
    pub fn new(base: BaseType) -> TypeBuilder {
        TypeBuilder::new(base)
    }

    /// Creates an unqualified base type, e.g. `int`.
    pub fn base(base: BaseType) -> Type {
        Type::Base {
            base,
            quals: vec![],
        }
    }

    /// Creates a base type carrying the given qualifiers, e.g. `const int`.
    pub fn base_qualified(base: BaseType, quals: Vec<TypeQualifier>) -> Type {
        Type::Base { base, quals }
    }

    /// Creates a (non-const) pointer to `to`.
    pub fn ptr(to: Type) -> Type {
        Type::Pointer {
            quals: vec![],
            to: Box::new(to),
        }
    }

    /// Creates a `const` pointer to `to` (i.e. `T * const`).
    pub fn const_ptr(to: Type) -> Type {
        Type::Pointer {
            quals: vec![TypeQualifier::Const],
            to: Box::new(to),
        }
    }

    /// Creates an array of `of`. Pass `None` for an incomplete array (`[]`) or
    /// `Some(expr)` for a sized one (`[expr]`).
    pub fn array(of: Type, size: Option<Expr>) -> Type {
        Type::Array {
            size: size.map(Box::new),
            of: Box::new(of),
        }
    }

    /// Creates a function type returning `ret` and taking `params` (with an
    /// optional trailing `...` when `variadic`). Wrap it in [`Type::ptr`] to get
    /// a function pointer.
    pub fn func(ret: Type, params: Vec<Type>, variadic: bool) -> Type {
        Type::Function {
            ret: Box::new(ret),
            params,
            variadic,
        }
    }

    /// Creates a raw, verbatim type from an arbitrary spelling — an escape hatch
    /// for types Tamago does not model.
    ///
    /// # Examples
    /// ```rust
    /// let t = Type::raw("_Atomic(int)");
    /// assert_eq!(t.declarator("counter"), "_Atomic(int) counter");
    /// ```
    pub fn raw(spelling: impl Into<String>) -> Type {
        Type::Raw(spelling.into())
    }

    /// Renders the full declaration of this type for the identifier `name`.
    ///
    /// This is the method form of [`declare`]; it is what every declaration site
    /// (variables, fields, parameters, typedefs) uses to place the name correctly
    /// within the type.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let t = Type::ptr(Type::base(BaseType::Int));
    /// assert_eq!(t.declarator("x"), "int *x");
    /// ```
    pub fn declarator(&self, name: &str) -> String {
        declare(self, name)
    }

    /// Checks whether the outermost derivation of this type is an array.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let array_type = Type::new(BaseType::Int).make_array(5).build();
    /// assert!(array_type.is_array());
    /// let simple_type = Type::new(BaseType::Int).build();
    /// assert!(!simple_type.is_array());
    /// ```
    pub fn is_array(&self) -> bool {
        matches!(self, Type::Array { .. })
    }
}

impl Format for Type {
    fn format(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        // Formatting a bare `Type` yields its abstract declarator (empty name),
        // which is what casts and `sizeof` want.
        write!(fmt, "{}", declare(self, ""))
    }
}

/// A builder for constructing a `Type` instance with a fluent interface.
///
/// The `TypeBuilder` allows incremental configuration of a C type's properties,
/// such as qualifiers, pointers, and a single array dimension, before finalizing
/// the type. It is a facade over the recursive [`Type`] representation: `build`
/// assembles the qualified base, wraps it in the requested number of pointers,
/// and finally (if an array size was set) wraps that in an array.
pub struct TypeBuilder {
    base: BaseType,
    qualifiers: Vec<TypeQualifier>,
    pointers: u8,
    array: usize,
}

impl TypeBuilder {
    /// Creates and returns a new `TypeBuilder` to construct a `Type`.
    ///
    /// # Parameters
    ///
    /// * `base` - The base type to start with
    ///
    /// # Returns
    ///
    /// A new `TypeBuilder` instance with default values for qualifiers, pointers, and array size
    ///
    /// # Examples
    ///
    /// ```rust
    /// let builder = TypeBuilder::new(BaseType::Int);
    /// ```
    pub fn new(base: BaseType) -> Self {
        Self {
            base,
            qualifiers: vec![],
            pointers: 0,
            array: 0,
        }
    }

    /// Adds a type qualifier to the (base of the) type being built.
    ///
    /// # Parameters
    ///
    /// * `q` - The type qualifier to add
    ///
    /// # Returns
    ///
    /// The builder instance for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// let builder = TypeBuilder::new(BaseType::Float)
    ///     .type_qualifier(TypeQualifier::Const);
    /// ```
    pub fn type_qualifier(mut self, q: TypeQualifier) -> Self {
        self.qualifiers.push(q);
        self
    }

    /// Makes the type volatile.
    ///
    /// Adds the `volatile` qualifier to the base type.
    ///
    /// # Returns
    ///
    /// The builder instance for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// let builder = TypeBuilder::new(BaseType::Int).make_volatile();
    /// assert_eq!(builder.build().to_string(), "volatile int");
    /// ```
    pub fn make_volatile(self) -> Self {
        self.type_qualifier(TypeQualifier::Volatile)
    }

    /// Makes the type const.
    ///
    /// Adds the `const` qualifier to the base type. Note that, as in the original
    /// flat model, this always qualifies the base (the pointee when pointers are
    /// present); to make a *pointer* itself const, construct a
    /// [`Type::const_ptr`] directly.
    ///
    /// # Returns
    ///
    /// The builder instance for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// let builder = TypeBuilder::new(BaseType::Char).make_const();
    /// assert_eq!(builder.build().to_string(), "const char");
    /// ```
    pub fn make_const(self) -> Self {
        self.type_qualifier(TypeQualifier::Const)
    }

    /// Makes the type a pointer.
    ///
    /// Increases the pointer level by one.
    ///
    /// # Returns
    ///
    /// The builder instance for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// let builder = TypeBuilder::new(BaseType::Void).make_pointer();
    /// assert_eq!(builder.build().to_string(), "void *");
    /// ```
    pub fn make_pointer(mut self) -> Self {
        self.pointers += 1;
        self
    }

    /// Makes the type an array with the given size.
    ///
    /// A size of `0` is treated as "not an array" (preserving the original
    /// behavior); use the [`Type::array`] constructor for incomplete arrays.
    ///
    /// # Parameters
    ///
    /// * `size` - The size of the array
    ///
    /// # Returns
    ///
    /// The builder instance for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// let builder = TypeBuilder::new(BaseType::Int).make_array(10);
    /// ```
    pub fn make_array(mut self, size: usize) -> Self {
        self.array = size;
        self
    }

    /// Finalizes the type definition and returns a fully constructed `Type`.
    ///
    /// # Returns
    ///
    /// A fully constructed `Type` instance
    ///
    /// # Examples
    ///
    /// ```rust
    /// let t = TypeBuilder::new(BaseType::Double)
    ///     .make_const()
    ///     .make_pointer()
    ///     .build();
    /// assert_eq!(t.to_string(), "const double *");
    /// ```
    pub fn build(self) -> Type {
        let mut ty = Type::Base {
            base: self.base,
            quals: self.qualifiers,
        };

        for _ in 0..self.pointers {
            ty = Type::Pointer {
                quals: vec![],
                to: Box::new(ty),
            };
        }

        if self.array != 0 {
            ty = Type::Array {
                size: Some(Box::new(Expr::Int(self.array as i64))),
                of: Box::new(ty),
            };
        }

        ty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_type() {
        use BaseType::*;
        assert_eq!(Void.to_string(), "void");
        assert_eq!(Double.to_string(), "double");
        assert_eq!(Float.to_string(), "float");
        assert_eq!(Char.to_string(), "char");
        assert_eq!(Int.to_string(), "int");
        assert_eq!(UInt8.to_string(), "uint8_t");
        assert_eq!(UInt16.to_string(), "uint16_t");
        assert_eq!(UInt32.to_string(), "uint32_t");
        assert_eq!(UInt64.to_string(), "uint64_t");
        assert_eq!(Int8.to_string(), "int8_t");
        assert_eq!(Int16.to_string(), "int16_t");
        assert_eq!(Int32.to_string(), "int32_t");
        assert_eq!(Int64.to_string(), "int64_t");
        assert_eq!(Size.to_string(), "size_t");
        assert_eq!(UIntPtr.to_string(), "uintptr_t");
        assert_eq!(Bool.to_string(), "bool");
        assert_eq!(Enum("abc".to_string()).to_string(), "enum abc");
        assert_eq!(Struct("abc".to_string()).to_string(), "struct abc");
        assert_eq!(Union("abc".to_string()).to_string(), "union abc");
        assert_eq!(TypeDef("abc".to_string()).to_string(), "abc");
    }

    #[test]
    fn t() {
        use BaseType::*;
        let mut t = Type::new(Void).build();
        assert_eq!(t.to_string(), "void");

        t = Type::new(Void).make_pointer().make_pointer().build();
        assert_eq!(t.to_string(), "void **");

        t = Type::new(Void)
            .make_pointer()
            .make_pointer()
            .make_const()
            .build();
        assert_eq!(t.to_string(), "const void **");

        t = Type::new(Void)
            .make_pointer()
            .make_const()
            .make_array(10)
            .build();
        assert_eq!(t.to_string(), "const void *[10]");
        assert!(t.is_array())
    }

    #[test]
    fn declarator_named() {
        use BaseType::*;

        // simple pointer
        let t = Type::ptr(Type::base(Int));
        assert_eq!(t.declarator("x"), "int *x");

        // array of pointers
        let t = Type::array(Type::ptr(Type::base(Int)), Some(Expr::Int(10)));
        assert_eq!(t.declarator("a"), "int *a[10]");

        // pointer to array
        let t = Type::ptr(Type::array(Type::base(Int), Some(Expr::Int(10))));
        assert_eq!(t.declarator("p"), "int (*p)[10]");

        // function pointer
        let t = Type::ptr(Type::func(
            Type::base(Void),
            vec![Type::base(Int), Type::base(Char)],
            false,
        ));
        assert_eq!(t.declarator("cb"), "void (*cb)(int, char)");

        // const pointer to const char
        let t = Type::const_ptr(Type::base_qualified(Char, vec![TypeQualifier::Const]));
        assert_eq!(t.declarator("p"), "const char *const p");

        // incomplete (flexible) array
        let t = Type::array(Type::base(Int), None);
        assert_eq!(t.declarator("data"), "int data[]");
    }

    #[test]
    fn raw_type() {
        let t = Type::raw("_Atomic(int)");
        assert_eq!(t.declarator("counter"), "_Atomic(int) counter");
        assert_eq!(t.to_string(), "_Atomic(int)"); // abstract declarator

        // behaves like a base type: a pointer to a raw type still works
        let p = Type::ptr(Type::raw("__m128"));
        assert_eq!(p.declarator("v"), "__m128 *v");
    }
}
