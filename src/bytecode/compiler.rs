use crate::bytecode::program::*;
use crate::ast::lexer::*;
use crate::ast::parser::*;

use std::collections::BTreeMap;

fn process_constval(constval: Const, const_map: &mut BTreeMap<Const, usize>) -> Vec<Instruction> {
   let loadconst_idx = if !const_map.contains_key(&constval) {
        let len = const_map.len();
        const_map.insert(constval, len);
        len
    } else {
        *const_map.get(&constval).unwrap()
    };

    return vec![Instruction::LoadConst(loadconst_idx)];
}

fn compile_expr(expr: &Expr, const_map: &mut BTreeMap<Const, usize>) -> Vec<Instruction> {
    match expr {
        //TODO change Expr to Const(Const::Integer) so that it 
        //becomes easier to do this const stuff
        Expr::IntegerValue(i) => {
            let constval = Const::Integer(*i);
            return process_constval(constval, const_map);
        },
        Expr::FloatValue(f) => {
            let constval = Const::Float(*f);
            return process_constval(constval, const_map);
        },
        Expr::BooleanValue(b) => {
            let constval = Const::Boolean(*b);
            return process_constval(constval, const_map);
        },
        Expr::StringValue(s) => {
            let constval = Const::String(s.clone());
            return process_constval(constval, const_map);
        },
        Expr::None => {
            let constval = Const::None;
            return process_constval(constval, const_map);         
        }
        Expr::MemberAccess(expr, name) => {
            let mut lhs_program: Vec<Instruction> = compile_expr(expr, const_map);
            let mut final_instructions = vec![];
            final_instructions.append(&mut lhs_program);
            final_instructions.push(Instruction::LoadAttr(name.clone()));
            return final_instructions
        }
        Expr::BinaryOperation(lhs, op, rhs) => {
            match op {
                Operator::And | Operator::Or |  Operator::Xor => {
                    let mut load_attr: Vec<Instruction> = match op {
                        Operator::And => vec![Instruction::LoadAttr(String::from("__and__"))],
                        Operator::Or => vec![Instruction::LoadAttr(String::from("__or__"))],
                        Operator::Xor => vec![Instruction::LoadAttr(String::from("__xor__"))],
                        _ => panic!("operator not implemented: {:?}", op),
                    };

                    let mut lhs_program: Vec<Instruction> = compile_expr(lhs, const_map);
                    let mut rhs_program: Vec<Instruction> = compile_expr(rhs, const_map);

                    let call = Instruction::CallFunction {
                        number_arguments: 1
                    };

                    let mut final_instructions = vec![];
                    final_instructions.append(&mut lhs_program);
                    final_instructions.append(&mut load_attr);
                    final_instructions.append(&mut rhs_program);
                    final_instructions.push(call);

                    return final_instructions;
                },
                _ => {
                    let mut lhs_program: Vec<Instruction> = compile_expr(lhs, const_map);
                    let mut rhs_program: Vec<Instruction> = compile_expr(rhs, const_map);
                    let mut final_instructions = vec![];

                    final_instructions.append(&mut lhs_program);
                    final_instructions.append(&mut rhs_program);
                    let opcode = match op {
                        Operator::Plus => Instruction::BinaryAdd,
                        Operator::Mod => Instruction::BinaryModulus,
                        Operator::Minus => Instruction::BinarySubtract,
                        Operator::Multiply => Instruction::BinaryMultiply,
                        Operator::Divide => Instruction::BinaryTrueDivision,
                        Operator::Less => Instruction::CompareLessThan,
                        Operator::Greater => Instruction::CompareGreaterThan,
                        Operator::Equals => Instruction::CompareEquals,
                        Operator::GreaterEquals => Instruction::CompareGreaterEquals,
                        Operator::LessEquals => Instruction::CompareLessEquals,
                        Operator::NotEquals => Instruction::CompareNotEquals,
                        _ => {
                            panic!("Operator not implemented: {:?}", op)
                        }
                    };
                    final_instructions.push(opcode);

                    return final_instructions;
                }
            }
        }
        Expr::UnaryExpression(op, rhs) => {
            let mut load_attr: Vec<Instruction> = match op {
                Operator::Plus => vec![Instruction::LoadAttr(String::from("__pos__"))],
                Operator::Not => vec![Instruction::LoadAttr(String::from("__not__"))],
                Operator::Minus => vec![Instruction::LoadAttr(String::from("__neg__"))],
                _ => panic!("operator not implemented: {:?}", op),
            };

            let mut rhs_program: Vec<Instruction> = compile_expr(rhs, const_map);
            let call = Instruction::CallFunction {
                number_arguments: 0
            };

            let mut final_instructions = vec![];
            final_instructions.append(&mut rhs_program);
            final_instructions.append(&mut load_attr);
            final_instructions.push(call);

            return final_instructions;
        }
        Expr::FunctionCall(fcall_expr, params) => {
            //setup order of params
            let mut final_instructions = vec![];

            let method_to_call_instrs: Vec<Instruction> = compile_expr(fcall_expr, const_map);
            final_instructions.extend(method_to_call_instrs);
            let len_params = params.len();
        
            for param_expr in params {
                final_instructions.append(&mut compile_expr(param_expr, const_map));
            }

            final_instructions.push(Instruction::CallFunction {
                number_arguments: len_params,
            });
            return final_instructions;
        },
        Expr::IndexAccess(expr, index) =>  {
            let mut final_instructions = vec![];
            let indexed_value: Vec<Instruction> = compile_expr(expr, const_map);
            let index_value: Vec<Instruction> = compile_expr(index, const_map);
            final_instructions.extend(indexed_value);
            final_instructions.extend(index_value);
            final_instructions.push(Instruction::IndexAccess);
            return final_instructions;
        }
        Expr::Array(exprs) => {
            let mut final_instructions = vec![];
            let number_elements = exprs.len();
            for expr in exprs {
                final_instructions.append(&mut compile_expr(expr, const_map));
            }

            final_instructions.push(Instruction::BuildList { number_elements });
            return final_instructions;
        },
        Expr::Variable(var_name) => vec![Instruction::UnresolvedLoadName(var_name.clone())],
        Expr::Parenthesized(_) => panic!("Parenthesized expr should not leak to compiler"),
        
    }
}

struct ConstAndIndex {
    constval: Const,
    index: usize
}

pub fn resolve_loads_stores(code: &mut CodeObject) {
    let mut names_indices = BTreeMap::new();

    for name in code.params.iter() {
        names_indices.insert(name.clone(), names_indices.len());
    }


    //Find all variable stores and set slots for each one of them
    for instruction in code.instructions.iter() {
        if let Instruction::UnresolvedStoreName(name) = instruction {
            if !names_indices.contains_key(name) {
                names_indices.insert(name.clone(), names_indices.len());
            }
        }
        if let Instruction::UnresolvedStoreAttr(attr) = instruction {
            if !names_indices.contains_key(attr) {
                names_indices.insert(attr.clone(), names_indices.len());
            }
        }
    }

    //Instead of storing values in string names (hashing strings is slooooooooooooooooow), store variables in
    //integer slots 
    let new_instructions: Vec<Instruction> = code.instructions.iter().map(|instruction| {
        return if let Instruction::UnresolvedLoadName(name) = instruction {
            match names_indices.get(name) {
                Some(idx) => Instruction::LoadName(*idx),
                None => {
                    let cur_size = names_indices.len();
                    names_indices.insert(name.clone(), cur_size);

                    if code.params.contains(&name) {
                        Instruction::LoadName(cur_size)
                    } else {
                        Instruction::LoadGlobal(cur_size)
                    }
                }
            }
        }
        else if let Instruction::UnresolvedStoreName(name) = instruction {
            let idx = names_indices.get(name).unwrap();
            Instruction::StoreName(*idx)
        }
        else if let Instruction::UnresolvedStoreAttr(name) = instruction {
            let idx = names_indices.get(name).unwrap();
            Instruction::StoreAttr(*idx)
        }
        else {
            instruction.clone()
        }
    }).collect();

    let mut indices_names = vec!["".into(); names_indices.len()];

    for (k, v) in names_indices.iter() {
        indices_names[*v] = k.clone();
    }

    code.instructions = new_instructions;
    code.names = indices_names;
}

pub fn compile_repl(ast: Vec<AST>) -> Program {

    let mut compiled = compile(ast);
    println!("{:?}", compiled.code_objects);
    let instructions = &mut compiled.code_objects.last_mut().unwrap().instructions;

    let last_pop_location = instructions.len() - 3;

    if let Instruction::PopTop = instructions[last_pop_location] {
        instructions.remove(last_pop_location);
        compiled.code_objects.last_mut().unwrap().instructions.pop();
    }
    return compiled;
}

pub fn compile(ast: Vec<AST>) -> Program {

    let mut all_results = vec![];
    let mut compile_result = compile_ast(ast, 0, &mut all_results, &mut BTreeMap::new());
    compile_result.main = true;
    resolve_loads_stores(&mut compile_result);
    
    /*for inst in compile_result.instructions.iter() {
        if let Instruction::LoadConst(x) = inst {
            println!("instr {:?} {:?}", inst, compile_result.consts[*x]);
        } else {
            println!("{:?}", inst);
        }
    }*/

    all_results.insert(0, compile_result);

    Program {
        version: 1,
        code_objects: all_results
    }
}


fn generate_assign_path(remaining_parts: &[String], is_first: bool) -> Vec<Instruction> {
    let mut instructions = vec![];
    match remaining_parts {
        [first, rest @ ..] => {
            if rest.len() > 0 {
                if is_first {
                    instructions.push(Instruction::UnresolvedLoadName(first.clone()));
                } else {
                    instructions.push(Instruction::LoadAttr(first.clone()));
                }
                instructions.extend(generate_assign_path(rest, false));
            } else {
                instructions.push(Instruction::UnresolvedStoreAttr(first.clone()));
            }
        },
        [] => {}
    }
    return instructions;
}

fn build_fully_qualified_name(prefix: Option<String>, name: &str) -> String {
    match prefix {
        Some(s) => (s + "." + name).to_string(),
        None => name.to_string()
    }
}

pub fn compile_ast_internal(ast: Vec<AST>, offset: usize, qualified_prefix: Option<String>, ensure_return: bool, results: &mut Vec<CodeObject>, const_map: &mut BTreeMap<Const, usize>) -> CodeObject {
    let mut all_instructions = vec![];
    for ast_item in ast {
        match ast_item {
            AST::Assign {
                path,
                expression,
            } => {
                all_instructions.append(&mut compile_expr(&expression, const_map));
                if path.len() == 1 {
                    all_instructions.push(Instruction::UnresolvedStoreName(path[0].clone()));
                } else {
                    let instructions_for_assign = generate_assign_path(path.as_slice(), true);
                    all_instructions.extend(instructions_for_assign);
                }
            }
            AST::StandaloneExpr(expr) => {
                all_instructions.append(&mut compile_expr(&expr, const_map));
                all_instructions.push(Instruction::PopTop);
            },
            AST::Return(Some(expr)) => {
                all_instructions.append(&mut compile_expr(&expr, const_map));
                all_instructions.push(Instruction::ReturnValue);
            }
            AST::Return(None) => {
                all_instructions.push(Instruction::LoadConst(const_map[&Const::None]));
                all_instructions.push(Instruction::ReturnValue);
            }
            AST::ClassDeclaration{class_name, body} => {
                let qualname = build_fully_qualified_name(qualified_prefix.clone(), &class_name);

                let mut new_const_map = BTreeMap::new();
                let mut class_decl_function = compile_ast_internal(body, 0, Some(qualname.clone()), true, results, &mut new_const_map);
                class_decl_function.main = false;
                resolve_loads_stores(&mut class_decl_function);
                let constval_code = Const::CodeObject(class_decl_function);
                let mut code_idx = process_constval(constval_code, const_map);
                let constval_name = Const::String(qualname.clone());
                let mut name_idx = process_constval(constval_name, const_map);

                all_instructions.append(&mut code_idx);
                all_instructions.append(&mut name_idx);
                all_instructions.push(Instruction::MakeClass);
                all_instructions.push(Instruction::UnresolvedStoreName(class_name.clone()));
            }
            AST::DeclareFunction{function_name, parameters, body} => {
                let qualname = build_fully_qualified_name(qualified_prefix.clone(), &function_name);

                let mut new_const_map = BTreeMap::new();
                let mut func_instructions = compile_ast_internal(body, 0, Some(qualname.clone()), true, results, &mut new_const_map);
                func_instructions.main = false;
               
                func_instructions.params = parameters.iter()
                    .map(|x| match x {
                        FunctionParameter::Simple(x) => x.clone(),
                        FunctionParameter::DefaultValue(x, _) => x.clone()
                    }).collect();

                //we must generate the bytecode for default values
                let mut number_of_default_parameters = 0;
                let mut default_instructions = vec![];
                for param in parameters.iter() {
                    if let FunctionParameter::DefaultValue(_, expr) = param {
                        let compiled_expr = compile_expr(expr, const_map);
                        default_instructions.extend(compiled_expr);
                        number_of_default_parameters += 1;
                    }
                }

                resolve_loads_stores(&mut func_instructions);

                let constval_code = Const::CodeObject(func_instructions);
                let mut code_idx = process_constval(constval_code, const_map);
                let constval_name = Const::String(qualname.clone());
                let mut name_idx = process_constval(constval_name, const_map);

                all_instructions.extend(default_instructions);
                all_instructions.push(Instruction::BuildList { number_elements:number_of_default_parameters });
                all_instructions.append(&mut code_idx);
                all_instructions.append(&mut name_idx);
                all_instructions.push(Instruction::MakeFunction(number_of_default_parameters > 0));
                all_instructions.push(Instruction::UnresolvedStoreName(function_name.clone()));
                
            }
            AST::ForStatement{item_name, list_expression, body} => {
                //this should behave like this:
                /*
                iterator = list_expression.__iter__()
                while True:
                    try:
                        item = iterator.__next__()
                        {body}
                    except err as StopException e:
                        break;

                */

                //I'd like to do this by transforming AST into a while statement,
                //but for now I don't have support for try/except in the AST (or bytecode),
                //although you can raise them. Would be slower than python's way, but it would be cool :)
                
                //let's just copy python then


                let list_expr_instructions = compile_expr(&list_expression, const_map);
                all_instructions.extend(list_expr_instructions);

                all_instructions.push(Instruction::LoadAttr("__iter__".into()));
                all_instructions.push(Instruction::CallFunction{ number_arguments: 0 });
                
                let offset_before_for = all_instructions.len() + offset;
                //The for iter will call _next__() on the iterator and push it to the stack
                //Need to compute the body first to get an offset
                //and then we add to the beginning of the loop the ForIter instruction

                let compiled_body = compile_ast_internal(body, 0, qualified_prefix.clone(), false, results, const_map);
                let mut body_instructions = vec![];
                body_instructions.push(Instruction::UnresolvedStoreName(item_name.clone()));
                body_instructions.extend(compiled_body.instructions);
                
                //+2 because we are considering the ForIter and JumpUnconditional instructions
                //before generating the instructions
                let offset_after_loop = offset_before_for + body_instructions.len() + 2;
                
                let mut compiled_body_with_resolved_breaks: Vec<Instruction> = body_instructions
                    .into_iter()
                    .map(|instr| -> Instruction {
                        if let Instruction::UnresolvedBreak = instr {
                            Instruction::JumpUnconditional(offset_after_loop)
                        } else {
                            instr
                        }
                    })
                    .collect();
                
                //create the loop now, pointing to the end of the loop
                compiled_body_with_resolved_breaks.insert(0, Instruction::ForIter(offset_after_loop));
                //this has to jump back to the ForIter instruction so it loops
                compiled_body_with_resolved_breaks.push(Instruction::JumpUnconditional(offset_before_for));
       
                all_instructions.extend(compiled_body_with_resolved_breaks);
            
            },
            AST::IfStatement {
                true_branch,
                elifs: _,
                final_else,
            } => {
                let mut if_expr_compiled = compile_expr(&true_branch.expression, const_map);
                all_instructions.append(&mut if_expr_compiled);

                //+1 is because there will be a instruction before
                //that will do the jump
                let offset_before_if = offset + all_instructions.len() + 1;

                let mut true_branch_compiled =
                    compile_ast_internal(true_branch.statements, offset_before_if, qualified_prefix.clone(), false, results, const_map);
                //generate a jump to the code right after the true branch

                //if there is an else: statement, the true branch must jump to after the false branch
                if let Some(else_ast) = final_else {
                    //+1 because where will be a jump unconditional that is *part* of the true branch

                    let offset_after_true_branch =
                        offset_before_if + true_branch_compiled.instructions.len() + 1;
                    all_instructions.push(Instruction::JumpIfFalseAndPopStack(
                        offset_after_true_branch,
                    ));
                    all_instructions.append(&mut true_branch_compiled.instructions);

                    let mut false_branch_compiled = compile_ast_internal(else_ast, offset_after_true_branch, qualified_prefix.clone(), false, results, const_map);

                    //+1 because there will be an instruction
                    //in the true branch that will jump to *after* the false branch
                    let offset_after_else =
                        offset_after_true_branch + false_branch_compiled.instructions.len();

                    all_instructions.push(Instruction::JumpUnconditional(offset_after_else));
                    all_instructions.append(&mut false_branch_compiled.instructions);
                } else {
                    let offset_after_true_branch = offset_before_if + true_branch_compiled.instructions.len();
                    all_instructions.push(Instruction::JumpIfFalseAndPopStack(
                        offset_after_true_branch,
                    ));
                    all_instructions.append(&mut true_branch_compiled.instructions);
                }
            }
            AST::WhileStatement { expression, body } => {
                let offset_before_while = all_instructions.len() + offset;
                let mut compiled_expr = compile_expr(&expression, const_map);
                //+1 for the jump if false
                let offset_after_expr = all_instructions.len() + compiled_expr.len() + 1;
                let compiled_body = compile_ast_internal(body, offset_after_expr, qualified_prefix.clone(), false, results, const_map);
                all_instructions.append(&mut compiled_expr);
                let offset_after_body = offset_after_expr + compiled_body.instructions.len() + 1;
                all_instructions.push(Instruction::JumpIfFalseAndPopStack(offset_after_body));

                let mut compiled_body_with_resolved_breaks: Vec<Instruction> = compiled_body.instructions
                    .into_iter()
                    .map(|instr| -> Instruction {
                        if let Instruction::UnresolvedBreak = instr {
                            Instruction::JumpUnconditional(offset_after_body)
                        } else {
                            instr
                        }
                    })
                    .collect();

                all_instructions.append(&mut compiled_body_with_resolved_breaks);
                all_instructions.push(Instruction::JumpUnconditional(offset_before_while));
            }
            AST::Raise(expr) => {
                let mut if_expr_compiled = compile_expr(&expr, const_map);
                all_instructions.append(&mut if_expr_compiled);
                all_instructions.push(Instruction::Raise);
                if !const_map.contains_key(&Const::None) {
                    const_map.insert(Const::None, const_map.len());
                }
                //println!("{:#?}", new_const_map);
                all_instructions.push(Instruction::LoadConst(const_map[&Const::None]));
                all_instructions.push(Instruction::ReturnValue);
            }
            AST::Break => {
                //In python there's something called a "block stack" and an opcode called POP_BLOCK
                //that makes this much easier, as well as a BREAK_LOOP instruction that uses block information
                //to break the current loop.
                //So Python really has a loooot of information about high-level language features even in the 
                //lower level layers...
                //But for me it's a more interesting problem to not use these instructions and just use plain jumps. 
                //However, when I find a break in the AST, I don't yet know what the program will look like,
                //and therefore I don't know where to jump. 
                //Perhaps other features such as generators, for comprehensions, etc really need blocks? I doubt it.
                all_instructions.push(Instruction::UnresolvedBreak);
            }
        }
    }

    make_code_object(all_instructions, qualified_prefix.unwrap_or("__main__".to_owned()), const_map, ensure_return)
}

pub fn compile_ast(ast: Vec<AST>, offset: usize, results: &mut Vec<CodeObject>, const_map: &mut BTreeMap<Const, usize>) -> CodeObject {
    compile_ast_internal(ast,offset,None,true,results,const_map)
}

fn make_code_object(instrs: Vec<Instruction>, name: String, const_map: &mut BTreeMap<Const, usize>, ensure_return: bool) -> CodeObject {

    let mut vec_const = vec![];
    for (constval, index) in const_map.iter() {
        vec_const.push(ConstAndIndex {
            constval: constval.clone(),
            index: *index
        })
    }
    vec_const.sort_unstable_by(|a, b| a.index.cmp(&b.index));

    let mut code_obj = CodeObject {
        instructions: instrs,
        names: vec![],
        params: vec![],
        consts: vec_const.into_iter().map(|x| x.constval).collect(),
        main: false,
        objname: name
    };

    if ensure_return {
        match code_obj.instructions.last().unwrap() {
            Instruction::ReturnValue => { /*unchanged*/ },
            _ => {
                if !const_map.contains_key(&Const::None) {
                    const_map.insert(Const::None, const_map.len());
                    code_obj.consts.push(Const::None);
                }
                //println!("{:#?}", new_const_map);
                code_obj.instructions.push(Instruction::LoadConst(const_map[&Const::None]));
                code_obj.instructions.push(Instruction::ReturnValue);
            }
        }
    }
    return code_obj;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin_types::*;
    use crate::runtime::interpreter;
    use crate::runtime::vm::VM;

    #[test]
    fn run_pytests() -> std::io::Result<()> {

        for entry in std::fs::read_dir("./pytests")? {
            let dir = entry?;
            println!("Loading source {:?}", dir.path());
            let source = std::fs::read_to_string(dir.path());
            let mut vm = VM::new();
            register_builtins(&mut vm);
            loader::run_loader(&mut vm);
            let tokens = tokenize(&source.unwrap()).unwrap();
            let expr = parse_ast(tokens);
            let program = compile(expr);
            interpreter::execute_program(&mut vm, program);
        }
        
        Ok(())
    }

    #[test]
    fn test_literal_int_1() {
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("1").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        let stack_pop = vm.get_stack_offset(-1);
        let stack_value = vm.get_raw_data_of_pyobj(stack_pop).take_int();
        assert_eq!(stack_value, 1);
    }

    #[test]
    fn test_literal_float_1() {
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("1.0").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        let stack_pop = vm.get_stack_offset(-1);
        let stack_value = vm.get_raw_data_of_pyobj(stack_pop).take_float();
        assert_eq!(stack_value, 1.0);
    }

    #[test]
    fn test_literal_boolean_true() {
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("True").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        let stack_pop = vm.get_stack_offset(-1);
        let stack_value = vm.get_raw_data_of_pyobj(stack_pop).take_int();
        assert_eq!(stack_value, 1);
    }

    #[test]
    fn test_literal_boolean_false() {
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("False").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        let stack_pop = vm.get_stack_offset(-1);
        let stack_value = vm.get_raw_data_of_pyobj(stack_pop).take_int();
        assert_eq!(stack_value, 0);
    }

    #[test]
    fn test_1_plus_1() {
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("1 + 1").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        let stack_pop = vm.get_stack_offset(-1);
        let stack_value = vm.get_raw_data_of_pyobj(stack_pop).take_int();
        assert_eq!(stack_value, 2);
    }

    #[test]
    fn test_1_plus_float_3_5() {
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("1 + 3.5").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        println!("program: {:?}", program.code_objects);
        interpreter::execute_program(&mut vm, program);
        let stack_pop = vm.get_stack_offset(-1);
        let stack_value = vm.get_raw_data_of_pyobj(stack_pop).take_float();
        assert_eq!(stack_value, 4.5);
    }

    #[test]
    fn test_neg() {
        //-(5.0 / 9.0)
        let expected_result = -(5.0_f64 / 9.0_f64);
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("-(5.0 / 9.0)").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        let stack_pop = vm.get_stack_offset(-1);
        let stack_value = vm.get_raw_data_of_pyobj(stack_pop).take_float();
        assert_eq!(stack_value, expected_result);
    }

    #[test]
    fn test_div_neg_mul() {
        //-(5.0 / 9.0) * 32)
        let expected_result = -(5.0_f64 / 9.0_f64) * 32.0_f64;
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("-(5.0 / 9.0) * 32.0").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        let stack_pop = vm.get_stack_offset(-1);
        let stack_value = vm.get_raw_data_of_pyobj(stack_pop).take_float();
        assert_eq!(stack_value, expected_result);
    }

    #[test]
    fn test_div_minus_div() {
        //(1 - (5.0 / 9.0))
        let expected_result = 1.0_f64 - (5.0_f64 / 9.0_f64);
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("1.0 - (5.0 / 9.0)").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        let stack_pop = vm.get_stack_offset(-1);
        let stack_value = vm.get_raw_data_of_pyobj(stack_pop).take_float();
        assert_eq!(stack_value, expected_result);
    }

    #[test]
    fn test_fahrenheit() {
        let expected_result = (-(5.0_f64 / 9.0_f64) * 32.0_f64) / (1.0_f64 - (5.0_f64 / 9.0_f64));
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("(-(5.0 / 9.0) * 32.0) / (1.0 - (5.0 / 9.0))").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        let stack_pop = vm.get_stack_offset(-1);
        let stack_value = vm.get_raw_data_of_pyobj(stack_pop).take_float();
        assert_eq!(stack_value, expected_result);
    }

    #[test]
    fn test_function_calls_with_complex_expr() {
        let expected_result = (-(5.0_f64 / 9.0_f64) * 32.0_f64).sin().cos()
            / (1.0_f64.cos() - (5.0_f64 / 9.0_f64)).tanh();
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens =
            tokenize("cos(sin(-(5.0 / 9.0) * 32.0)) / tanh(cos(1.0) - (5.0 / 9.0))").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        let stack_pop = vm.get_stack_offset(-1);
        let stack_value = vm.get_raw_data_of_pyobj(stack_pop).take_float();
        assert_eq!(stack_value, expected_result);
    }

    #[test]
    fn test_fcall() {
        let expected_result = 1.0_f64.sin();
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("sin(1.0)").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        let stack_pop = vm.get_stack_offset(-1);
        let stack_value = vm.get_raw_data_of_pyobj(stack_pop).take_float();
        assert_eq!(stack_value, expected_result);
    }

    #[test]
    fn test_bind_local() {
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("x = 1 + 2").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        let x = vm.get_local(0).unwrap();
        let stack_value = vm.get_raw_data_of_pyobj(x).take_int();
        assert_eq!(stack_value, 3);
    }

    #[test]
    fn test_string_concat() {
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("\"abc\" + 'cde'").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        let stack_top = vm.get_stack_offset(-1);
        let stack_value = vm.get_raw_data_of_pyobj(stack_top).take_string();
        assert_eq!(stack_value, "abccde");
    }

    #[test]
    fn boolean_and() {
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("True and False").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        let stack_top = vm.get_stack_offset(-1);
        let stack_value = vm.get_raw_data_of_pyobj(stack_top).take_int();
        assert_eq!(stack_value, 0);
    }
    use crate::runtime::datamodel::*;
    #[test]
    fn load_method_with_loadattr_instruction() -> Result<(), String> {
        use crate::runtime::vm::VM;
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("\"abc\".lower").unwrap();
        let expr = parse_ast(tokens);
        let program =  compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        let stack_top = vm.get_stack_offset(-1);
        let stack_value = vm.get_pyobj_byaddr(stack_top);
        match &stack_value.structure {
            PyObjectStructure::BoundMethod { .. } => {
                Ok(())                   
            },
            _ => {
                Err("Did not load attribute, which should be a bound method".into())
            }
        }
    }

    #[test]
    fn load_module_property_with_loadattr_instruction() -> Result<(), String> {
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("__builtins__.float").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        let stack_top = vm.get_stack_offset(-1);
        let stack_value = vm.get_pyobj_byaddr(stack_top);
        match &stack_value.structure {
            PyObjectStructure::Type {
                name, functions:_, supertype: _
            } => {
                if name == "float" {
                    Ok(())                   
                } else {
                    Err("Loaded an attribute with name != float from module __builtins__".into())
                }
            },
            _ => {
                Err("Did not load attribute, which should be a native type called float (from module __builtins__)".into())
            }
        }
    }


    #[test]
    fn runs_classdef() -> Result<(), String> {
        let mut vm = VM::new();
        register_builtins(&mut vm);
        let tokens = tokenize("
class SomeClass:
    def __init__(self):        
        self.x = 1
").unwrap();
        let expr = parse_ast(tokens);
        let program = compile_repl(expr);
        interpreter::execute_program(&mut vm, program);
        Ok(())
    }
}
