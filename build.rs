fn main() {
    if cfg!(target_os = "windows") {
        let _ = embed_resource::compile("brandkit/windows/element.rc", embed_resource::NONE);
    }
}
