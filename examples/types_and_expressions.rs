//! The declarator engine (pointers, arrays, function pointers nest correctly)
//! and precedence-aware expression printing (only the parentheses a grouping
//! needs are emitted).
//!
//! Run with: `cargo run --example types_and_expressions`

use tamago::*;

fn main() {
    // ----- Derived types -----
    // typedef void (*Callback)(int, char);
    let callback = TypeDefBuilder::new_with_str(
        Type::ptr(Type::func(
            Type::base(BaseType::Void),
            vec![Type::base(BaseType::Int), Type::base(BaseType::Char)],
            false,
        )),
        "Callback",
    )
    .build();

    // int (*grid)[8];  -- pointer to an array of 8 ints
    let grid = VariableBuilder::new_with_str(
        "grid",
        Type::ptr(Type::array(Type::base(BaseType::Int), Some(Expr::Int(8)))),
    )
    .build();

    // char *argv[];  -- array of pointers to char
    let argv = VariableBuilder::new_with_str(
        "argv",
        Type::array(Type::ptr(Type::base(BaseType::Char)), None),
    )
    .build();

    let types = ScopeBuilder::new()
        .global_statement(GlobalStatement::TypeDef(callback))
        .global_statement(GlobalStatement::Variable(grid))
        .global_statement(GlobalStatement::Variable(argv))
        .build();

    println!("// ===== derived types =====");
    print!("{types}");

    // ----- Precedence-aware expressions -----
    let a = || Expr::new_ident_with_str("a");
    let b = || Expr::new_ident_with_str("b");
    let c = || Expr::new_ident_with_str("c");

    // a * b + c   (no parentheses needed)
    let e1 = Expr::new_binary(Expr::new_binary(a(), BinOp::Mul, b()), BinOp::Add, c());

    // (a + b) * c   (parentheses required to preserve grouping)
    let e2 = Expr::new_binary(Expr::new_binary(a(), BinOp::Add, b()), BinOp::Mul, c());

    // *(a + b)   (dereference of a sum)
    let e3 = Expr::new_unary(Expr::new_binary(a(), BinOp::Add, b()), UnaryOp::Deref);

    // node->next->value   (chained arrows stay flat)
    let e4 = Expr::new_ptr_mem_access_with_str(
        Expr::new_ptr_mem_access_with_str(Expr::new_ident_with_str("node"), "next"),
        "value",
    );

    println!("\n// ===== expressions =====");
    for e in [e1, e2, e3, e4] {
        println!("{e}");
    }
}
