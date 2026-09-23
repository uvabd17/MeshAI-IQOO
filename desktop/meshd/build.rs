fn main() {
    // The admin panel is embedded with include_dir!; make cargo notice edits to it.
    println!("cargo:rerun-if-changed=../admin/src");
    println!("cargo:rerun-if-changed=../../admin/src");
}
