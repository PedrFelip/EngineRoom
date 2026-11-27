fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=crates/engineroom-rune/engineroom.rc");
        println!("cargo:rerun-if-changed=assets/icons/engineroom-rune.ico");
        embed_resource::compile("crates/engineroom-rune/engineroom.rc", embed_resource::NONE)
            .manifest_required()
            .expect("embed EngineRoom icon in the Windows executable");
    }
}
