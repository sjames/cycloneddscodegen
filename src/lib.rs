/*
    Copyright 2020 Sojan James

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

        http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.
*/

use std::env;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::io::LineWriter;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use syn::{ForeignItem, Item, Type};

#[cfg(feature = "rust_codegen")]
use cyclonedds_idlc::{generate_with_loader, Configuration, IdlLoader};
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

pub fn get_idlc_jar() -> Option<PathBuf> {
    if let Ok(idlc_path) = env::var("CYCLONEDDS_IDLC_JAR") {
        Some(PathBuf::from(idlc_path))    
    } else {
        let cdds_installed_jar = Path::new("/usr/local/lib/cmake/CycloneDDS/idlc/idlc-jar-with-dependencies.jar");
        if cdds_installed_jar.exists() {
            println!("Using CycloneDDS installed IDLC Jar. Set CYCLONEDDS_IDLC_JAR environment variable to override");
            Some(cdds_installed_jar.into())
        } else {
            None
        }
    }
}

pub fn generate_and_compile_datatypes(path_to_idl: Vec<&str>) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    if let Some(idlc_jar) = get_idlc_jar() {
        println!("IDLC JAR {:?}", &idlc_jar);
        let mut paths = Vec::new();

        for filename in &path_to_idl {
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
        #[cfg(not(feature = "rust_codegen"))]
        generate_bindings(generated_files);
        #[cfg(feature = "rust_codegen")]
        generate_bindings(&path_to_idl);
    } else {

        panic!(
            "Did not find environment variable CYCLONEDDS_IDLC_JAR. Please set this and try again"
        );
    }
}

#[cfg(feature = "rust_codegen")]
pub fn generate_bindings(path_to_idl: &Vec<&str>) {
    let config = Configuration::default();

    for filename in path_to_idl {
        let path_to_idl = PathBuf::from(filename);
        let search_path = vec![String::from(
            path_to_idl.parent().unwrap().to_str().unwrap(),
        )];
        let mut loader = Loader::new(search_path);
        let data = load_from(
            path_to_idl.parent().unwrap(),
            path_to_idl.file_name().unwrap().to_str().unwrap(),
        )
        .unwrap();

        if let Ok(path) = env::var("OUT_DIR") {
            let out_path = PathBuf::from(path);
            println!("Out path:{:?}",&out_path.join("bindings.rs"));
            let mut of = OpenOptions::new()
                .append(true)
                .create(true)
                .open(&out_path.join("bindings.rs"))
                .expect("Unable to open bindings.rs for write");
            //let mut of = File::create(std::path::Path::new(&out_path.join("bindings.rs"))).expect("Unable to open bindings.rs for writing");
            let res = generate_with_loader(&mut of, &mut loader, &config, &data);
        } else {
            let res = generate_with_loader(&mut std::io::stdout(), &mut loader, &config, &data);
        };
    }
}

#[cfg(not(feature = "rust_codegen"))]
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

#[cfg(feature = "rust_codegen")]
#[derive(Debug, Clone, Default)]
struct Loader {
    search_path: Vec<String>,
}
#[cfg(feature = "rust_codegen")]
fn load_from(prefix: &std::path::Path, filename: &str) -> Result<String, Error> {
    let fullname = prefix.join(filename);

    let mut file = File::open(fullname)?;
    let mut data = String::new();

    file.read_to_string(&mut data)?;

    return Ok(data);
}
#[cfg(feature = "rust_codegen")]
impl Loader {
    pub fn new(search_path: Vec<String>) -> Loader {
        Loader {
            search_path: search_path,
        }
    }
}
#[cfg(feature = "rust_codegen")]
impl IdlLoader for Loader {
    fn load(&self, filename: &str) -> Result<String, Error> {
        for prefix in &self.search_path {
            let prefix_path = std::path::Path::new(&prefix);
            match load_from(&prefix_path, filename) {
                Ok(data) => return Ok(data),
                _ => continue,
            }
        }
        Err(Error::from(ErrorKind::NotFound))
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
