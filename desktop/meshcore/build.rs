fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc");
    std::env::set_var("PROTOC", protoc);
    let proto = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proto/mesh.proto");
    println!("cargo:rerun-if-changed={}", proto.display());
    prost_build::Config::new()
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .compile_protos(&[proto.clone()], &[proto.parent().unwrap()])
        .expect("compile mesh.proto");
}
