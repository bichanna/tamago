//! Attributes on functions, structs, fields, parameters, and variables, rendered
//! in both GNU (`__attribute__((...))`) and C23 (`[[gnu::...]]`) styles.
//!
//! Run with: `cargo run --example attributes`

use tamago::*;

fn main() {
    // __attribute__((noreturn)) void panic(__attribute__((unused)) const char *msg);
    let panic = FunctionBuilder::new_with_str("panic", Type::base(BaseType::Void))
        .attr(Attribute::noreturn())
        .param(
            ParameterBuilder::new_with_str("msg", Type::const_ptr(Type::base(BaseType::Char)))
                .attr(Attribute::unused())
                .build(),
        )
        .build();

    // A packed struct with an aligned field.
    let header = StructBuilder::new_with_str("Packet")
        .attr(Attribute::packed())
        .field(
            FieldBuilder::new_with_str("magic", Type::base(BaseType::UInt32))
                .attr(Attribute::aligned(4))
                .build(),
        )
        .field(FieldBuilder::new_with_str("len", Type::base(BaseType::UInt16)).build())
        .build();

    // A static variable placed in a named section.
    let boot_flag = VariableBuilder::new_with_str("boot_flag", Type::base(BaseType::Int))
        .make_static()
        .attr(Attribute::section(".boot"))
        .value(Expr::Int(1))
        .build();

    // Group them in one scope so declarations get their trailing semicolons.
    let scope = ScopeBuilder::new()
        .global_statement(GlobalStatement::Function(panic))
        .new_line()
        .global_statement(GlobalStatement::Struct(header))
        .new_line()
        .global_statement(GlobalStatement::Variable(boot_flag))
        .build();

    for style in [AttrStyle::Gnu, AttrStyle::C23] {
        println!("// ===== {style:?} =====");
        let opts = RenderOptions {
            attr_style: style,
            ..Default::default()
        };
        print!("{}", render(&scope, opts));
        println!();
    }
}
