use std::env;
use std::fs::{ File};
use std::io::prelude::*;
use std::io::LineWriter;
use std::path::PathBuf;
use syn::{ForeignItem, Item, Type, };

use std::process::Command;

/*
fn main() {
    cc::Build::new()
        .file("src-gen/HelloWorldData.c")
        .compile("hello");

    println!("cargo:rerun-if-changed=src-gen/HelloWorldData.c");

    generate_bindings("src-gen/HelloWorldData.c");
}
*/

pub fn generate_and_compile_datatypes(path_to_idl: Vec<&str>) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    if let Ok(idlc_path) = env::var("CYCLONEDDS_IDLC_JAR") {
        let idlc_jar = PathBuf::from(idlc_path);
        println!("IDLC JAR {:?}", &idlc_jar);
        let mut paths = Vec::new();

        for filename in path_to_idl {
            Command::new("java")
                .arg("-classpath")
                .arg(&idlc_jar)
                .arg("org.eclipse.cyclonedds.compilers.Idlc")
                .arg("-d")
                .arg(&out_dir)
                .arg(filename)
                .output()
                .expect("Error executing DDS IDLC Code generator");

            paths.push(std::path::PathBuf::from(filename));
        }

        let mut lib_name = String::from("lib");
        let mut builder = cc::Build::new();
        let mut generated_files = Vec::new();
        for f in paths {
            let generated_c_filename =
                String::from(format!("{}.c", f.file_stem().unwrap().to_str().unwrap()));
            let generated_c = out_dir.join(generated_c_filename);
            let generated_c_str = String::from(generated_c.to_str().unwrap());
            generated_files.push(generated_c_str);
            lib_name.push_str(f.file_stem().unwrap().to_str().unwrap());
            builder.file(&generated_c);
        }
        builder.compile(&lib_name);
        generate_bindings(generated_files);
    } else {
        panic!(
            "Did not find environment variable CYCLONEDDS_IDLC_JAR. Please set this and try again"
        );
    }
}

pub fn generate_bindings(header: Vec<String>) {
    let mut bindings = bindgen::Builder::default();
    for h in header {
        bindings = bindings.header(h);
    }

    let bindings = bindings
        .whitelist_recursively(false)
        .ignore_functions()
        .blacklist_item("dds_topic_descriptor") // to use these types from the cyclonedds_rs crate.
        .blacklist_item("*__keys")
        .blacklist_item("*__ops")
        .blacklist_item("dds_entity_kind")
        .blacklist_item("dds_entity_kind_t")
        .blacklist_type("dds_stream_typecode_subtype")
        .blacklist_type("dds_stream_typecode_primary")
        .blacklist_type("dds_stream_opcode")
        .blacklist_type("dds_stream_typecode")
        .blacklist_item("dds_topic_descriptor_t")
        .blacklist_type("dds_free_op_t")
        .blacklist_type("dds_allocator")
        .blacklist_type("dds_allocator_t")
        .blacklist_type("max_align_t")
        .blacklist_type("dds_alloc_fn_t")
        .blacklist_type("dds_realloc_fn_t")
        .blacklist_type("dds_free_fn_t")
        .blacklist_type("dds_instance_handle_t")
        .blacklist_type("dds_domainid_t")
        .raw_line("use cyclonedds_sys::dds_topic_descriptor_t;");

    let gen = bindings.generate().expect("Unable to generate bindings");

    if let Ok(path) = env::var("OUT_DIR") {
        let out_path = PathBuf::from(path);
        gen.write_to_file(out_path.join("bindings.rs"))
            .expect("Couldn't write bindings");

        write_trait_impls(gen.to_string());
    } else {
        println!("OUT_DIR not set, not generating bindings");
    }
}

fn find_ids_with_type_as_descriptor(bindings: String) -> Vec<String> {
    let mut ids = Vec::<String>::new();

    let syntax = syn::parse_file(&bindings).expect("Unable to parse generated binding");
    //println!("{:#?}", syntax.to_string());

    for item in syntax.items {
        // println!("{:}", &item);

        match item {
            Item::Static(_item) => {
                println!("Found static item");
            }
            Item::ForeignMod(item) => {
                for it in item.items {
                    match it {
                        ForeignItem::Static(item) => match *item.ty {
                            Type::Path(path) => {
                                if let Some(ident) = path.path.get_ident() {
                                    println!(" {} : Ident:{}", item.ident, ident);
                                    ids.push(item.ident.to_string());
                                }
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                }
            }
            _ => {
                //println!("Skipping");
            }
        }
    }
    ids
}

/// Generate the necessary code for the specified.  I attempted to
/// use a macro for this but hit a wall as macros cannot be used to
/// concat names of identifiers. Or atleast I could not figure out how to do so.
fn generate_code_for_type(typename: &str) -> String {
    let template = r###"
    impl DDSGenType for {TYPENAME}{
        unsafe fn get_descriptor() -> &'static dds_topic_descriptor_t {
            &{TYPENAME}_desc
        }
    }
"###;

    template.replace("{TYPENAME}", typename)
}

/// Write the DdsAllocator implementations for all the generated structs
/// We identify the topic descritor type "dds_topic_descriptor_t". Once we find this
/// we can get the name of the descriptor and then call the macro.
fn write_trait_impls(bindings: String) {
    if let Ok(path) = env::var("OUT_DIR") {
        let out_path = PathBuf::from(path).join("DdsAllocator_impl.rs");
        let file = File::create(&out_path)
            .expect(format!("Unable to open {} for writing", &out_path.to_str().unwrap()).as_str());
        let mut file = LineWriter::new(file);
        let ids = find_ids_with_type_as_descriptor(bindings);
        println!("{:?}", &ids);
        for id in ids {
            let mut s = String::from(id);
            s.truncate(&s.len() - 5);
            println!("{}", s);
            //impl_allocator_for_dds_type!(HelloWorldData_Msg);
            //file.write_all(format!("impl_allocator_for_dds_type!({});\n", s).as_bytes());
            file.write_all(generate_code_for_type(&s).as_bytes())
                .expect("Unable to write generated bindings");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gen() {
        let mut ids = vec!["test/HelloWorldData.idl"];

        generate_and_compile_datatypes(ids);
    }
}
