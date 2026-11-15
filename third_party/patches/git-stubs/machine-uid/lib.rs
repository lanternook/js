// OHOS build stub. The real machine-uid crate is not built for this target;
// this only satisfies cargo resolution and the rare non-OHOS cfg branch.
pub fn get() -> Result<String, std::io::Error> {
    Ok("ohos-machine-uid-stub".to_owned())
}
