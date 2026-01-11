fn main() {
    deno_bindgen::Builder::default()
        .bindgen()
        .build()
        .unwrap();
}

