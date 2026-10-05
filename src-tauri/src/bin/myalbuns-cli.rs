//! Console entry for other programs; the windowed executable has no usable
//! standard streams and treats a project path as a request to open it.
fn main() {
    std::process::exit(myalbuns_desktop_lib::run_cli());
}
