use std::error::Error;
use std::path::{Path, PathBuf};
use yaml_rust::{Yaml, YamlEmitter, YamlLoader};

/// List of directories containing files to expand. The first tuple element is the source
/// directory, while the second tuple element is the destination directory.
#[rustfmt::skip]
static TO_EXPAND: &[(&str, &str)] = &[
    ("src/ci/github-actions", ".github/workflows"),
];

/// Name of a special key that will be removed from all the maps in expanded configuration files.
/// This key can then be used to contain shared anchors.
static REMOVE_MAP_KEY: &str = "x--expand-yaml-anchors--remove";

/// Message that will be included at the top of all the expanded files. {source} will be replaced
/// with the source filename relative to the base path.
static HEADER_MESSAGE: &str = "\
#############################################################
#   WARNING: automatically generated file, DO NOT CHANGE!   #
#############################################################

# This file was automatically generated by the expand-yaml-anchors tool. The
# source file that generated this one is:
#
#   {source}
#
# Once you make changes to that file you need to run:
#
#   ./x.py run src/tools/expand-yaml-anchors/
#
# The CI build will fail if the tool is not run after changes to this file.

";

enum Mode {
    Check,
    Generate,
}

struct App {
    mode: Mode,
    base: PathBuf,
}

impl App {
    fn from_args() -> Result<Self, Box<dyn Error>> {
        // Parse CLI arguments
        let args = std::env::args().skip(1).collect::<Vec<_>>();
        let (mode, base) = match args.iter().map(|s| s.as_str()).collect::<Vec<_>>().as_slice() {
            &["generate", ref base] => (Mode::Generate, PathBuf::from(base)),
            &["check", ref base] => (Mode::Check, PathBuf::from(base)),
            _ => {
                eprintln!("usage: expand-yaml-anchors <source-dir> <dest-dir>");
                std::process::exit(1);
            }
        };

        Ok(App { mode, base })
    }

    fn run(&self) -> Result<(), Box<dyn Error>> {
        for (source, dest) in TO_EXPAND {
            let source = self.base.join(source);
            let dest = self.base.join(dest);
            for entry in std::fs::read_dir(&source)? {
                let path = entry?.path();
                if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("yml") {
                    continue;
                }

                let dest_path = dest.join(path.file_name().unwrap());
                self.expand(&path, &dest_path).with_context(|| match self.mode {
                    Mode::Generate => format!(
                        "failed to expand {} into {}",
                        self.path(&path),
                        self.path(&dest_path)
                    ),
                    Mode::Check => format!("{} is not up to date", self.path(&dest_path)),
                })?;
            }
        }
        Ok(())
    }

    fn expand(&self, source: &Path, dest: &Path) -> Result<(), Box<dyn Error>> {
        let content = std::fs::read_to_string(source)
            .with_context(|| format!("failed to read {}", self.path(source)))?;

        let mut buf = HEADER_MESSAGE.replace("{source}", &self.path(source).to_string());

        let documents = YamlLoader::load_from_str(&content)
            .with_context(|| format!("failed to parse {}", self.path(source)))?;
        for mut document in documents.into_iter() {
            document = yaml_merge_keys::merge_keys(document)
                .with_context(|| format!("failed to expand {}", self.path(source)))?;
            document = filter_document(document);

            YamlEmitter::new(&mut buf).dump(&document).map_err(|err| WithContext {
                context: "failed to serialize the expanded yaml".into(),
                source: Box::new(err),
            })?;
            buf.push('\n');
        }

        match self.mode {
            Mode::Check => {
                let old = std::fs::read_to_string(dest)
                    .with_context(|| format!("failed to read {}", self.path(dest)))?;
                if old != buf {
                    return Err(Box::new(StrError(format!(
                        "{} and {} are different",
                        self.path(source),
                        self.path(dest),
                    ))));
                }
            }
            Mode::Generate => {
                std::fs::write(dest, buf.as_bytes())
                    .with_context(|| format!("failed to write to {}", self.path(dest)))?;
            }
        }
        Ok(())
    }

    fn path<'a>(&self, path: &'a Path) -> impl std::fmt::Display + 'a {
        path.strip_prefix(&self.base).unwrap_or(path).display()
    }
}

fn filter_document(document: Yaml) -> Yaml {
    match document {
        Yaml::Hash(map) => Yaml::Hash(
            map.into_iter()
                .filter(|(key, _)| {
                    if let Yaml::String(string) = &key { string != REMOVE_MAP_KEY } else { true }
                })
                .map(|(key, value)| (filter_document(key), filter_document(value)))
                .collect(),
        ),
        Yaml::Array(vec) => {
            Yaml::Array(vec.into_iter().map(|item| filter_document(item)).collect())
        }
        other => other,
    }
}

fn main() {
    if let Err(err) = App::from_args().and_then(|app| app.run()) {
        eprintln!("error: {}", err);

        let mut source = err.as_ref() as &dyn Error;
        while let Some(err) = source.source() {
            eprintln!("caused by: {}", err);
            source = err;
        }

        std::process::exit(1);
    }
}

#[derive(Debug)]
struct StrError(String);

impl Error for StrError {}

impl std::fmt::Display for StrError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

#[derive(Debug)]
struct WithContext {
    context: String,
    source: Box<dyn Error>,
}

impl std::fmt::Display for WithContext {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.context)
    }
}

impl Error for WithContext {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

pub(crate) trait ResultExt<T> {
    fn with_context<F: FnOnce() -> String>(self, f: F) -> Result<T, Box<dyn Error>>;
}

impl<T, E: Into<Box<dyn Error>>> ResultExt<T> for Result<T, E> {
    fn with_context<F: FnOnce() -> String>(self, f: F) -> Result<T, Box<dyn Error>> {
        match self {
            Ok(ok) => Ok(ok),
            Err(err) => Err(WithContext { source: err.into(), context: f() }.into()),
        }
    }
}
