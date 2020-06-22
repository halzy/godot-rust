#[macro_use]
extern crate serde_derive;

use proc_macro2::TokenStream;

use quote::{format_ident, quote};

pub mod api;
mod classes;
pub mod dependency;
mod documentation;
mod methods;
mod special_methods;

pub use crate::api::*;
use crate::classes::*;
pub use crate::dependency::*;
use crate::documentation::*;
use crate::methods::*;
use crate::special_methods::*;

use heck::SnakeCase as _;
use rayon::prelude::*;

use std::fs::File;
use std::io::{self, Write as _};
use std::path::PathBuf;

pub type GeneratorResult<T = ()> = Result<T, io::Error>;

pub fn generate_bindings(api: &mut Api, path: &PathBuf) -> TokenStream {
    api.classes.par_iter().for_each(|(_, class)| {
        if !class.is_generated {
            return;
        }

        let class_code = generate_class_bindings(&api, &class);

        let output = quote! {
            use libc;
            use std::sync::Once;
            use std::os::raw::c_char;
            use std::ptr;
            use std::mem;

            use gdnative_core::sys;
            use gdnative_core::*;
            use gdnative_core::private::get_api;
            use gdnative_core::object::PersistentRef;

            use crate::generated::*;

            #class_code
        };

        let module_name = class.name.to_snake_case();
        let mut module_path = path.clone();
        module_path.push(module_name);
        module_path.set_extension("rs");
        let file = File::create(module_path).expect("Should be able to open file");
        write!(&file, "{}", output).expect("Should be able to write");
    });

    let modules = api.classes.iter().filter_map(|(_, class)| {
        if !class.is_generated {
            return None;
        }

        let module = format_ident!("{}", class.name.to_snake_case());
        let class_name = format_ident!("{}", class.name);
        let enums = class.enums.iter().map(|e| {
            let enum_name = format_ident!("{}{}", class.name, e.name);
            quote! { pub use crate::generated::#module::#enum_name; }
        });

        Some(quote! {
            pub mod #module;
            pub use crate::generated::#module::#class_name;
            #(#enums)*
        })
    });

    quote! {
        #(#modules)*
    }
}

pub fn generate_class(class_name: &str) -> TokenStream {
    let api = Api::new();

    let class = api.find_class(class_name);

    if let Some(class) = class {
        generate_class_bindings(&api, &class)
    } else {
        Default::default()
    }
}

fn generate_class_bindings(api: &Api, class: &GodotClass) -> TokenStream {
    // types and methods
    let types_and_methods = {
        let documentation = generate_class_documentation(&api, class);

        let class_struct = generate_class_struct(class);

        let enums = generate_enums(class);

        let constants = if !class.constants.is_empty() {
            generate_class_constants(class)
        } else {
            Default::default()
        };

        let class_impl = generate_class_impl(&api, class);

        quote! {
            #documentation
            #class_struct
            #enums
            #constants
            #class_impl
        }
    };

    // traits
    let traits = {
        let object_impl = generate_godot_object_impl(class);

        let free_impl = generate_queue_free_impl(&api, class);

        let base_class = if !class.base_class.is_empty() {
            generate_deref_impl(class)
        } else {
            Default::default()
        };

        let mem_type = if class.is_refcounted() {
            generate_impl_ref_counted(class)
        } else {
            generate_impl_manually_managed(class)
        };

        // Instantiable
        let instantiable = if class.instantiable {
            generate_instantiable_impl(class)
        } else {
            Default::default()
        };

        quote! {
            #object_impl
            #free_impl
            #base_class
            #mem_type
            #instantiable
        }
    };

    // methods and method table for classes with functions
    let methods_and_table = if class.instantiable || !class.methods.is_empty() {
        let table = generate_method_table(&api, class);

        let methods = class
            .methods
            .iter()
            .map(|method| generate_method_impl(&api, class, method));

        quote! {
            #table
            #(#methods)*
        }
    } else {
        Default::default()
    };

    quote! {
        #types_and_methods
        #traits
        #methods_and_table
    }
}

fn rust_safe_name(name: &str) -> proc_macro2::Ident {
    match name {
        "use" => format_ident!("_use"),
        "type" => format_ident!("_type"),
        "loop" => format_ident!("_loop"),
        "in" => format_ident!("_in"),
        "override" => format_ident!("_override"),
        "where" => format_ident!("_where"),
        name => format_ident!("{}", name),
    }
}

#[cfg(feature = "debug")]
#[cfg(test)]
pub(crate) mod test_prelude {
    use super::*;
    use std::io::{BufWriter, Write};

    macro_rules! validate_and_clear_buffer {
        ($buffer:ident) => {
            $buffer.flush().unwrap();
            let content = std::str::from_utf8($buffer.get_ref()).unwrap();
            if syn::parse_file(&content).is_err() {
                let mut code_file = std::env::temp_dir();
                code_file.set_file_name("bad_code.rs");
                std::fs::write(&code_file, &content).unwrap();
                panic!(
                    "Could not parse generated code. Check {}",
                    code_file.display()
                );
            }
            $buffer.get_mut().clear();
        };
    }

    #[test]
    fn sanity_test_generated_code() {
        let api = Api::new();
        let mut buffer = BufWriter::new(Vec::with_capacity(16384));
        for class in &api.classes {
            let code = generate_class_documentation(&api, &class);
            write!(&mut buffer, "{}", code).unwrap();
            write!(&mut buffer, "{}", quote! { struct Docs {} }).unwrap();
            validate_and_clear_buffer!(buffer);

            let code = generate_class_struct(&class);
            write!(&mut buffer, "{}", code).unwrap();
            validate_and_clear_buffer!(buffer);

            let code = generate_enums(&class);
            write!(&mut buffer, "{}", code).unwrap();
            validate_and_clear_buffer!(buffer);

            if !class.constants.is_empty() {
                let code = generate_class_constants(&class);
                write!(&mut buffer, "{}", code).unwrap();
                validate_and_clear_buffer!(buffer);
            }

            let code = generate_class_impl(&api, &class);
            write!(&mut buffer, "{}", code).unwrap();
            validate_and_clear_buffer!(buffer);

            // traits
            let code = generate_godot_object_impl(&class);
            write!(&mut buffer, "{}", code).unwrap();
            validate_and_clear_buffer!(buffer);

            let code = generate_queue_free_impl(&api, &class);
            write!(&mut buffer, "{}", code).unwrap();
            validate_and_clear_buffer!(buffer);

            if !class.base_class.is_empty() {
                let code = generate_deref_impl(&class);
                write!(&mut buffer, "{}", code).unwrap();
                validate_and_clear_buffer!(buffer);
            }

            // RefCounted
            if class.is_refcounted() {
                let code = generate_impl_ref_counted(&class);
                write!(&mut buffer, "{}", code).unwrap();
                validate_and_clear_buffer!(buffer);
            } else {
                let code = generate_impl_manually_managed(&class);
                write!(&mut buffer, "{}", code).unwrap();
                validate_and_clear_buffer!(buffer);
            }

            // Instantiable
            if class.instantiable {
                let code = generate_instantiable_impl(&class);
                write!(&mut buffer, "{}", code).unwrap();
                validate_and_clear_buffer!(buffer);
            }

            // methods and method table
            let code = generate_method_table(&api, &class);
            write!(&mut buffer, "{}", code).unwrap();
            validate_and_clear_buffer!(buffer);

            for method in &class.methods {
                let code = generate_method_impl(&api, &class, method);
                write!(&mut buffer, "{}", code).unwrap();
                validate_and_clear_buffer!(buffer);
            }
        }
    }
}
