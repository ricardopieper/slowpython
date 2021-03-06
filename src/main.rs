mod ast;
mod commons;
mod builtin_types;
mod bytecode;
#[macro_use]
mod runtime;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use std::env;
use std::fs;
use crate::ast::lexer;
use crate::ast::parser;

fn main() {
    let mut vm = runtime::vm::VM::new();
    builtin_types::register_builtins(&mut vm);
    builtin_types::loader::run_loader(&mut vm);
    let args: Vec<String> = env::args().collect();

    if args.len() == 2 {
        let input =
            fs::read_to_string(args[1].clone()).expect(&format!("Could not read file {}", args[1]));
        let tokens = lexer::tokenize(input.as_str());
        //println!("Tokens: {:?}", tokens);
        let ast = parser::parse_ast(tokens.unwrap());

        let program = bytecode::compiler::compile(ast);
        runtime::interpreter::execute_program(&mut vm, program);
       
        return;
    }

    println!(
        "horse 0.0.1 (rustc {})",
        rustc_version_runtime::version()
    );
    println!("No help, copyright or licensing commands available. You're on your own.");
    let mut rl = Editor::<()>::new();
    if rl.load_history("history.txt").is_err() {
        println!("No previous history.");
    }
    loop {
        let readline = rl.readline(">>> ");
        match readline {
            Ok(input) => {
                rl.add_history_entry(input.as_str());
                if input == "\n" {
                    continue;
                }
                if input == "exit\n" {
                    return;
                }
                let tokens = lexer::tokenize(input.as_str());
                let ast = parser::parse_ast(tokens.unwrap());
                let program = bytecode::compiler::compile_repl(ast);
                runtime::interpreter::execute_program(&mut vm, program);
                let result_addr = vm.get_stack_offset(-1);
                let result_string = vm.call_method(result_addr, "__repr__", runtime::vm::PositionalParameters::empty());
                match result_string {
                    None => {}
                    Some((addr, _)) => {
                        let pyobj_str = vm.get_raw_data_of_pyobj(addr).take_string();
                        println!("{}", pyobj_str);
                    }
                }

                vm.set_pc(0);
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    rl.save_history("history.txt").unwrap();
}
