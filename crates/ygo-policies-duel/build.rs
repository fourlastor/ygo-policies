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
        "duel_arena.cpp",
        "duel_swap.cpp",
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
    // Add the simulation-only RNG setter to a build-local copy. The pinned
    // vendor checkout stays untouched, and every compiled core translation
    // unit sees the same declaration (no private-access or layout tricks).
    let search_core = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("search-core");
    std::fs::create_dir_all(&search_core).unwrap();
    for entry in std::fs::read_dir(&root).unwrap() {
        let source = entry.unwrap().path();
        if source.is_file() && source.extension().is_some_and(|ext| ext == "h" || ext == "cpp") {
            std::fs::copy(&source, search_core.join(source.file_name().unwrap())).unwrap();
        }
    }
    let header = search_core.join("duel.h");
    let declaration = std::fs::read_to_string(&header).unwrap();
    let anchor = "\tbool restore_snapshot(const snapshot_state& snapshot);";
    assert_eq!(declaration.matches(anchor).count(), 1, "vendor duel API changed: review the search RNG bridge");
    std::fs::write(&header, declaration.replace(anchor, &format!(
        "{anchor}\n\tvoid reseed_for_search(const RNG::Xoshiro256StarStar::StateType& state) {{ random = RNG::Xoshiro256StarStar(state); }}"
    ))).unwrap();
    cc::Build::new()
        .cpp(true)
        .files(core.iter().map(|file| search_core.join(file)))
        .file("src/search_bridge.cpp")
        .include(&search_core)
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
    println!("cargo:rerun-if-changed=src/search_bridge.cpp");
    println!("cargo:rerun-if-changed={}", root.display());
}
