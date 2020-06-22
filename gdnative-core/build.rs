use gdnative_bindings_generator::*;

use std::env;
use std::fs::File;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let generated_rs = out_path.join("generated.rs");

    {
        let mut output = File::create(&generated_rs).unwrap();

        let mut api = Api::new();
        let include_classes = strongly_connected_components(&mut api, "Object", None);
        api.classes.iter_mut().for_each(|(class_name, class)| {
            class.is_generated = include_classes.contains(class_name);
        });

        let module_path = PathBuf::from(env::var("OUT_DIR").unwrap());
        let code = generate_bindings(&mut api, &module_path);
        write!(&mut output, "{}", code).unwrap();
    }

    for file in &[generated_rs] {
        let output = Command::new("rustup")
            .arg("run")
            .arg("stable")
            .arg("rustfmt")
            .arg("--edition")
            .arg("2018")
            .arg(file)
            .output()
            .unwrap();
        eprintln!("Formatting output: {:?}", output);
    }
}
