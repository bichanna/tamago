# Tamago

Tamago is a code generator library for C, written in Rust. It is designed to
simplify the process of generating C code programmatically, leveraging Rust's
safety and expressiveness. This crate makes heavy use of the builder pattern to
provide a pretty API (I hope) for constructing C code structures.

## Features

- Generate C code programmatically with a type-safe Rust API.
- The builder pattern throughout, for readable, self-documenting construction.
- Precedence-aware expressions: only the parentheses a grouping needs are
  emitted (`a * b + c`, but `(a + b) * c`).
- Attributes in GNU (`__attribute__((packed))`) or C23 (`[[gnu::packed]]`) style.
- Lightweight, with a small and focused API.

## Installation

Add `tamago` to your project by including it in your `Cargo.toml`:

```toml
[dependencies]
tamago = "0.2.0"  # replace with the actual version
```

## Usage

This example builds a small packed tagged-union type and an accessor, then lets
Tamago split it into a header and a source file.

```rust
use tamago::*;

fn main() {
    let v = || Expr::new_ident_with_str("v");

    // struct __attribute__((packed)) Value { int tag; union { int i; double d; }; };
    let value_struct = StructBuilder::new_with_str("Value")
        .attr(Attribute::packed())
        .field(FieldBuilder::new_with_str("tag", Type::base(BaseType::Int)).build())
        .field(
            Field::anonymous_union(vec![
                FieldBuilder::new_with_str("i", Type::base(BaseType::Int)).build(),
                FieldBuilder::new_with_str("d", Type::base(BaseType::Double)).build(),
            ])
            .build(),
        )
        .build();

    // typedef struct Value Value;
    let value_typedef =
        TypeDefBuilder::new_with_str(Type::base(BaseType::Struct("Value".to_string())), "Value")
            .build();

    // double value_as_double(const Value *v) {
    //     return v->tag ? v->d : (double)v->i;
    // }
    let as_double = FunctionBuilder::new_with_str("value_as_double", Type::base(BaseType::Double))
        .param(
            ParameterBuilder::new_with_str(
                "v",
                Type::new(BaseType::TypeDef("Value".to_string()))
                    .make_const()
                    .make_pointer()
                    .build(),
            )
            .build(),
        )
        .body(
            BlockBuilder::new()
                .statement(Statement::Return(Some(Expr::new_ternary(
                    Expr::new_ptr_mem_access_with_str(v(), "tag"),
                    Expr::new_ptr_mem_access_with_str(v(), "d"),
                    Expr::new_cast(
                        Type::base(BaseType::Double),
                        Expr::new_ptr_mem_access_with_str(v(), "i"),
                    ),
                ))))
                .build(),
        )
        .build();

    let m = Module::new("value")
        .struct_(value_struct)
        .typedef(value_typedef)
        .header_newline()
        .function(as_double)
        .build();

    print!("{}", m.header());
    print!("{}", m.source());
}
```

The generated header (`value.h`):

```c
#ifndef VALUE_H
#define VALUE_H

struct __attribute__((packed)) Value {
  int tag;
  union {
    int i;
    double d;
  };
};
typedef struct Value Value;

double value_as_double(const Value *v);

#endif /* VALUE_H */
```

And the generated source (`value.c`):

```c
#include "value.h"

double value_as_double(const Value *v) {
  return v->tag ? v->d : (double)v->i;
}
```

To emit the attributes in C23 style (`[[gnu::packed]]`) or to interleave
`#line` directives that map back to your own source, render through
`render(&item, RenderOptions { attr_style: AttrStyle::C23, line_directives: true })`
instead of `to_string()`.
