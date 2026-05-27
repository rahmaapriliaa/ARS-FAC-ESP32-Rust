// build.rs — wajib ada untuk esp-hal agar linker script terhubung
fn main() {
    println!("cargo:rustc-link-arg-bins=-Tlinkall.x");
    println!("cargo:rustc-link-arg-bins=-Trom_functions.x");
}
