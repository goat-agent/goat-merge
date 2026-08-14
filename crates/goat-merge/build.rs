use std::path::Path;
use std::process::Command;

fn main() {
    let web = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web");
    for watched in ["src", "index.html", "package.json", "vite.config.ts"] {
        println!("cargo::rerun-if-changed={}", web.join(watched).display());
    }
    println!("cargo::rerun-if-env-changed=GOAT_MERGE_SKIP_WEB_BUILD");

    if std::env::var("GOAT_MERGE_SKIP_WEB_BUILD").is_ok() {
        make_sure_something_is_there(&web);
        return;
    }
    let built = web.join("dist/index.html");
    let releasing = std::env::var("PROFILE").is_ok_and(|profile| profile == "release");
    if built.exists() && !releasing {
        return;
    }

    let install = Command::new("pnpm")
        .arg("install")
        .current_dir(&web)
        .status();
    let build = Command::new("pnpm")
        .args(["run", "build"])
        .current_dir(&web)
        .status();

    match (install, build) {
        (Ok(install), Ok(build)) if install.success() && build.success() => {}
        _ => {
            make_sure_something_is_there(&web);
            println!(
                "cargo::warning=the console could not be built with pnpm, so this binary serves a \
                 placeholder. Run `pnpm --dir web build`, or set GOAT_MERGE_SKIP_WEB_BUILD=1 to \
                 stop trying."
            );
        }
    }
}

fn make_sure_something_is_there(web: &Path) {
    let dist = web.join("dist");
    if dist.join("index.html").exists() {
        return;
    }
    let _ = std::fs::create_dir_all(&dist);
    let _ = std::fs::write(
        dist.join("index.html"),
        "<!doctype html><meta charset=\"utf-8\"><title>Merge Queue</title>\
         <p>The console was not built into this binary. Run <code>pnpm --dir web build</code>.</p>",
    );
}
