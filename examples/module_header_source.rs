//! Header/source splitting: describe a translation unit once and get a matching
//! `.h` and `.c`. Function definitions become a prototype in the header and a
//! body in the source automatically.
//!
//! Run with: `cargo run --example module_header_source`

use tamago::*;

// Small helpers so the expression trees stay readable.
fn ident(name: &str) -> Expr {
    Expr::new_ident_with_str(name)
}
fn member(object: &str, field: &str) -> Expr {
    Expr::new_mem_access_with_str(ident(object), field)
}

fn main() {
    let float = Type::base(BaseType::Float);
    let vec2 = Type::base(BaseType::TypeDef("Vec2".to_string()));

    // struct Vec2 { float x; float y; };
    let vec2_struct = StructBuilder::new_with_str("Vec2")
        .field(FieldBuilder::new_with_str("x", float.clone()).build())
        .field(FieldBuilder::new_with_str("y", float.clone()).build())
        .build();

    // typedef struct Vec2 Vec2;
    let vec2_typedef =
        TypeDefBuilder::new_with_str(Type::base(BaseType::Struct("Vec2".to_string())), "Vec2")
            .build();

    // Vec2 vec2_add(Vec2 a, Vec2 b) { return (Vec2){a.x + b.x, a.y + b.y}; }
    let add = FunctionBuilder::new_with_str("vec2_add", vec2.clone())
        .param(ParameterBuilder::new_with_str("a", vec2.clone()).build())
        .param(ParameterBuilder::new_with_str("b", vec2.clone()).build())
        .body(
            BlockBuilder::new()
                .statement(Statement::Return(Some(Expr::new_cast(
                    vec2.clone(),
                    Expr::new_init_struct_in_order(vec![
                        Expr::new_binary(member("a", "x"), BinOp::Add, member("b", "x")),
                        Expr::new_binary(member("a", "y"), BinOp::Add, member("b", "y")),
                    ]),
                ))))
                .build(),
        )
        .build();

    // float vec2_dot(Vec2 a, Vec2 b) { return a.x * b.x + a.y * b.y; }
    let dot = FunctionBuilder::new_with_str("vec2_dot", float.clone())
        .param(ParameterBuilder::new_with_str("a", vec2.clone()).build())
        .param(ParameterBuilder::new_with_str("b", vec2.clone()).build())
        .body(
            BlockBuilder::new()
                .statement(Statement::Return(Some(Expr::new_binary(
                    Expr::new_binary(member("a", "x"), BinOp::Mul, member("b", "x")),
                    BinOp::Add,
                    Expr::new_binary(member("a", "y"), BinOp::Mul, member("b", "y")),
                ))))
                .build(),
        )
        .build();

    let m = Module::new("vec2")
        .struct_(vec2_struct)
        .typedef(vec2_typedef)
        .header_newline()
        .function(add)
        .function(dot)
        .build();

    println!("// ===== vec2.h =====");
    print!("{}", m.header());
    println!("\n// ===== vec2.c =====");
    print!("{}", m.source());
}
