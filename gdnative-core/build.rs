use gdnative_bindings_generator::*;
use std::env;
use std::fs::File;
use std::path::PathBuf;

fn main() {
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    let mut types_output = File::create(out_path.join("core_types.rs")).unwrap();
    let mut traits_output = File::create(out_path.join("core_traits.rs")).unwrap();
    let mut methods_output = File::create(out_path.join("core_methods.rs")).unwrap();

    let api = Api::new();

    let classes = strongly_connected_components(&api, "Object", None);

    generate_method_table(
        &mut methods_output,
        "CORE_METHOD_TABLE",
        &api,
        api.classes
            .iter()
            .filter(|class| classes.contains(&class.name))
            .collect::<Vec<&GodotClass>>()
            .as_slice(),
    )
    .unwrap();

    for class in classes {
        generate_class(
            &mut types_output,
            &mut traits_output,
            &mut methods_output,
            &class,
        )
        .unwrap();
    }
}
