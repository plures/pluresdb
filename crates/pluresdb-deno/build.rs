fn main() {
    // deno_bindgen generates bindings via procedural macros
    // No build script actions needed for 0.9.0-alpha
    println!("cargo:rerun-if-changed=src/lib.rs");
}

