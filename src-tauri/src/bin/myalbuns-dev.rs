#[cfg(windows)]
#[path = "../dev_supervisor.rs"]
mod dev_supervisor;

#[cfg(windows)]
fn main() {
    match dev_supervisor::run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("não foi possível supervisionar o ambiente de desenvolvimento: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("o supervisor de desenvolvimento do MyAlbuns exige Windows");
    std::process::exit(1);
}
