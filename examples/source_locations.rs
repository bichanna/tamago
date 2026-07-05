//! Source-location mapping: tag statements with a [`SourceLoc`] and render with
//! line directives so the generated C points back at your own source.
//!
//! Run with: `cargo run --example source_locations`

use tamago::*;

fn main() {
    let body = BlockBuilder::new()
        .statement(
            Statement::Variable(
                VariableBuilder::new_with_str("x", Type::base(BaseType::Int))
                    .value(Expr::Int(1))
                    .build(),
            )
            .located(SourceLoc::new("calc.lang", 4)),
        )
        .statement(
            Statement::Return(Some(Expr::new_binary(
                Expr::new_ident_with_str("x"),
                BinOp::Add,
                Expr::Int(41),
            )))
            .located(SourceLoc::new("calc.lang", 5)),
        )
        .build();

    let answer = FunctionBuilder::new_with_str("answer", Type::base(BaseType::Int))
        .body(body)
        .build();

    println!("// ===== plain (locations ignored) =====");
    print!("{answer}");

    println!("\n// ===== with #line directives =====");
    let opts = RenderOptions {
        line_directives: true,
        ..Default::default()
    };
    print!("{}", render(&answer, opts));
}
