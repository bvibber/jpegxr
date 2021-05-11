use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let target = env::var("TARGET").expect("empty TARGET");
    let emscripten = target == "wasm32-unknown-emscripten";

    let src = vec![
        // SRC_SYS
        "jxrlib/image/sys/adapthuff.c",
        "jxrlib/image/sys/image.c",
        "jxrlib/image/sys/strcodec.c",
        "jxrlib/image/sys/strPredQuant.c",
        "jxrlib/image/sys/strTransform.c",
        "jxrlib/image/sys/perfTimerANSI.c",
        // SRC_DEC
        "jxrlib/image/decode/decode.c",
        "jxrlib/image/decode/postprocess.c",
        "jxrlib/image/decode/segdec.c",
        "jxrlib/image/decode/strdec.c",
        "jxrlib/image/decode/strInvTransform.c",
        "jxrlib/image/decode/strPredQuantDec.c",
        "jxrlib/image/decode/JXRTranscode.c",
        // SRC_ENC
        "jxrlib/image/encode/encode.c",
        "jxrlib/image/encode/segenc.c",
        "jxrlib/image/encode/strenc.c",
        "jxrlib/image/encode/strFwdTransform.c",
        "jxrlib/image/encode/strPredQuantEnc.c",
        // glue lib
        "jxrlib/jxrgluelib/JXRGlue.c",
        "jxrlib/jxrgluelib/JXRGlueJxr.c",
        "jxrlib/jxrgluelib/JXRGluePFC.c",
        "jxrlib/jxrgluelib/JXRMeta.c",
    ];
    let mut builder = cc::Build::new();
    if emscripten {
        builder.flag("-s");
        builder.flag("DISABLE_EXCEPTION_CATCHING=1");
    }
    builder
        .files(src.iter())
        .include("jxrlib")
        .include("jxrlib/common/include")
        .include("jxrlib/image/sys")
        .include("jxrlib/jxrgluelib")
        .define("__ANSI__", None)
        .define("DISABLE_PERF_MEASUREMENT", None)
        // quiet the build on mac with clang
        .flag_if_supported("-Wno-constant-conversion")
        .flag_if_supported("-Wno-unused-const-variable")
        .flag_if_supported("-Wno-deprecated-declarations")
        .flag_if_supported("-Wno-comment")
        .flag_if_supported("-Wno-unused-value")
        .flag_if_supported("-Wno-unused-function")
        .flag_if_supported("-Wno-unknown-pragmas")
        .flag_if_supported("-Wno-extra-tokens")
        .flag_if_supported("-Wno-missing-field-initializers")
        .flag_if_supported("-Wno-shift-negative-value")
        .flag_if_supported("-Wno-dangling-else")
        .flag_if_supported("-Wno-sign-compare")
        // quiet the build on linux with gcc
        .flag_if_supported("-Wno-strict-aliasing")
        .flag_if_supported("-Wno-implicit-fallthrough")
        .flag_if_supported("-Wno-old-style-declaration")
        .flag_if_supported("-Wno-endif-labels")
        .flag_if_supported("-Wno-parentheses")
        .flag_if_supported("-Wno-misleading-indentation")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .opt_level(2)
        .compile("jpegxr");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    let mut clang_args = Vec::<String>::new();
    if emscripten {
        // emcc --cflags
        let cflags = Command::new("emcc")
            .arg("--cflags")
            .output()
            .expect("Failed to invoke 'emcc --cflags'")
            .stdout;

        // @fixme this could includes quotes in paths?
        clang_args.extend(std::str::from_utf8(&cflags)
            .expect("UTF-8 failure on emcc --cflags")
            .split(" ")
            .filter(|&str| str != "-fignore-exceptions") // for clang 11 and earlier
            .map(|str| str.to_string()));

        // workaround for https://github.com/rust-lang/rust-bindgen/issues/751
        clang_args.push("-fvisibility=default".to_string());
    }
    clang_args.push("-D__ANSI__".to_string());
    clang_args.push("-DDISABLE_PERF_MEASUREMENT".to_string());
    clang_args.push("-Ijxrlib/jxrgluelib".to_string());
    clang_args.push("-Ijxrlib/common/include".to_string());
    clang_args.push("-Ijxrlib/image/sys".to_string());

    bindgen::Builder::default()
        .header("jxrlib/jxrgluelib/JXRGlue.h")
        .allowlist_function("^(WMP|PK|PixelFormatLookup|GetPixelFormatFromHash|GetImageEncodeIID|GetImageDecodeIID|FreeDescMetadata).*")
        .allowlist_var("^(WMP|PK|LOOKUP|GUID_PK|IID).*")
        .allowlist_type("^(WMP|PK|ERR|BITDEPTH|BD_|BITDEPTH_BITS|COLORFORMAT).*")
        .clang_args(&clang_args)
        .derive_eq(true)
        .size_t_is_usize(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Error building libjpegxr bindings")
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
