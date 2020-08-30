# cycloneddscodegen

Use the cyclonedds idlc code generator (Java) to generate C code from the IDL.  The generated code is compiled into the library.
Use this library to create a library crate out of your IDL and use the power of Cargo to manage your interfaces.

# Experimental feature [rust_codegen]

Generating rust bindings from the generated C code works, but I miss the module concept. Turning on this
experimental feature parses the IDL and generates the rust code for the topic definitions. We still
rely on the C code for the topic descriptor structures.






