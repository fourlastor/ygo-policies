//! Compiles OCGCore (`vendor/ocgcore`) and its Lua (`vendor/ocgcore/lua/src`)
//! the way the core's own premake script does: Lua as C++, without the
//! libraries OCGCore strips, with `luaconf-customize.h` force-included.

use std::path::PathBuf;

const LUA_EXCLUDED: &[&str] = &[
    "lbitlib.c",
    "lcorolib.c",
    "ldblib.c",
    "linit.c",
    "loadlib.c",
    "loslib.c",
    "ltests.c",
    "lua.c",
    "luac.c",
    "lutf8lib.c",
    "onelua.c",
];

fn main() {
    let root =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../vendor/ocgcore");
    let lua = root.join("lua/src");
    assert!(
        lua.join("lua.h").exists(),
        "vendor/ocgcore/lua/src is missing: run `git submodule update --init --recursive`"
    );

    let mut lua_files: Vec<PathBuf> = std::fs::read_dir(&lua)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "c"))
        .filter(|path| !LUA_EXCLUDED.contains(&path.file_name().unwrap().to_str().unwrap()))
        .collect();
    lua_files.sort();
    cc::Build::new()
        .cpp(true)
        .files(&lua_files)
        .include(root.join("lua"))
        .flag("-xc++")
        .flag("-include")
        .flag(root.join("lua/luaconf-customize.h").to_str().unwrap())
        .flag_if_supported("-std=c++17")
        .warnings(false)
        .opt_level(2)
        .compile("ocgcore_lua");

    let core = [
        "card.cpp",
        "duel.cpp",
        "effect.cpp",
        "field.cpp",
        "interpreter.cpp",
        "libcard.cpp",
        "libdebug.cpp",
        "libduel.cpp",
        "libeffect.cpp",
        "libgroup.cpp",
        "ocgapi.cpp",
        "operations.cpp",
        "playerop.cpp",
        "processor.cpp",
        "processor_visit.cpp",
        "scriptlib.cpp",
    ];
    cc::Build::new()
        .cpp(true)
        .files(core.iter().map(|file| root.join(file)))
        .include(&root)
        .include(&lua)
        .include(root.join("lua"))
        .flag_if_supported("-std=c++17")
        .flag_if_supported("-Wno-unused-parameter")
        .define("OCGCORE_EXPORT_FUNCTIONS", None)
        .warnings(false)
        .opt_level(2)
        .compile("ocgcore");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", root.display());
}
