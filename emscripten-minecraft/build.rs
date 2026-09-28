fn main() {
    // Bin-only: `-Clink-arg` in .cargo/config.toml would reach side-module
    // links too, which have no `_main`. `stop` is called from the SIGINT
    // handler main installs.
    println!("cargo::rustc-link-arg-bins=-sEXPORTED_FUNCTIONS=_main,_stop");
}
