use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn latest(directory: &Path, predicate: impl Fn(&Path) -> bool) -> PathBuf {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .expect("Android SDK directory missing")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| predicate(path))
        .collect();
    entries.sort_by_key(|path| {
        path.file_name()
            .unwrap()
            .to_string_lossy()
            .split(|c: char| !c.is_ascii_digit())
            .filter_map(|part| part.parse::<u32>().ok())
            .collect::<Vec<_>>()
    });
    entries
        .pop()
        .expect("Install an Android SDK platform >=30 and build-tools with d8")
}
fn run(command: &mut Command) {
    assert!(
        command
            .status()
            .expect("Android Java build tool unavailable")
            .success(),
        "Android Java bridge compilation failed"
    );
}
fn main() {
    println!("cargo:rerun-if-changed=android/InsetsObserver.java");
    for name in ["ANDROID_HOME", "ANDROID_SDK_ROOT", "JAVA_HOME"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        return;
    }
    let sdk = PathBuf::from(
        env::var_os("ANDROID_HOME")
            .or_else(|| env::var_os("ANDROID_SDK_ROOT"))
            .expect("ANDROID_HOME or ANDROID_SDK_ROOT is required"),
    );
    let platform = latest(&sdk.join("platforms"), |path| {
        path.join("android.jar").is_file()
    });
    let tools = latest(&sdk.join("build-tools"), |path| {
        path.join(if cfg!(windows) { "d8.bat" } else { "d8" })
            .is_file()
    });
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let classes = out.join("java-classes");
    fs::create_dir_all(&classes).unwrap();
    let javac = env::var_os("JAVA_HOME")
        .map(|home| PathBuf::from(home).join("bin/javac"))
        .unwrap_or_else(|| PathBuf::from("javac"));
    run(Command::new(javac)
        .args(["--release", "8", "-classpath"])
        .arg(platform.join("android.jar"))
        .arg("-d")
        .arg(&classes)
        .arg("android/InsetsObserver.java"));
    let mut d8 = Command::new(tools.join(if cfg!(windows) { "d8.bat" } else { "d8" }));
    d8.args(["--min-api", "26", "--lib"])
        .arg(platform.join("android.jar"))
        .arg("--output")
        .arg(&out);
    let mut class_files: Vec<_> = fs::read_dir(classes.join("dev/blitz/shell"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|class| class.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "class"))
        .collect();
    class_files.sort();
    d8.args(class_files);
    run(&mut d8);
}
