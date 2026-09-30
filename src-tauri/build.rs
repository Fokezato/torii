use std::path::Path;

fn main() {
    tauri_build::build();
    copy_vlc_runtime();
}

fn copy_vlc_runtime() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let src = Path::new(&manifest_dir).join("vendor").join("vlc");
    if !src.exists() {
        return;
    }

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir)
        .ancestors()
        .nth(3)
        .expect("OUT_DIR mais raso que o esperado")
        .to_path_buf();

    copy_dir_if_changed(&src, &dest);
    println!("cargo:rerun-if-changed={}", src.display());
}

fn copy_dir_if_changed(src: &Path, dest: &Path) {
    for entry in walk(src) {
        let rel = entry.strip_prefix(src).unwrap();
        let dest_path = dest.join(rel);
        if entry.is_dir() {
            let _ = std::fs::create_dir_all(&dest_path);
            continue;
        }
        let needs_copy = match std::fs::metadata(&dest_path) {
            Ok(dest_meta) => match std::fs::metadata(&entry) {
                Ok(src_meta) => src_meta.len() != dest_meta.len(),
                Err(_) => true,
            },
            Err(_) => true,
        };
        if needs_copy {
            if let Some(parent) = dest_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::copy(&entry, &dest_path);
        }
    }
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.push(path.clone());
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out
}
