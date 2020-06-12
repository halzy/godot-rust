#[macro_use]
extern crate serde_derive;

pub mod api;
mod classes;
pub mod dependency;
mod documentation;
mod methods;
mod special_methods;

use std::collections::HashSet;
use std::io::Write;

pub use crate::api::*;
use crate::classes::*;
pub use crate::dependency::*;
use crate::documentation::*;
use crate::methods::*;
use crate::special_methods::*;

use std::io;

pub type GeneratorResult<T = ()> = Result<T, io::Error>;

pub fn generate_bindings(
    output_types_impls: &mut impl Write,
    output_trait_impls: &mut impl Write,
    output_method_table: &mut impl Write,
    table_name: &str,
    ignore: Option<HashSet<String>>,
) -> GeneratorResult {
    let to_ignore = ignore.unwrap_or_default();

    let api = Api::new();
    let classes: Vec<&GodotClass> = api
        .classes
        .iter()
        .filter(|class| !to_ignore.contains(&class.name))
        .collect();

    generate_imports(output_types_impls)?;

    generate_method_table(output_method_table, table_name, &api, classes.as_slice())?;

    for class in &classes {
        // ignore classes that have been generated before.
        if to_ignore.contains(&class.name) {
            continue;
        }

        generate_class_bindings(
            output_types_impls,
            output_trait_impls,
            output_method_table,
            "BINDINGS_METHOD_TABLE",
            "",
            &api,
            class,
        )?;
    }

    Ok(())
}

pub fn generate_imports(output: &mut impl Write) -> GeneratorResult {
    writeln!(output, "use std::os::raw::c_char;")?;
    writeln!(output, "use std::ptr;")?;
    writeln!(output, "use std::mem;")?;

    Ok(())
}

pub fn generate_class(
    output_types_impls: &mut impl Write,
    output_trait_impls: &mut impl Write,
    output_method_table: &mut impl Write,
    class_name: &str,
) -> GeneratorResult {
    let api = Api::new();

    let class = api.find_class(class_name);

    if let Some(class) = class {
        generate_class_bindings(
            output_types_impls,
            output_trait_impls,
            output_method_table,
            "CORE_METHOD_TABLE",
            "generated::",
            &api,
            class,
        )?;
    }

    Ok(())
}

pub fn generate_method_table(
    output: &mut impl Write,
    table_name: &str,
    api: &Api,
    classes: &[&GodotClass],
) -> GeneratorResult {
    use heck::ShoutySnakeCase;
    let camel_table_name = table_name.to_shouty_snake_case();

    writeln!(
        output,
        r#"
#[doc(hidden)]
pub static mut {table_name}: Option<{camel_table_name}> = None;

#[doc(hidden)]
#[allow(non_camel_case_types)]
pub struct {camel_table_name} {{"#,
        table_name = table_name,
        camel_table_name = camel_table_name,
    )?;

    for class in classes {
        writeln!(
            output,
            r#"
    pub {class_name}__class_constructor: sys::godot_class_constructor,"#,
            class_name = class.name
        )?;
        for method in &class.methods {
            let MethodName {
                rust_name: method_name,
                ..
            } = method.get_name();
            if method_name == "free" {
                continue;
            }
            writeln!(
                output,
                "    pub {}__{}: *mut sys::godot_method_bind,",
                class.name, method_name
            )?;
        }
    }

    writeln!(
        output,
        r#"
}}

#[doc(hidden)]
impl {camel_table_name} {{
    pub fn new() -> Self {{
        Self {{"#,
        camel_table_name = camel_table_name
    )?;

    for class in classes {
        writeln!(
            output,
            r#"
            {class_name}__class_constructor: None,"#,
            class_name = class.name
        )?;
        for method in &class.methods {
            let MethodName {
                rust_name: method_name,
                ..
            } = method.get_name();
            if method_name == "free" {
                continue;
            }
            writeln!(
                output,
                "            {}__{}: 0 as *mut sys::godot_method_bind,",
                class.name, method_name
            )?;
        }
    }

    writeln!(
        output,
        r#"
        }}
    }}
}}

pub fn bind_method_table(gd_api: &GodotApi) {{
    static INIT: Once = Once::new();
    INIT.call_once(|| unsafe {{
        let get_constructor = gd_api.godot_get_class_constructor;
        let get_method = gd_api.godot_method_bind_get_method;
        let mut table = {camel_table_name}::new();"#,
        camel_table_name = camel_table_name
    )?;

    for class in classes {
        let has_underscore = api.api_underscore.contains(&class.name);
        let class_lookup_name: String = if has_underscore {
            format!("_{class}", class = class.name)
        } else {
            class.name.clone()
        };
        writeln!(
            output,
            r#"
        // Bindings for {class_name}
        let class_name = b"{class_lookup_name}\0".as_ptr() as *const c_char;
        table.{class_name}__class_constructor = (get_constructor)(class_name);"#,
            class_name = class.name,
            class_lookup_name = class_lookup_name
        )?;

        for method in &class.methods {
            let MethodName {
                rust_name: method_name,
                original_name,
            } = method.get_name();
            if method_name == "free" {
                continue;
            }

            writeln!(
                output,
                r#"        table.{class_name}__{method_name} = (get_method)(class_name, "{original_name}\0".as_ptr() as *const c_char );"#,
                class_name = class.name,
                method_name = method_name,
                original_name = original_name,
            )?;
        }
    }

    writeln!(
        output,
        r#"
        {table_name}.replace(table);
    }});
}}"#,
        table_name = table_name
    )?;

    Ok(())
}
fn generate_class_bindings(
    output_types_impls: &mut impl Write,
    output_trait_impls: &mut impl Write,
    output_method_table: &mut impl Write,
    table_name: &str,
    namespace: &str,
    api: &Api,
    class: &GodotClass,
) -> GeneratorResult {
    // types and methods
    {
        generate_class_documentation(output_types_impls, &api, class)?;

        generate_class_struct(output_types_impls, class)?;

        for e in &class.enums {
            generate_enum(output_types_impls, class, e)?;
        }

        generate_class_constants(output_types_impls, class)?;

        writeln!(output_types_impls, "impl {} {{", class.name)?;

        if class.singleton {
            generate_singleton_getter(output_types_impls, class)?;
        }

        if class.name == "GDNativeLibrary" {
            generate_gdnative_library_singleton_getter(output_types_impls, class)?;
        }

        if class.instanciable {
            if class.is_refcounted() {
                generate_reference_ctor(output_types_impls, table_name, namespace, class)?;
            } else {
                generate_non_reference_ctor(output_types_impls, table_name, namespace, class)?;
            }
        }

        if class.is_refcounted() {
            generate_reference_copy(output_types_impls, class)?;
        }

        let mut method_set = HashSet::default();

        generate_methods(
            output_types_impls,
            &api,
            &mut method_set,
            table_name,
            namespace,
            &class.name,
            class.is_pointer_safe(),
            true,
        )?;

        generate_upcast(
            output_types_impls,
            &api,
            &class.base_class,
            class.is_pointer_safe(),
        )?;

        generate_dynamic_cast(output_types_impls, class)?;

        writeln!(output_types_impls, "}}")?;
    }

    // traits
    {
        generate_godot_object_impl(output_trait_impls, class)?;

        generate_free_impl(output_trait_impls, &api, class)?;

        if !class.base_class.is_empty() {
            generate_deref_impl(output_trait_impls, class)?;
        }

        if class.is_refcounted() {
            generate_reference_clone(output_trait_impls, class)?;
            generate_drop(output_trait_impls, class)?;
        }

        if class.instanciable {
            generate_instanciable_impl(output_trait_impls, class)?;
        }
    }

    Ok(())
}

fn rust_safe_name(name: &str) -> &str {
    match name {
        "use" => "_use",
        "type" => "_type",
        "loop" => "_loop",
        "in" => "_in",
        "override" => "_override",
        "where" => "_where",
        name => name,
    }
}
