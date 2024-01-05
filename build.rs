fn main() {
    println!("cargo:rustc-link-lib=sixel");
    println!("cargo:rustc-link-search=native=/opt/homebrew/lib");
}
