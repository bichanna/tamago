//! Getting started: a `#include`, a `main` function, and printing the result.
//!
//! Run with: `cargo run --example getting_started`

use tamago::*;

fn main() {
    let main_fn = FunctionBuilder::new_with_str("main", Type::base(BaseType::Int))
        .body(
            BlockBuilder::new()
                .statement(Statement::Expr(Expr::new_fn_call_with_name(
                    "puts".to_string(),
                    vec![Expr::Str("Hello from Tamago!".to_string())],
                )))
                .statement(Statement::Return(Some(Expr::Int(0))))
                .build(),
        )
        .build();

    let program = ScopeBuilder::new()
        .global_statement(GlobalStatement::Include(
            IncludeBuilder::new_system_with_str("stdio.h").build(),
        ))
        .new_line()
        .global_statement(GlobalStatement::Function(main_fn))
        .build();

    print!("{program}");
}
