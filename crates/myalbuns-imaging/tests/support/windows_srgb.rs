// The Windows test host supplies this profile; the product never reads it from
// the operating system. Its exact identity is fixed independently of its name.
pub fn standard_profile() -> Vec<u8> {
    use sha2::{Digest, Sha256};

    let system_root = std::env::var_os("SystemRoot").expect("Windows defines SystemRoot");
    let path = std::path::PathBuf::from(system_root)
        .join("System32/spool/drivers/color/sRGB Color Space Profile.icm");
    let profile = std::fs::read(path).expect("Windows supplies its standard sRGB profile");
    assert_eq!(profile.len(), 3_144);
    assert_eq!(
        format!("{:x}", Sha256::digest(&profile)),
        "2b3aa1645779a9e634744faf9b01e9102b0c9b88fd6deced7934df86b949af7e",
        "a changed system profile requires an explicit fixture review"
    );
    profile
}
