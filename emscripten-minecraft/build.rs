use std::env;

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("emscripten") {
        return;
    }
    for arg in [
        "-sBINARYEN_EXTRA_PASSES=--translate-to-exnref",
        "-sENVIRONMENT=node",
        // Real TCP sockets and the host filesystem (which also mirrors
        // process.env through NODE_HOST_ENV).
        "-sNODERAWSOCKETS",
        "-sNODERAWFS",
        "-sALLOW_MEMORY_GROWTH=1",
        "-sSTACK_SIZE=8MB",
        // Keep growth headroom bounded after generation's temporary allocations.
        "-sMEMORY_GROWTH_LINEAR_STEP=2097152",
        "-sEXPORTED_FUNCTIONS=_main,_stop",
        "-sASSERTIONS=0",
    ] {
        println!("cargo::rustc-link-arg-bins={arg}");
    }
}
