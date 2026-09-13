use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn workspace_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn resolve_path(path_str: &str) -> PathBuf {
    let path = Path::new(path_str);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root().join(path)
    }
}

pub fn skip_dir_name(name: &str) -> bool {
    matches!(
        name,
        "target"
            | "node_modules"
            | "dist"
            | "build"
            | "__pycache__"
            | ".git"
            | ".svn"
            | ".hg"
            | "vendor"
    ) || name.starts_with('.')
}

pub fn looks_binary(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).unwrap_or(""),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "ico"
            | "pdf"
            | "zip"
            | "gz"
            | "tar"
            | "bz2"
            | "7z"
            | "exe"
            | "dll"
            | "so"
            | "dylib"
            | "wasm"
            | "bin"
            | "o"
            | "a"
            | "class"
            | "woff"
            | "woff2"
            | "ttf"
            | "eot"
            | "mp3"
            | "mp4"
            | "webm"
    )
}

pub fn read_text_file(path: &Path) -> Result<String> {
    let bytes =
        std::fs::read(path).with_context(|| format!("Failed to read '{}'", path.display()))?;
    if bytes.contains(&0) {
        anyhow::bail!("Refusing to read binary file '{}'", path.display());
    }
    String::from_utf8(bytes)
        .map_err(|_| anyhow::anyhow!("File '{}' is not valid UTF-8", path.display()))
}

pub fn glob_match(pattern: &str, path: &str) -> bool {
    glob_match_inner(pattern.as_bytes(), path.as_bytes())
}

fn glob_match_inner(pat: &[u8], text: &[u8]) -> bool {
    let mut p = 0;
    let mut t = 0;
    let mut star_p = None;
    let mut star_t = 0;

    while t < text.len() {
        if p < pat.len() && (pat[p] == b'?' || pat[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p + 1 < pat.len() && pat[p] == b'*' && pat[p + 1] == b'*' {
            // ** matches across path segments
            let rest_p = if p + 2 < pat.len() && pat[p + 2] == b'/' {
                p + 3
            } else {
                p + 2
            };
            if glob_match_inner(&pat[rest_p..], &text[t..]) {
                return true;
            }
            if t < text.len() {
                t += 1;
            } else {
                return rest_p >= pat.len();
            }
        } else if p < pat.len() && pat[p] == b'*' {
            star_p = Some(p);
            star_t = t;
            p += 1;
        } else if let Some(sp) = star_p {
            // * does not cross '/'
            if text[star_t] == b'/' {
                return false;
            }
            star_t += 1;
            t = star_t;
            p = sp + 1;
        } else {
            return false;
        }
    }

    while p < pat.len() {
        if p + 1 < pat.len() && pat[p] == b'*' && pat[p + 1] == b'*' {
            p += 2;
            if p < pat.len() && pat[p] == b'/' {
                p += 1;
            }
        } else if pat[p] == b'*' {
            p += 1;
        } else {
            break;
        }
    }
    p >= pat.len()
}

#[cfg(test)]
mod tests {
    use super::glob_match;

    #[test]
    fn glob_star() {
        assert!(glob_match("*.rs", "main.rs"));
        assert!(!glob_match("*.rs", "src/main.rs"));
        assert!(glob_match("**/*.rs", "src/main.rs"));
        assert!(glob_match("**/mod.rs", "cli/src/ui/mod.rs"));
        assert!(!glob_match("*.rs", "main.toml"));
        assert!(glob_match("cli/src/*.rs", "cli/src/main.rs"));
        assert!(glob_match("**/*", "a/b/c"));
    }
}
