//! Sum types: a packed tagged union built from a C11 anonymous union, with a
//! pointer accessor that uses `->`, a ternary, and a cast.
//!
//! Run with: `cargo run --example tagged_union`

use tamago::*;

fn main() {
    let v = || Expr::new_ident_with_str("v");

    // struct __attribute__((packed)) Value {
    //   int tag;
    //   union { int i; double d; };   // anonymous: write v.i, not v.payload.i
    // };
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

    println!("// ===== value.h =====");
    print!("{}", m.header());
    println!("\n// ===== value.c =====");
    print!("{}", m.source());
}
