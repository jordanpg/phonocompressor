//! Running `phono` and finding the data file and fixtures.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// The data file: `PHONO_DATA` if set, else the workspace's `data/generated/phono.bin`. A
/// missing file fails the test that asked for it, with how to build it.
pub fn data() -> &'static Path {
    static DATA: OnceLock<PathBuf> = OnceLock::new();
    DATA.get_or_init(|| {
        let path = std::env::var_os("PHONO_DATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| workspace().join("data/generated/phono.bin"));
        assert!(
            path.is_file(),
            "{}: not found. Run `mise run //crates/cli:e2e`, which builds it first, or set \
             PHONO_DATA",
            path.display()
        );
        path
    })
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A file in `tests/fixtures/`.
pub fn fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .display()
        .to_string()
}

/// An empty directory for one test's output files, under Cargo's scratch directory for tests.
pub fn out_dir(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("e2e")
        .join(test);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The working directory every run uses: empty, so the CLI's relative defaults
/// (`data/generated/phono.bin`, `tools/library-keys.tsv`) never find the repo's files.
fn empty_cwd() -> &'static Path {
    static CWD: OnceLock<PathBuf> = OnceLock::new();
    CWD.get_or_init(|| out_dir("cwd"))
}

/// What one run of `phono` did.
#[derive(Debug)]
pub struct Run {
    pub args: Vec<String>,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    /// Asserts the run succeeded.
    #[track_caller]
    pub fn ok(self) -> Self {
        assert_eq!(
            self.code,
            Some(0),
            "phono {:?} failed:\n{}",
            self.args,
            self.stderr
        );
        self
    }

    /// Asserts the run exited with `code` and that stderr contains every one of `needles`.
    #[track_caller]
    pub fn fails(self, code: i32, needles: &[&str]) -> Self {
        assert_eq!(
            self.code,
            Some(code),
            "phono {:?}: expected exit {code}\nstdout:\n{}\nstderr:\n{}",
            self.args,
            self.stdout,
            self.stderr
        );
        for n in needles {
            assert!(
                self.stderr.contains(n),
                "phono {:?}: stderr lacks {n:?}:\n{}",
                self.args,
                self.stderr
            );
        }
        self
    }

    pub fn lines(&self) -> Vec<&str> {
        self.stdout.lines().collect()
    }

    /// The first stdout line starting with `prefix`, if any.
    pub fn line(&self, prefix: &str) -> Option<&str> {
        self.stdout.lines().find(|l| l.starts_with(prefix))
    }

    /// Asserts stdout has a line equal to `line`, or starting with it if `line` ends in `…`.
    #[track_caller]
    pub fn has_line(&self, line: &str) -> &Self {
        let found = match line.strip_suffix('…') {
            Some(prefix) => self.line(prefix).is_some(),
            None => self.stdout.lines().any(|l| l == line),
        };
        assert!(
            found,
            "phono {:?}: no line {line:?} in:\n{}",
            self.args, self.stdout
        );
        self
    }
}

/// Runs `phono` with exactly these arguments.
pub fn phono_raw<S: AsRef<str>>(args: &[S]) -> Run {
    let args: Vec<String> = args.iter().map(|a| a.as_ref().to_owned()).collect();
    let out = Command::new(env!("CARGO_BIN_EXE_phono"))
        .args(&args)
        .current_dir(empty_cwd())
        .output()
        .expect("phono runs");
    Run {
        args,
        code: out.status.code(),
        stdout: String::from_utf8(out.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(out.stderr).expect("stderr is UTF-8"),
    }
}

/// Runs `phono --data <data file>` with these arguments.
pub fn phono<S: AsRef<str>>(args: &[S]) -> Run {
    let mut all = vec!["--data".to_owned(), data().display().to_string()];
    all.extend(args.iter().map(|a| a.as_ref().to_owned()));
    phono_raw(&all)
}

/// One `phono compress --json` result. Unknown fields fail: the shape is what the wasm parity
/// check and the web front end rely on.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Guess {
    pub text: String,
    pub latin: String,
    pub phones: String,
    pub score: f32,
    pub close_enough: bool,
    pub codepoints: usize,
}

/// `phono compress <phrase> --json <extra>`, parsed.
pub fn compress(phrase: &str, extra: &[&str]) -> Vec<Guess> {
    let mut args = vec!["compress", phrase, "--json"];
    args.extend_from_slice(extra);
    let run = phono(&args).ok();
    serde_json::from_str(&run.stdout)
        .unwrap_or_else(|e| panic!("phono {:?}: bad JSON ({e}):\n{}", run.args, run.stdout))
}
