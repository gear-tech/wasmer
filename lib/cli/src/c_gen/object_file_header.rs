//! Generate a header file for the object file produced by the ObjectFile engine.

use super::{generate_c, CStatement, CType};
use wasmer_compiler::{Symbol, SymbolRegistry};
use wasmer_vm::ModuleInfo;

/// Helper functions to simplify the usage of the object file engine.
const HELPER_FUNCTIONS: &str = r#"
wasm_byte_vec_t generate_serialized_data() {
        // We need to pass all the bytes as one big buffer so we have to do all this logic to memcpy
        // the various pieces together from the generated header file.
        //
        // We should provide a `deseralize_vectored` function to avoid requiring this extra work.

        char* byte_ptr = (char*)&WASMER_METADATA[0];

        size_t num_function_pointers
                = sizeof(function_pointers) / sizeof(void*);
        size_t num_function_trampolines
                = sizeof(function_trampolines) / sizeof(void*);
        size_t num_dynamic_function_trampoline_pointers
                = sizeof(dynamic_function_trampoline_pointers) / sizeof(void*);


        size_t buffer_size = module_bytes_len
                + sizeof(size_t) + sizeof(function_pointers)
                + sizeof(size_t) + sizeof(function_trampolines)
                + sizeof(size_t) + sizeof(dynamic_function_trampoline_pointers);

        char* memory_buffer = (char*) malloc(buffer_size);
        size_t current_offset = 0;

        memcpy(memory_buffer + current_offset, byte_ptr, module_bytes_len);
        current_offset += module_bytes_len;

        memcpy(memory_buffer + current_offset, (void*)&num_function_pointers, sizeof(size_t));
        current_offset += sizeof(size_t);

        memcpy(memory_buffer + current_offset, (void*)&function_pointers[0], sizeof(function_pointers));
        current_offset += sizeof(function_pointers);

        memcpy(memory_buffer + current_offset, (void*)&num_function_trampolines, sizeof(size_t));
        current_offset += sizeof(size_t);

        memcpy(memory_buffer + current_offset, (void*)&function_trampolines[0], sizeof(function_trampolines));
        current_offset += sizeof(function_trampolines);

        memcpy(memory_buffer + current_offset, (void*)&num_dynamic_function_trampoline_pointers, sizeof(size_t));
        current_offset += sizeof(size_t);

        memcpy(memory_buffer + current_offset, (void*)&dynamic_function_trampoline_pointers[0], sizeof(dynamic_function_trampoline_pointers));
        current_offset += sizeof(dynamic_function_trampoline_pointers);

        wasm_byte_vec_t module_byte_vec = {
                .size = buffer_size,
                .data = memory_buffer,
        };
        return module_byte_vec;
}

wasm_module_t* wasmer_object_file_engine_new(wasm_store_t* store, const char* wasm_name) {
        // wasm_name intentionally unused for now: will be used in the future.
        wasm_byte_vec_t module_byte_vec = generate_serialized_data();
        wasm_module_t* module = wasm_module_deserialize(store, &module_byte_vec);
        free(module_byte_vec.data);

        return module;
}
"#;

/// Generate the header file that goes with the generated object file.
pub fn generate_header_file(
    module_info: &ModuleInfo,
    symbol_registry: &dyn SymbolRegistry,
    metadata_length: usize,
) -> String {
    let mut c_statements = vec![];
    c_statements.push(CStatement::LiteralConstant {
        value: "#include <stdlib.h>\n#include <string.h>\n\n".to_string(),
    });
    c_statements.push(CStatement::LiteralConstant {
        value: "#ifdef __cplusplus\nextern \"C\" {\n#endif\n\n".to_string(),
    });
    c_statements.push(CStatement::Declaration {
        name: "module_bytes_len".to_string(),
        is_extern: false,
        is_const: true,
        ctype: CType::U32,
        definition: Some(Box::new(CStatement::LiteralConstant {
            value: metadata_length.to_string(),
        })),
    });
    c_statements.push(CStatement::Declaration {
        name: "WASMER_METADATA".to_string(),
        is_extern: true,
        is_const: true,
        ctype: CType::Array { inner: Box::new(CType::U8) },
        definition: None,
    });
    let function_declarations = module_info
        .functions
        .iter()
        .filter_map(|(f_index, sig_index)| {
            Some((module_info.local_func_index(f_index)?, sig_index))
        })
        .map(|(function_local_index, _sig_index)| {
            let function_name =
                symbol_registry.symbol_to_name(Symbol::LocalFunction(function_local_index));
            // TODO: figure out the signature here too
            CStatement::Declaration {
                name: function_name,
                is_extern: false,
                is_const: false,
                ctype: CType::Function { arguments: vec![CType::Void], return_value: None },
                definition: None,
            }
        });
    c_statements.push(CStatement::LiteralConstant {
        value: r#"
// Compiled Wasm function pointers ordered by function index: the order they
// appeared in in the Wasm module.
"#
        .to_string(),
    });
    c_statements.extend(function_declarations);

    // function pointer array
    {
        let function_pointer_array_statements = module_info
            .functions
            .iter()
            .filter_map(|(f_index, sig_index)| {
                Some((module_info.local_func_index(f_index)?, sig_index))
            })
            .map(|(function_local_index, _sig_index)| {
                let function_name =
                    symbol_registry.symbol_to_name(Symbol::LocalFunction(function_local_index));
                // TODO: figure out the signature here too

                CStatement::Cast {
                    target_type: CType::void_ptr(),
                    expression: Box::new(CStatement::LiteralConstant { value: function_name }),
                }
            })
            .collect::<Vec<_>>();

        c_statements.push(CStatement::Declaration {
            name: "function_pointers".to_string(),
            is_extern: false,
            is_const: true,
            ctype: CType::Array { inner: Box::new(CType::void_ptr()) },
            definition: Some(Box::new(CStatement::LiteralArray {
                items: function_pointer_array_statements,
            })),
        });
    }

    let func_trampoline_declarations =
        module_info.signatures.iter().map(|(sig_index, _func_type)| {
            let function_name =
                symbol_registry.symbol_to_name(Symbol::FunctionCallTrampoline(sig_index));

            CStatement::Declaration {
                name: function_name,
                is_extern: false,
                is_const: false,
                ctype: CType::Function {
                    arguments: vec![CType::void_ptr(), CType::void_ptr(), CType::void_ptr()],
                    return_value: None,
                },
                definition: None,
            }
        });
    c_statements.push(CStatement::LiteralConstant {
        value: r#"
// Trampolines (functions by which we can call into Wasm) ordered by signature.
// There is 1 trampoline per function signature in the order they appear in
// the Wasm module.
"#
        .to_string(),
    });
    c_statements.extend(func_trampoline_declarations);

    // function trampolines
    {
        let function_trampoline_statements = module_info
            .signatures
            .iter()
            .map(|(sig_index, _vm_shared_index)| {
                let function_name =
                    symbol_registry.symbol_to_name(Symbol::FunctionCallTrampoline(sig_index));
                CStatement::LiteralConstant { value: function_name }
            })
            .collect::<Vec<_>>();

        c_statements.push(CStatement::Declaration {
            name: "function_trampolines".to_string(),
            is_extern: false,
            is_const: true,
            ctype: CType::Array { inner: Box::new(CType::void_ptr()) },
            definition: Some(Box::new(CStatement::LiteralArray {
                items: function_trampoline_statements,
            })),
        });
    }

    let dyn_func_declarations =
        module_info.functions.keys().take(module_info.num_imported_functions).map(|func_index| {
            let function_name =
                symbol_registry.symbol_to_name(Symbol::DynamicFunctionTrampoline(func_index));
            // TODO: figure out the signature here
            CStatement::Declaration {
                name: function_name,
                is_extern: false,
                is_const: false,
                ctype: CType::Function {
                    arguments: vec![CType::void_ptr(), CType::void_ptr(), CType::void_ptr()],
                    return_value: None,
                },
                definition: None,
            }
        });
    c_statements.push(CStatement::LiteralConstant {
        value: r#"
// Dynamic trampolines are per-function and are used for each function where
// the type signature is not known statically. In this case, this corresponds to
// the imported functions.
"#
        .to_string(),
    });
    c_statements.extend(dyn_func_declarations);

    c_statements.push(CStatement::TypeDef {
        source_type: CType::Function {
            arguments: vec![CType::void_ptr(), CType::void_ptr(), CType::void_ptr()],
            return_value: None,
        },
        new_name: "dyn_func_trampoline_t".to_string(),
    });

    // dynamic function trampoline pointer array
    {
        let dynamic_function_trampoline_statements = module_info
            .functions
            .keys()
            .take(module_info.num_imported_functions)
            .map(|func_index| {
                let function_name =
                    symbol_registry.symbol_to_name(Symbol::DynamicFunctionTrampoline(func_index));
                CStatement::LiteralConstant { value: function_name }
            })
            .collect::<Vec<_>>();
        c_statements.push(CStatement::Declaration {
            name: "dynamic_function_trampoline_pointers".to_string(),
            is_extern: false,
            is_const: true,
            ctype: CType::Array {
                inner: Box::new(CType::TypeDef("dyn_func_trampoline_t".to_string())),
            },
            definition: Some(Box::new(CStatement::LiteralArray {
                items: dynamic_function_trampoline_statements,
            })),
        });
    }

    c_statements.push(CStatement::LiteralConstant { value: HELPER_FUNCTIONS.to_string() });

    c_statements.push(CStatement::LiteralConstant {
        value: "\n#ifdef __cplusplus\n}\n#endif\n\n".to_string(),
    });

    generate_c(&c_statements)
}
