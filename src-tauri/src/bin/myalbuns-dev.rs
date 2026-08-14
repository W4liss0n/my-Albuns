#[cfg(windows)]
#[path = "../dev_job.rs"]
mod dev_job;

#[cfg(windows)]
#[path = "../dev_supervisor.rs"]
mod dev_supervisor;

#[cfg(windows)]
fn main() {
    match dev_supervisor::run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("could not supervise the development environment: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("the MyAlbuns development supervisor requires Windows");
    std::process::exit(1);
}
