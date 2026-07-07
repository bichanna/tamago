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

//! # Expression Module
//!
//! This module provides a set of tools for creating, manipulating, and
//! formatting C expressions. It implements an API for representing various
//! types of C expressions, including:
//!
//! - Literals (integers, floats, booleans, characters, strings)
//! - Identifiers and variables
//! - Binary operations (arithmetic, comparison, bitwise)
//! - Parenthesized expressions
//! - Unary operations (increment, decrement, negation, etc.)
//! - Assignment operations
//! - Ternary conditionals
//! - Function calls
//! - Member access
//! - Array indexing
//! - Type casting
//! - Size queries
//! - Array and struct initializations
//!
//! The module is designed to facilitate C code generation with a type-safe Rust interface.

use std::fmt::{self, Write};

use crate::escape::{escape_c_char, escape_c_str};
use crate::{Format, Formatter, Type};
use tamacro::{DisplayFromConstSymbol, DisplayFromFormat, FormatFromConstSymbol};

/// Formats an `f64` as a valid C `double` constant
fn format_c_double(num: f64) -> String {
    if num.is_nan() {
        "NAN".to_string()
    } else if num.is_infinite() {
        if num < 0.0 {
            "-INFINITY".to_string()
        } else {
            "INFINITY".to_string()
        }
    } else {
        let s = format!("{num}");
        if s.contains('.') || s.contains('e') || s.contains('E') {
            s
        } else {
            format!("{s}.0")
        }
    }
}

/// Formats an `f32` as a valid C `float` constant.
fn format_c_float(num: f32) -> String {
    if num.is_nan() {
        "NAN".to_string()
    } else if num.is_infinite() {
        if num < 0.0 {
            "-INFINITY".to_string()
        } else {
            "INFINITY".to_string()
        }
    } else {
        let s = format!("{num}");
        if s.contains('.') || s.contains('e') || s.contains('E') {
            format!("{s}f")
        } else {
            format!("{s}.0f")
        }
    }
}

/// Encapsulates all types of expressions in C.
///
/// This enum represents the complete set of expression types in C, providing a
/// comprehensive way to generate syntactically correct C expressions. Each variant
/// represents a distinct expression type with its associated data.
///
/// # Examples
///
/// ```
/// // Create a binary expression: (a + b)
/// let add_expr = Expr::new_binary(
///     Expr::new_ident_with_str("a"),
///     BinOp::Add,
///     Expr::new_ident_with_str("b")
/// );
///
/// // Create a function call: printf("Hello, %s", name)
/// let printf_call = Expr::new_fn_call_with_name(
///     "printf".to_string(),
///     vec![
///         Expr::Str("Hello, %s".to_string()),
///         Expr::new_ident_with_str("name")
///     ]
/// );
/// ```
#[derive(Debug, Clone, DisplayFromFormat)]
pub enum Expr {
    /// A signed integer literal (e.g., `42`, `-17`).
    Int(i64),

    /// An unsigned integer literal (e.g., `42u`, `0xFF`).
    UInt(u64),

    /// A double precision floating point number literal (e.g., `3.14159`).
    Double(f64),

    /// A single precision floating point number literal (e.g., `3.14f`).
    Float(f32),

    /// A boolean value (`true` or `false`).
    Bool(bool),

    /// A one-byte character literal (e.g., `'a'`, `'\n'`).
    Char(char),

    /// A string literal (e.g., `"hello"`).
    Str(String),

    /// An identifier representing a variable or function name.
    Ident(String),

    /// A binary expression combining two expressions with an operator.
    ///
    /// Examples: `a + b`, `x * y`, `ptr != NULL`
    Binary {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
    },

    /// A parenthesized expression.
    ///
    /// Examples: `(a + b) * c`
    Parenthesized { expr: Box<Expr> },

    /// A unary expression applying an operator to a single expression.
    ///
    /// Examples: `-x`, `!condition`, `++counter`, `&variable`
    Unary { op: UnaryOp, expr: Box<Expr> },

    /// A variable assignment expression.
    ///
    /// Examples: `x = 5`, `ptr += offset`, `flags &= mask`
    Assign {
        lvalue: Box<Expr>,
        op: AssignOp,
        value: Box<Expr>,
    },

    /// A ternary conditional expression.
    ///
    /// Example: `condition ? true_value : false_value`
    Ternary {
        cond: Box<Expr>,
        lexpr: Box<Expr>,
        rexpr: Box<Expr>,
    },

    /// A function call expression.
    ///
    /// Example: `printf("Hello, %s", name)`
    FnCall { name: Box<Expr>, args: Vec<Expr> },

    /// A struct member access expression.
    ///
    /// Example: `person.name`
    MemAccess { expr: Box<Expr>, member: String },

    /// A struct-pointer member access expression (the `->` operator), i.e.
    /// shorthand for dereferencing and then accessing a member.
    ///
    /// Example: `person->name`
    PtrMemAccess { expr: Box<Expr>, member: String },

    /// An array indexing expression.
    ///
    /// Example: `array[index]`
    ArrIndex { arr: Box<Expr>, idx: Box<Expr> },

    /// A type casting expression.
    ///
    /// Example: `(int *)pointer`
    Cast { t: Type, expr: Box<Expr> },

    /// A `sizeof` operator expression.
    ///
    /// Example: `sizeof(int)`
    SizeOf(Type),

    /// An alignment-query expression: `_Alignof(t)` (or `alignof(t)` in C23
    /// style).
    ///
    /// Example: `_Alignof(int)`
    AlignOf(Type),

    /// An `offsetof(type, member)` expression (from `<stddef.h>`).
    ///
    /// Example: `offsetof(struct S, field)`
    OffsetOf {
        /// The aggregate type.
        t: Type,
        /// The member whose offset is queried.
        member: String,
    },

    /// A string literal with an optional encoding prefix, e.g. `L"wide"`,
    /// `u8"utf8"`, `u"utf16"`, or `U"utf32"`.
    StrLit {
        /// The encoding prefix.
        prefix: EncodingPrefix,
        /// The (unescaped) string value.
        value: String,
    },

    /// A character literal with an optional encoding prefix, e.g. `L'x'`,
    /// `u'x'`, `U'x'`, or `u8'x'` (C23).
    CharLit {
        /// The encoding prefix.
        prefix: EncodingPrefix,
        /// The character value.
        value: char,
    },

    /// An array initialization expression.
    ///
    /// Examples:
    /// - In-order: `{1, 2, 3}`
    /// - Designated: `{[0]=1, [5]=2}`
    InitArr(Vec<(Option<usize>, Expr)>),

    /// A struct initialization expression.
    ///
    /// Examples:
    /// - In-order: `{1, "hello", 3.14}`
    /// - Designated: `{.x=1, .name="hello"}`
    InitStruct(Vec<(Option<String>, Expr)>),

    /// A compound literal (C99): a parenthesized type followed by a brace
    /// initializer, e.g. `(Point){1, 2}` or `(int[]){1, 2, 3}`.
    ///
    /// Prefer this over a cast applied to a brace initializer — a compound
    /// literal is a distinct construct (and an lvalue), not a cast
    CompoundLiteral {
        /// The type being constructed
        t: Type,
        /// The brace initializer (typically an [`Expr::InitStruct`] or
        /// [`Expr::InitArr`]).
        init: Box<Expr>,
    },

    /// An integer literal with an explicit radix and suffix, e.g. `0xFFULL`
    ///
    /// Unlike [`Expr::Int`]/[`Expr::UInt`], this gives control over the base and
    /// the `u`/`l`/`ll` suffix, which is what you need for width- and
    /// signedness-correct constants across data models.
    IntLit {
        /// The literal value.
        value: i128,
        /// The radix used to render it.
        base: IntBase,
        /// The integer suffix (`U`, `L`, `UL`, `LL`, `ULL`).
        suffix: IntSuffix,
    },

    /// A raw C expression as a string (for cases not covered by other variants).
    Raw(String),
}

/// The encoding prefix for a string or character literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EncodingPrefix {
    /// No prefix: `"..."` / `'x'`.
    #[default]
    None,
    /// `L` — wide (`wchar_t`): `L"..."` / `L'x'`.
    Wide,
    /// `u8` — UTF-8: `u8"..."` (and `u8'x'` in C23).
    Utf8,
    /// `u` — UTF-16 (`char16_t`): `u"..."` / `u'x'`.
    Utf16,
    /// `U` — UTF-32 (`char32_t`): `U"..."` / `U'x'`.
    Utf32,
}

impl EncodingPrefix {
    /// The literal prefix text (`""`, `"L"`, `"u8"`, `"u"`, `"U"`).
    pub fn as_str(self) -> &'static str {
        match self {
            EncodingPrefix::None => "",
            EncodingPrefix::Wide => "L",
            EncodingPrefix::Utf8 => "u8",
            EncodingPrefix::Utf16 => "u",
            EncodingPrefix::Utf32 => "U",
        }
    }
}

/// The radix used to render an [`Expr::IntLit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IntBase {
    /// Decimal, e.g. `42`.
    #[default]
    Dec,
    /// Hexadecimal, e.g. `0x2A`.
    Hex,
    /// Octal, e.g. `052`.
    Oct,
}

/// The suffix applied to an [`Expr::IntLit`], controlling its type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IntSuffix {
    /// No suffix (`int`).
    #[default]
    None,
    /// `U` (`unsigned int`).
    U,
    /// `L` (`long`).
    L,
    /// `UL` (`unsigned long`).
    UL,
    /// `LL` (`long long`).
    LL,
    /// `ULL` (`unsigned long long`).
    ULL,
}

impl IntSuffix {
    fn as_str(self) -> &'static str {
        match self {
            IntSuffix::None => "",
            IntSuffix::U => "U",
            IntSuffix::L => "L",
            IntSuffix::UL => "UL",
            IntSuffix::LL => "LL",
            IntSuffix::ULL => "ULL",
        }
    }
}

impl Expr {
    /// Creates a new identifier expression.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the identifier as a String.
    ///
    /// # Returns
    ///
    /// A new `Expr::Ident` with the given name.
    pub fn new_ident(name: String) -> Self {
        Self::Ident(name)
    }

    /// Creates a new identifier expression with a string slice.
    ///
    /// This is a convenience method that converts the string slice to a String.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the identifier as a string slice.
    ///
    /// # Returns
    ///
    /// A new `Expr::Ident` with the given name.
    pub fn new_ident_with_str(name: &str) -> Self {
        Self::new_ident(name.to_string())
    }

    /// Creates a new NULL pointer expression.
    ///
    /// # Returns
    ///
    /// A new `Expr::Ident` representing the NULL constant in C.
    pub fn new_null() -> Self {
        Self::new_ident("NULL".to_string())
    }

    /// Creates a new binary expression with the given expressions and binary operator.
    ///
    /// # Arguments
    ///
    /// * `left` - The expression on the left side of the operator.
    /// * `op` - The binary operator.
    /// * `right` - The expression on the right side of the operator.
    ///
    /// # Returns
    ///
    /// A new `Expr::Binary` combining the left and right expressions with the given operator.
    pub fn new_binary(left: Expr, op: BinOp, right: Expr) -> Self {
        Self::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
        }
    }

    /// Creates a new parenthesized expression with the given expression.
    ///
    /// # Arguments
    ///
    /// * `expr` - The expression to be parenthesized.
    ///
    /// # Returns
    ///
    /// A new `Expr::Parenthesized` with the given expression.
    pub fn new_parenthesized(expr: Expr) -> Self {
        Self::Parenthesized {
            expr: Box::new(expr),
        }
    }

    /// Creates a new unary expression with the given expression and unary operator.
    ///
    /// # Arguments
    ///
    /// * `expr` - The expression to which the operator is applied.
    /// * `op` - The unary operator.
    ///
    /// # Returns
    ///
    /// A new `Expr::Unary` applying the operator to the expression.
    pub fn new_unary(expr: Expr, op: UnaryOp) -> Self {
        Self::Unary {
            expr: Box::new(expr),
            op,
        }
    }

    /// Creates a new assignment expression.
    ///
    /// # Arguments
    ///
    /// * `lvalue` - The expression on the left side (must be a valid lvalue in C).
    /// * `op` - The assignment operator.
    /// * `value` - The expression on the right side.
    ///
    /// # Returns
    ///
    /// A new `Expr::Assign` representing the assignment operation.
    pub fn new_assign(lvalue: Expr, op: AssignOp, value: Expr) -> Self {
        Self::Assign {
            lvalue: Box::new(lvalue),
            op,
            value: Box::new(value),
        }
    }

    /// Creates a new ternary conditional expression.
    ///
    /// # Arguments
    ///
    /// * `cond` - The condition expression.
    /// * `lexpr` - The expression to evaluate if the condition is true.
    /// * `rexpr` - The expression to evaluate if the condition is false.
    ///
    /// # Returns
    ///
    /// A new `Expr::Ternary` representing the conditional expression.
    pub fn new_ternary(cond: Expr, lexpr: Expr, rexpr: Expr) -> Self {
        Self::Ternary {
            cond: Box::new(cond),
            lexpr: Box::new(lexpr),
            rexpr: Box::new(rexpr),
        }
    }

    /// Creates a new function call expression.
    ///
    /// # Arguments
    ///
    /// * `name` - The expression representing the function name or pointer.
    /// * `args` - A vector of expressions representing the function arguments.
    ///
    /// # Returns
    ///
    /// A new `Expr::FnCall` representing the function call.
    pub fn new_fn_call(name: Expr, args: Vec<Expr>) -> Self {
        Self::FnCall {
            name: Box::new(name),
            args,
        }
    }

    /// Creates a new function call expression with a string name.
    ///
    /// This is a convenience method for calling a function by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the function as a String.
    /// * `args` - A vector of expressions representing the function arguments.
    ///
    /// # Returns
    ///
    /// A new `Expr::FnCall` representing the function call.
    pub fn new_fn_call_with_name(name: String, args: Vec<Expr>) -> Self {
        Self::FnCall {
            name: Box::new(Self::Ident(name)),
            args,
        }
    }

    /// Creates a new struct member access expression.
    ///
    /// # Arguments
    ///
    /// * `expr` - The expression representing the struct instance.
    /// * `member` - The name of the struct member as a String.
    ///
    /// # Returns
    ///
    /// A new `Expr::MemAccess` representing the member access.
    pub fn new_mem_access(expr: Expr, member: String) -> Self {
        Self::MemAccess {
            expr: Box::new(expr),
            member,
        }
    }

    /// Creates a new struct member access expression with a string slice.
    ///
    /// This is a convenience method that converts the string slice to a String.
    ///
    /// # Arguments
    ///
    /// * `expr` - The expression representing the struct instance.
    /// * `member` - The name of the struct member as a string slice.
    ///
    /// # Returns
    ///
    /// A new `Expr::MemAccess` representing the member access.
    pub fn new_mem_access_with_str(expr: Expr, member: &str) -> Self {
        Self::new_mem_access(expr, member.to_string())
    }

    /// Creates a new struct-pointer member access expression (the `->`
    /// operator).
    ///
    /// # Arguments
    ///
    /// * `expr` - The expression representing the pointer to a struct instance.
    /// * `member` - The name of the struct member as a String.
    ///
    /// # Returns
    ///
    /// A new `Expr::PtrMemAccess` representing the member access.
    pub fn new_ptr_mem_access(expr: Expr, member: String) -> Self {
        Self::PtrMemAccess {
            expr: Box::new(expr),
            member,
        }
    }

    /// Creates a new struct-pointer member access expression (the `->`
    /// operator) with a string slice.
    ///
    /// This is a convenience method that converts the string slice to a String.
    ///
    /// # Arguments
    ///
    /// * `expr` - The expression representing the pointer to a struct instance.
    /// * `member` - The name of the struct member as a string slice.
    ///
    /// # Returns
    ///
    /// A new `Expr::PtrMemAccess` representing the member access.
    pub fn new_ptr_mem_access_with_str(expr: Expr, member: &str) -> Self {
        Self::new_ptr_mem_access(expr, member.to_string())
    }

    /// Creates a new array indexing expression.
    ///
    /// # Arguments
    ///
    /// * `arr` - The expression representing the array.
    /// * `idx` - The expression representing the index.
    ///
    /// # Returns
    ///
    /// A new `Expr::ArrIndex` representing the array indexing operation.
    pub fn new_arr_index(arr: Expr, idx: Expr) -> Self {
        Self::ArrIndex {
            arr: Box::new(arr),
            idx: Box::new(idx),
        }
    }

    /// Creates a new type casting expression.
    ///
    /// # Arguments
    ///
    /// * `t` - The target type for the cast.
    /// * `expr` - The expression to be cast.
    ///
    /// # Returns
    ///
    /// A new `Expr::Cast` representing the type casting operation.
    pub fn new_cast(t: Type, expr: Expr) -> Self {
        Self::Cast {
            t,
            expr: Box::new(expr),
        }
    }

    /// Creates a compound literal `(t){init}`.
    ///
    /// `init` is typically an [`Expr::new_init_struct_in_order`] /
    /// [`Expr::new_init_struct_designated`] or an array initializer.
    ///
    /// # Examples
    /// ```rust
    /// let p = Expr::new_compound_literal(
    ///     Type::base(BaseType::TypeDef("Point".to_string())),
    ///     Expr::new_init_struct_in_order(vec![Expr::Int(1), Expr::Int(2)]),
    /// );
    /// assert_eq!(p.to_string(), "(Point){1, 2}");
    /// ```
    pub fn new_compound_literal(t: Type, init: Expr) -> Self {
        Self::CompoundLiteral {
            t,
            init: Box::new(init),
        }
    }

    /// Creates a decimal integer literal with an explicit suffix, e.g.
    /// `Expr::new_int_lit(1, IntSuffix::ULL)` renders `1ULL`.
    pub fn new_int_lit(value: i128, suffix: IntSuffix) -> Self {
        Self::IntLit {
            value,
            base: IntBase::Dec,
            suffix,
        }
    }

    /// Creates a hexadecimal integer literal with an explicit suffix, e.g.
    /// `Expr::new_hex(255, IntSuffix::U)` renders `0xFFU`.
    pub fn new_hex(value: i128, suffix: IntSuffix) -> Self {
        Self::IntLit {
            value,
            base: IntBase::Hex,
            suffix,
        }
    }

    /// Creates a new `sizeof` operator expression.
    ///
    /// # Arguments
    ///
    /// * `t` - The type whose size to query.
    ///
    /// # Returns
    ///
    /// A new `Expr::SizeOf` representing the sizeof operation.
    pub fn new_sizeof(t: Type) -> Self {
        Self::SizeOf(t)
    }

    /// Creates an alignment-query expression, rendered `_Alignof(t)` (or
    /// `alignof(t)` in C23 style).
    pub fn new_alignof(t: Type) -> Self {
        Self::AlignOf(t)
    }

    /// Creates an `offsetof(t, member)` expression.
    pub fn new_offsetof(t: Type, member: &str) -> Self {
        Self::OffsetOf {
            t,
            member: member.to_string(),
        }
    }

    /// Creates a string literal with the given encoding prefix, e.g.
    /// `Expr::new_str_lit(EncodingPrefix::Wide, "hi")` renders `L"hi"`.
    pub fn new_str_lit(prefix: EncodingPrefix, value: &str) -> Self {
        Self::StrLit {
            prefix,
            value: value.to_string(),
        }
    }

    /// Creates a character literal with the given encoding prefix, e.g.
    /// `Expr::new_char_lit(EncodingPrefix::Utf32, 'A')` renders `U'A'`.
    pub fn new_char_lit(prefix: EncodingPrefix, value: char) -> Self {
        Self::CharLit { prefix, value }
    }

    /// Creates a new in-order array initialization expression.
    ///
    /// # Arguments
    ///
    /// * `exprs` - A vector of expressions representing the array elements in order.
    ///
    /// # Returns
    ///
    /// A new `Expr::InitArr` representing the array initialization.
    pub fn new_init_arr_in_order(exprs: Vec<Expr>) -> Self {
        Self::InitArr(exprs.into_iter().map(|expr| (None, expr)).collect())
    }

    /// Creates a new designated array initialization expression.
    ///
    /// # Arguments
    ///
    /// * `x` - A vector of indices for the designated initializers.
    /// * `y` - A vector of expressions representing the values for each index.
    ///
    /// # Returns
    ///
    /// A new `Expr::InitArr` representing the designated array initialization.
    ///
    /// # Panics
    ///
    /// Panics if the lengths of `x` and `y` are not equal.
    pub fn new_init_arr_designated(x: Vec<usize>, y: Vec<Expr>) -> Self {
        assert!(x.len() == y.len());
        Self::InitArr(x.into_iter().map(Some).zip(y).collect())
    }

    /// Creates a new in-order struct initialization expression.
    ///
    /// # Arguments
    ///
    /// * `exprs` - A vector of expressions representing the struct members in order.
    ///
    /// # Returns
    ///
    /// A new `Expr::InitStruct` representing the struct initialization.
    pub fn new_init_struct_in_order(exprs: Vec<Expr>) -> Self {
        Self::InitStruct(exprs.into_iter().map(|expr| (None, expr)).collect())
    }

    /// Creates a new designated struct initialization expression.
    ///
    /// # Arguments
    ///
    /// * `x` - A vector of member names for the designated initializers.
    /// * `y` - A vector of expressions representing the values for each member.
    ///
    /// # Returns
    ///
    /// A new `Expr::InitStruct` representing the designated struct initialization.
    ///
    /// # Panics
    ///
    /// Panics if the lengths of `x` and `y` are not equal.
    pub fn new_init_struct_designated(x: Vec<String>, y: Vec<Expr>) -> Self {
        assert!(x.len() == y.len());
        Self::InitStruct(x.into_iter().map(Some).zip(y).collect())
    }
}

const PREC_ASSIGN: u8 = 2; // = += -= ... (right associative)
const PREC_TERNARY: u8 = 3; // ?: (right associative)
const PREC_LOGIC_OR: u8 = 4; // ||
const PREC_LOGIC_AND: u8 = 5; // &&
const PREC_BIT_OR: u8 = 6; // |
const PREC_BIT_XOR: u8 = 7; // ^
const PREC_BIT_AND: u8 = 8; // &
const PREC_EQ: u8 = 9; // == !=
const PREC_REL: u8 = 10; // < <= > >=
const PREC_SHIFT: u8 = 11; // << >>
const PREC_ADD: u8 = 12; // + -
const PREC_MUL: u8 = 13; // * / %
const PREC_UNARY: u8 = 14; // prefix ! ~ - & *  and casts (right associative)
const PREC_POSTFIX: u8 = 15; // postfix ++ -- , calls, [] and . member access
const PREC_PRIMARY: u8 = 16; // literals, identifiers, and anything self-delimiting, basically

impl BinOp {
    /// Returns the C precedence level of this binary operator (higher binds
    /// tighter). Every binary operator in C is left-associative
    pub fn precedence(&self) -> u8 {
        use BinOp::*;
        match self {
            Mul | Div | Mod => PREC_MUL,
            Add | Sub => PREC_ADD,
            LShift | RShift => PREC_SHIFT,
            LT | LTE | GT | GTE => PREC_REL,
            Eq | NEq => PREC_EQ,
            BitAnd => PREC_BIT_AND,
            XOr => PREC_BIT_XOR,
            BitOr => PREC_BIT_OR,
            And => PREC_LOGIC_AND,
            Or => PREC_LOGIC_OR,
        }
    }
}

impl Expr {
    /// Returns the C precedence level of this expression (higher binds tighter).
    ///
    /// This is what drives minimal parenthesization when formatting: a child
    /// expression is wrapped in parentheses only when its precedence (and
    /// associativity) would otherwise cause the emitted C to regroup.
    pub fn precedence(&self) -> u8 {
        use Expr::*;
        match self {
            IntLit { value, .. } if *value < 0 => PREC_UNARY,
            Int(n) if *n < 0 => PREC_UNARY,

            Int(_)
            | UInt(_)
            | Double(_)
            | Float(_)
            | Bool(_)
            | Char(_)
            | Str(_)
            | Ident(_)
            | Parenthesized { .. }
            | SizeOf(_)
            | AlignOf(_)
            | OffsetOf { .. }
            | StrLit { .. }
            | CharLit { .. }
            | InitArr(_)
            | InitStruct(_)
            | CompoundLiteral { .. }
            | IntLit { .. }
            | Raw(_) => PREC_PRIMARY,

            // Calls, subscripting, and member access are postfix. `Inc`/`Dec`
            // are rendered postfix here too, so they sit at the postfix level
            FnCall { .. } | MemAccess { .. } | PtrMemAccess { .. } | ArrIndex { .. } => {
                PREC_POSTFIX
            }
            Unary { op, .. } if matches!(op, UnaryOp::Inc | UnaryOp::Dec) => PREC_POSTFIX,

            // Remaining unary operators are prefix, and casts share their level.
            Unary { .. } | Cast { .. } => PREC_UNARY,

            Binary { op, .. } => op.precedence(),
            Ternary { .. } => PREC_TERNARY,
            Assign { .. } => PREC_ASSIGN,
        }
    }

    /// Formats `self`, wrapping it in parentheses iff `parens` is true.
    fn fmt_paren_if(&self, fmt: &mut Formatter<'_>, parens: bool) -> fmt::Result {
        if parens {
            write!(fmt, "(")?;
            self.format(fmt)?;
            write!(fmt, ")")
        } else {
            self.format(fmt)
        }
    }
}

/// Returns true when placing `operand` directly after the prefix operator `op`
/// would merge into a different token (e.g. `-` followed by `-x` becoming `--x`,
/// or `&` followed by `&x` becoming `&&`). In those cases a separating space is
/// emitted instead of relying on parentheses.
fn needs_separating_space(op: &str, operand: &str) -> bool {
    matches!(
        (op.chars().last(), operand.chars().next()),
        (Some('-'), Some('-')) | (Some('+'), Some('+')) | (Some('&'), Some('&'))
    )
}

/// Formats `items` separated by `", "`, invoking `each` for every element. This
/// replaces the hand-written `for x in &v[..v.len() - 1] { ...; write!(", ") }`
/// plus a trailing `v.last()` idiom, which was duplicated across calls,
/// array initializers, and struct initializers and is easy to get off by one.
fn format_comma_separated<T>(
    fmt: &mut Formatter<'_>,
    items: &[T],
    mut each: impl FnMut(&mut Formatter<'_>, &T) -> fmt::Result,
) -> fmt::Result {
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            write!(fmt, ", ")?;
        }
        each(fmt, item)?;
    }
    Ok(())
}

impl Format for Expr {
    fn format(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        use Expr::*;
        match self {
            Int(num) => write!(fmt, "{num}"),
            UInt(num) => write!(fmt, "{num}u"),
            Double(num) => write!(fmt, "{}", format_c_double(*num)),
            Float(num) => write!(fmt, "{}", format_c_float(*num)),
            Bool(b) => write!(fmt, "{}", if *b { "true" } else { "false" }),
            Char(c) => write!(fmt, "'{}'", escape_c_char(*c)),
            Str(s) => write!(fmt, "\"{}\"", escape_c_str(s)),
            Ident(name) => write!(fmt, "{name}"),
            Binary { left, op, right } => {
                let p = op.precedence();
                left.fmt_paren_if(fmt, left.precedence() < p)?;
                write!(fmt, " ")?;
                op.format(fmt)?;
                write!(fmt, " ")?;
                right.fmt_paren_if(fmt, right.precedence() <= p)
            }
            Parenthesized { expr } => {
                write!(fmt, "(")?;
                expr.format(fmt)?;
                write!(fmt, ")")
            }
            Unary { op, expr } => {
                if matches!(op, UnaryOp::Inc | UnaryOp::Dec) {
                    expr.fmt_paren_if(fmt, expr.precedence() < PREC_POSTFIX)?;
                    op.format(fmt)
                } else {
                    // prefix operator
                    op.format(fmt)?;
                    if expr.precedence() < PREC_UNARY {
                        write!(fmt, "(")?;
                        expr.format(fmt)?;
                        write!(fmt, ")")
                    } else {
                        let operand = crate::render(&**expr, fmt.options());
                        if needs_separating_space(&op.to_string(), &operand) {
                            write!(fmt, " ")?;
                        }
                        write!(fmt, "{operand}")
                    }
                }
            }
            Assign { lvalue, op, value } => {
                lvalue.fmt_paren_if(fmt, lvalue.precedence() <= PREC_ASSIGN)?;
                write!(fmt, " ")?;
                op.format(fmt)?;
                write!(fmt, " ")?;
                value.fmt_paren_if(fmt, value.precedence() < PREC_ASSIGN)
            }
            Ternary { cond, lexpr, rexpr } => {
                cond.fmt_paren_if(fmt, cond.precedence() <= PREC_TERNARY)?;
                write!(fmt, " ? ")?;
                lexpr.format(fmt)?;
                write!(fmt, " : ")?;
                rexpr.fmt_paren_if(fmt, rexpr.precedence() < PREC_TERNARY)
            }
            FnCall { name, args } => {
                name.fmt_paren_if(fmt, name.precedence() < PREC_POSTFIX)?;
                write!(fmt, "(")?;
                format_comma_separated(fmt, args, |fmt, arg| arg.format(fmt))?;
                write!(fmt, ")")
            }
            MemAccess { expr, member } => {
                expr.fmt_paren_if(fmt, expr.precedence() < PREC_POSTFIX)?;
                write!(fmt, ".{member}")
            }
            PtrMemAccess { expr, member } => {
                expr.fmt_paren_if(fmt, expr.precedence() < PREC_POSTFIX)?;
                write!(fmt, "->{member}")
            }
            ArrIndex { arr, idx } => {
                arr.fmt_paren_if(fmt, arr.precedence() < PREC_POSTFIX)?;
                write!(fmt, "[")?;
                idx.format(fmt)?;
                write!(fmt, "]")
            }
            Cast { t, expr } => {
                write!(fmt, "(")?;
                t.format(fmt)?;
                write!(fmt, ")")?;
                expr.fmt_paren_if(fmt, expr.precedence() < PREC_UNARY)
            }
            SizeOf(t) => {
                write!(fmt, "sizeof(")?;
                t.format(fmt)?;
                write!(fmt, ")")
            }
            AlignOf(t) => {
                let kw = if fmt.c23_keywords() {
                    "alignof"
                } else {
                    "_Alignof"
                };
                write!(fmt, "{kw}(")?;
                t.format(fmt)?;
                write!(fmt, ")")
            }
            OffsetOf { t, member } => {
                write!(fmt, "offsetof(")?;
                t.format(fmt)?;
                write!(fmt, ", {member})")
            }
            StrLit { prefix, value } => {
                write!(fmt, "{}\"{}\"", prefix.as_str(), escape_c_str(value))
            }
            CharLit { prefix, value } => {
                write!(fmt, "{}'{}'", prefix.as_str(), escape_c_char(*value))
            }
            InitArr(v) => {
                write!(fmt, "{{")?;
                format_comma_separated(fmt, v, |fmt, (idx, expr)| {
                    if let Some(idx) = idx {
                        write!(fmt, "[{idx}]=")?;
                    }
                    expr.format(fmt)
                })?;
                write!(fmt, "}}")
            }
            InitStruct(v) => {
                write!(fmt, "{{")?;
                format_comma_separated(fmt, v, |fmt, (name, expr)| {
                    if let Some(name) = name {
                        write!(fmt, ".{name}=")?;
                    }
                    expr.format(fmt)
                })?;
                write!(fmt, "}}")
            }
            CompoundLiteral { t, init } => {
                write!(fmt, "(")?;
                t.format(fmt)?;
                write!(fmt, ")")?;
                init.format(fmt)
            }
            IntLit {
                value,
                base,
                suffix,
            } => {
                let neg = *value < 0;
                let mag = value.unsigned_abs();
                if neg {
                    write!(fmt, "-")?;
                }
                match base {
                    IntBase::Dec => write!(fmt, "{mag}")?,
                    IntBase::Hex => write!(fmt, "0x{mag:X}")?,
                    IntBase::Oct => write!(fmt, "0{mag:o}")?,
                }
                write!(fmt, "{}", suffix.as_str())
            }
            Raw(s) => write!(fmt, "{s}"),
        }
    }
}

/// Encapsulates binary operators used in C expressions.
///
/// This enum represents all binary operators in C, including arithmetic,
/// comparison, logical, and bitwise operators.
#[derive(Debug, Clone, DisplayFromConstSymbol, FormatFromConstSymbol)]
pub enum BinOp {
    /// Addition operator (`+`)
    #[symbol = "+"]
    Add,

    /// Subtraction operator (`-`)
    #[symbol = "-"]
    Sub,

    /// Multiplication operator (`*`)
    #[symbol = "*"]
    Mul,

    /// Division operator (`/`)
    #[symbol = "/"]
    Div,

    /// Modulo operator (`%`)
    #[symbol = "%"]
    Mod,

    /// Equality comparison operator (`==`)
    #[symbol = "=="]
    Eq,

    /// Inequality comparison operator (`!=`)
    #[symbol = "!="]
    NEq,

    /// Greater than comparison operator (`>`)
    #[symbol = ">"]
    GT,

    /// Less than comparison operator (`<`)
    #[symbol = "<"]
    LT,

    /// Greater than or equal comparison operator (`>=`)
    #[symbol = ">="]
    GTE,

    /// Less than or equal comparison operator (`<=`)
    #[symbol = "<="]
    LTE,

    /// Logical AND operator (`&&`)
    #[symbol = "&&"]
    And,

    /// Logical OR operator (`||`)
    #[symbol = "||"]
    Or,

    /// Bitwise AND operator (`&`)
    #[symbol = "&"]
    BitAnd,

    /// Bitwise OR operator (`|`)
    #[symbol = "|"]
    BitOr,

    /// Bitwise XOR operator (`^`)
    #[symbol = "^"]
    XOr,

    /// Left shift operator (`<<`)
    #[symbol = "<<"]
    LShift,

    /// Right shift operator (`>>`)
    #[symbol = ">>"]
    RShift,
}

/// Encapsulates unary operators used in C expressions.
///
/// This enum represents all unary operators in C, including prefix
/// and postfix operators for various operations.
#[derive(Debug, Clone, DisplayFromConstSymbol, FormatFromConstSymbol)]
pub enum UnaryOp {
    /// Postfix increment operator (`x++`).
    #[symbol = "++"]
    Inc,

    /// Postfix decrement operator (`x--`).
    #[symbol = "--"]
    Dec,

    /// Prefix increment operator (`++x`).
    #[symbol = "++"]
    PreInc,

    /// Prefix decrement operator (`--x`).
    #[symbol = "--"]
    PreDec,

    /// Unary negation operator (`-`)
    #[symbol = "-"]
    Neg,

    /// Logical negation operator (`!`)
    #[symbol = "!"]
    LogicNeg,

    /// Bitwise NOT operator (`~`)
    #[symbol = "~"]
    BitNot,

    /// Address-of operator (`&`)
    #[symbol = "&"]
    AddrOf,

    /// Dereference operator (`*`)
    #[symbol = "*"]
    Deref,
}

/// Encapsulates assignment operators used in C expressions.
///
/// This enum represents all assignment operators in C, including
/// simple assignment and compound assignments combining assignment
/// with other operations.
#[derive(Debug, Clone, DisplayFromConstSymbol, FormatFromConstSymbol)]
pub enum AssignOp {
    /// Simple assignment operator (`=`)
    #[symbol = "="]
    Assign,

    /// Addition assignment operator (`+=`)
    #[symbol = "+="]
    AddAssign,

    /// Subtraction assignment operator (`-=`)
    #[symbol = "-="]
    SubAssign,

    /// Multiplication assignment operator (`*=`)
    #[symbol = "*="]
    MulAssign,

    /// Division assignment operator (`/=`)
    #[symbol = "/="]
    DivAssign,

    /// Modulo assignment operator (`%=`)
    #[symbol = "%="]
    ModAssign,

    /// Bitwise AND assignment operator (`&=`)
    #[symbol = "&="]
    BitAndAssign,

    /// Bitwise OR assignment operator (`|=`)
    #[symbol = "|="]
    BitOrAssign,

    /// Bitwise XOR assignment operator (`^=`)
    #[symbol = "^="]
    BitXOrAssign,

    /// Left shift assignment operator (`<<=`)
    #[symbol = "<<="]
    LShiftAssign,

    /// Right shift assignment operator (`>>=`)
    #[symbol = ">>="]
    RShiftAssign,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn binary() {
        let b = Expr::new_binary(
            Expr::new_binary(Expr::Int(123), BinOp::LT, Expr::Int(321)),
            BinOp::BitOr,
            Expr::new_binary(Expr::Double(1.23), BinOp::Sub, Expr::Float(3.21)),
        );
        let res = "123 < 321 | 1.23 - 3.21f";
        assert_eq!(b.to_string(), res);
    }

    #[test]
    fn parenthesized() {
        let p = Expr::new_binary(
            Expr::Int(3),
            BinOp::Mul,
            Expr::new_parenthesized(Expr::new_binary(Expr::Int(123), BinOp::Add, Expr::Int(321))),
        );
        let res = "3 * (123 + 321)";
        assert_eq!(p.to_string(), res);
    }

    #[test]
    fn unary() {
        let u = Expr::new_unary(
            Expr::new_unary(
                Expr::new_sizeof(Type::new(BaseType::Struct("some_struct".to_string())).build()),
                UnaryOp::Inc,
            ),
            UnaryOp::Neg,
        );
        let res = "-sizeof(struct some_struct)++";
        assert_eq!(u.to_string(), res);
    }

    #[test]
    fn assign() {
        let a = Expr::new_assign(
            Expr::Ident("abc".to_string()),
            AssignOp::SubAssign,
            Expr::Int(123),
        );
        let res = "abc -= 123";
        assert_eq!(a.to_string(), res);

        let b = Expr::new_assign(
            Expr::Ident("abc".to_string()),
            AssignOp::BitAndAssign,
            Expr::Bool(false),
        );
        let res = "abc &= false";
        assert_eq!(b.to_string(), res);
    }

    #[test]
    fn ternary() {
        let t = Expr::new_ternary(
            Expr::Bool(true),
            Expr::Str("hello".to_string()),
            Expr::Str("olleh".to_string()),
        );
        let res = r#"true ? "hello" : "olleh""#;
        assert_eq!(t.to_string(), res);
    }

    #[test]
    fn fncall() {
        let f = Expr::new_fn_call(Expr::Ident("some_func".to_string()), vec![]);
        let res = "some_func()";
        assert_eq!(f.to_string(), res);

        let f2 = Expr::new_fn_call(
            Expr::Ident("some_func".to_string()),
            vec![
                Expr::Char('a'),
                Expr::new_sizeof(Type::new(BaseType::Char).build()),
            ],
        );
        let res2 = "some_func('a', sizeof(char))";
        assert_eq!(f2.to_string(), res2);
    }

    #[test]
    fn mem_access() {
        let m = Expr::new_mem_access(Expr::Ident("person".to_string()), "age".to_string());
        let res = "person.age";
        assert_eq!(m.to_string(), res);
    }

    #[test]
    fn ptr_mem_access() {
        let m = Expr::new_ptr_mem_access_with_str(Expr::new_ident_with_str("person"), "age");
        assert_eq!(m.to_string(), "person->age");

        let chained = Expr::new_ptr_mem_access_with_str(
            Expr::new_ptr_mem_access_with_str(Expr::new_ident_with_str("a"), "b"),
            "c",
        );
        assert_eq!(chained.to_string(), "a->b->c");

        let deref = Expr::new_ptr_mem_access_with_str(
            Expr::new_unary(Expr::new_ident_with_str("pp"), UnaryOp::Deref),
            "x",
        );
        assert_eq!(deref.to_string(), "(*pp)->x");

        let mixed = Expr::new_mem_access_with_str(
            Expr::new_ptr_mem_access_with_str(Expr::new_ident_with_str("node"), "next"),
            "value",
        );
        assert_eq!(mixed.to_string(), "node->next.value");
    }

    #[test]
    fn arr_index() {
        let a = Expr::new_arr_index(Expr::Ident("some_arr".to_string()), Expr::Int(5));
        let res = "some_arr[5]";
        assert_eq!(a.to_string(), res);
    }

    #[test]
    fn cast() {
        let c = Expr::new_cast(
            Type::new(BaseType::Void).make_pointer().build(),
            Expr::Ident("something".to_string()),
        );
        let res = "(void *)something";
        assert_eq!(c.to_string(), res);
    }

    #[test]
    fn int_literals_with_suffixes() {
        assert_eq!(Expr::new_int_lit(1, IntSuffix::ULL).to_string(), "1ULL");
        assert_eq!(Expr::new_int_lit(-5, IntSuffix::L).to_string(), "-5L");
        assert_eq!(Expr::new_hex(255, IntSuffix::U).to_string(), "0xFFU");
        assert_eq!(Expr::new_hex(4096, IntSuffix::None).to_string(), "0x1000");
        assert_eq!(
            Expr::IntLit {
                value: 42,
                base: IntBase::Oct,
                suffix: IntSuffix::None
            }
            .to_string(),
            "052"
        );
    }

    #[test]
    fn compound_literal() {
        let cl = Expr::new_compound_literal(
            Type::base(BaseType::TypeDef("Point".to_string())),
            Expr::new_init_struct_in_order(vec![Expr::Int(1), Expr::Int(2)]),
        );
        assert_eq!(cl.to_string(), "(Point){1, 2}");

        let access = Expr::new_mem_access_with_str(cl, "x");
        assert_eq!(access.to_string(), "(Point){1, 2}.x");
    }

    #[test]
    fn encoding_prefixed_literals() {
        assert_eq!(
            Expr::new_str_lit(EncodingPrefix::Wide, "hi").to_string(),
            "L\"hi\""
        );
        assert_eq!(
            Expr::new_str_lit(EncodingPrefix::Utf8, "hi").to_string(),
            "u8\"hi\""
        );
        assert_eq!(
            Expr::new_str_lit(EncodingPrefix::Utf16, "hi").to_string(),
            "u\"hi\""
        );
        assert_eq!(
            Expr::new_str_lit(EncodingPrefix::Utf32, "hi").to_string(),
            "U\"hi\""
        );
        assert_eq!(
            Expr::new_char_lit(EncodingPrefix::Wide, 'x').to_string(),
            "L'x'"
        );
        assert_eq!(
            Expr::new_str_lit(EncodingPrefix::Wide, "a\"b").to_string(),
            "L\"a\\\"b\""
        );
    }

    #[test]
    fn alignof_and_offsetof() {
        let a = Expr::new_alignof(Type::base(BaseType::Int));
        assert_eq!(a.to_string(), "_Alignof(int)");
        let c23 = render(
            &a,
            RenderOptions {
                c23_keywords: true,
                ..Default::default()
            },
        );
        assert_eq!(c23, "alignof(int)");

        let o = Expr::new_offsetof(Type::base(BaseType::Struct("S".to_string())), "field");
        assert_eq!(o.to_string(), "offsetof(struct S, field)");
    }

    #[test]
    fn prefix_and_postfix_inc_dec() {
        let id = || Expr::new_ident_with_str("a");
        assert_eq!(Expr::new_unary(id(), UnaryOp::Inc).to_string(), "a++");
        assert_eq!(Expr::new_unary(id(), UnaryOp::Dec).to_string(), "a--");
        assert_eq!(Expr::new_unary(id(), UnaryOp::PreInc).to_string(), "++a");
        assert_eq!(Expr::new_unary(id(), UnaryOp::PreDec).to_string(), "--a");

        let deref_post = Expr::new_unary(
            Expr::new_unary(Expr::new_ident_with_str("p"), UnaryOp::Inc),
            UnaryOp::Deref,
        );
        assert_eq!(deref_post.to_string(), "*p++");
    }

    #[test]
    fn precedence_parenthesization() {
        let a = || Box::new(Expr::Ident("a".to_string()));
        let b = || Box::new(Expr::Ident("b".to_string()));
        let c = || Box::new(Expr::Ident("c".to_string()));

        // Tighter child on the right needs no parens
        let e = Expr::Binary {
            left: a(),
            op: BinOp::Add,
            right: Box::new(Expr::Binary {
                left: b(),
                op: BinOp::Mul,
                right: c(),
            }),
        };
        assert_eq!(e.to_string(), "a + b * c");

        // Looser child forced tighter needs parens
        let e = Expr::Binary {
            left: Box::new(Expr::Binary {
                left: a(),
                op: BinOp::Add,
                right: b(),
            }),
            op: BinOp::Mul,
            right: c(),
        };
        assert_eq!(e.to_string(), "(a + b) * c");

        let left_nested = Expr::Binary {
            left: Box::new(Expr::Binary {
                left: a(),
                op: BinOp::Sub,
                right: b(),
            }),
            op: BinOp::Sub,
            right: c(),
        };
        assert_eq!(left_nested.to_string(), "a - b - c");

        let right_nested = Expr::Binary {
            left: a(),
            op: BinOp::Sub,
            right: Box::new(Expr::Binary {
                left: b(),
                op: BinOp::Sub,
                right: c(),
            }),
        };
        assert_eq!(right_nested.to_string(), "a - (b - c)");
    }

    #[test]
    fn precedence_unary_and_mixed() {
        let a = || Box::new(Expr::Ident("a".to_string()));
        let b = || Box::new(Expr::Ident("b".to_string()));

        // Dereference of a sum
        let e = Expr::Unary {
            op: UnaryOp::Deref,
            expr: Box::new(Expr::Binary {
                left: a(),
                op: BinOp::Add,
                right: b(),
            }),
        };
        assert_eq!(e.to_string(), "*(a + b)");

        // Negation of a negation must not fuse into `--`
        let e = Expr::Unary {
            op: UnaryOp::Neg,
            expr: Box::new(Expr::Unary {
                op: UnaryOp::Neg,
                expr: a(),
            }),
        };
        assert_eq!(e.to_string(), "- -a");

        // Call through a function pointer
        let e = Expr::FnCall {
            name: Box::new(Expr::Unary {
                op: UnaryOp::Deref,
                expr: Box::new(Expr::Ident("fp".to_string())),
            }),
            args: vec![*a()],
        };
        assert_eq!(e.to_string(), "(*fp)(a)");

        // Member access on a dereference
        let e = Expr::MemAccess {
            expr: Box::new(Expr::Unary {
                op: UnaryOp::Deref,
                expr: Box::new(Expr::Ident("p".to_string())),
            }),
            member: "x".to_string(),
        };
        assert_eq!(e.to_string(), "(*p).x");
    }

    #[test]
    fn precedence_assign_and_ternary() {
        let a = || Box::new(Expr::Ident("a".to_string()));
        let b = || Box::new(Expr::Ident("b".to_string()));
        let c = || Box::new(Expr::Ident("c".to_string()));

        let e = Expr::Assign {
            lvalue: a(),
            op: AssignOp::Assign,
            value: Box::new(Expr::Assign {
                lvalue: b(),
                op: AssignOp::Assign,
                value: c(),
            }),
        };
        assert_eq!(e.to_string(), "a = b = c");

        let e = Expr::Ternary {
            cond: a(),
            lexpr: b(),
            rexpr: Box::new(Expr::Assign {
                lvalue: c(),
                op: AssignOp::Assign,
                value: Box::new(Expr::Int(0)),
            }),
        };
        assert_eq!(e.to_string(), "a ? b : (c = 0)");
    }

    #[test]
    fn sizeof() {
        let s = Expr::new_sizeof(Type::new(BaseType::Struct("some_struct".to_string())).build());
        let res = "sizeof(struct some_struct)";
        assert_eq!(s.to_string(), res);
    }

    #[test]
    fn init_arr() {
        let i = Expr::new_init_arr_in_order(vec![Expr::Int(1), Expr::Int(3), Expr::Int(2)]);
        let res = "{1, 3, 2}";
        assert_eq!(i.to_string(), res);

        let i2 = Expr::new_init_arr_designated(
            vec![0, 1, 2],
            vec![Expr::Float(1.1), Expr::Float(2.1), Expr::Float(4.4)],
        );
        let res2 = "{[0]=1.1f, [1]=2.1f, [2]=4.4f}";
        assert_eq!(i2.to_string(), res2);
    }

    #[test]
    fn init_struct() {
        let i = Expr::new_init_struct_in_order(vec![
            Expr::Str("abc".to_string()),
            Expr::Int(15),
            Expr::Char('x'),
        ]);
        let res = "{\"abc\", 15, 'x'}";
        assert_eq!(i.to_string(), res);

        let i2 = Expr::new_init_struct_designated(
            vec!["name".to_string(), "age".to_string()],
            vec![Expr::Str("bichanna".to_string()), Expr::Int(18)],
        );
        let res2 = "{.name=\"bichanna\", .age=18}";
        assert_eq!(i2.to_string(), res2);
    }

    #[test]
    fn uint_suffix() {
        assert_eq!(Expr::UInt(42).to_string(), "42u");
        assert_eq!(
            Expr::UInt(18446744073709551615).to_string(),
            "18446744073709551615u"
        );
    }

    #[test]
    fn string_escaping() {
        assert_eq!(
            Expr::Str("a\"b\\c\nd".to_string()).to_string(),
            r#""a\"b\\c\nd""#
        );
        assert_eq!(Expr::Str("x\0y".to_string()).to_string(), r#""x\000y""#);
    }

    #[test]
    fn char_escaping() {
        assert_eq!(Expr::Char('\n').to_string(), r"'\n'");
        assert_eq!(Expr::Char('\'').to_string(), r"'\''");
        assert_eq!(Expr::Char('\\').to_string(), r"'\\'");
        assert_eq!(Expr::Char('a').to_string(), "'a'");
    }

    #[test]
    fn float_literals() {
        assert_eq!(Expr::Float(1.0).to_string(), "1.0f");
        assert_eq!(Expr::Double(2.0).to_string(), "2.0");
        assert_eq!(Expr::Float(0.5).to_string(), "0.5f");
        assert_eq!(Expr::Double(3.5).to_string(), "3.5");
        assert_eq!(Expr::Double(f64::INFINITY).to_string(), "INFINITY");
        assert_eq!(Expr::Double(f64::NEG_INFINITY).to_string(), "-INFINITY");
        assert_eq!(Expr::Float(f32::NAN).to_string(), "NAN");
    }
}
