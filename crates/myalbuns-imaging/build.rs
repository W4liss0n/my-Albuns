fn main() {
    println!("cargo:rerun-if-changed=windows-resource.rc");
    println!("cargo:rerun-if-changed=../../resources/windows/myalbuns.manifest");
    embed_resource::compile("windows-resource.rc", embed_resource::NONE)
        .manifest_required()
        .expect("the Imaging executable requires its Windows long-path manifest");
}
