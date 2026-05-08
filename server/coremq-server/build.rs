fn main() {
    // Ensure client/dist exists so rust-embed compiles even before `yarn build` is run
    std::fs::create_dir_all("../../client/dist").ok();
    println!("cargo:rerun-if-changed=../../client/dist");
}
