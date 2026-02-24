/// Tell Cargo to re-run this build script (and therefore re-embed the dashboard)
/// whenever any file inside the Next.js static export changes.
fn main() {
    println!("cargo:rerun-if-changed=../dashboard/out");
}
