use rustc_hash::FxHashMap;
use std::{
    fs,
    path::{Path, PathBuf},
    result::Result,
    string::ToString,
};
use unicode_bom::Bom;

pub struct ShimError {
    pub reason: ShimErrorKind,
    pub description: String,
}

impl ToString for ShimError {
    #[inline]
    fn to_string(&self) -> String {
        self.description.clone()
    }
}

pub enum ShimErrorKind {
    NotFound,
    Other,
    InvalidData,
}

pub struct Shim {
    pub target_path: PathBuf,
    pub args: Option<Vec<String>>,
}

impl Shim {
    pub fn init(current_exe: String) -> Result<Self, ShimError> {
        let shim_path = get_shim_file_path(current_exe)?;
        let kvs = parse_shim_file(&shim_path)?;
        let target_path = match kvs.get("path") {
            Some(p) => PathBuf::from(p),
            None => {
                let mut err = String::from("no path key in ");
                err.push_str(&shim_path.to_string_lossy());
                return Err(ShimError {
                    reason: ShimErrorKind::NotFound,
                    description: err,
                });
            }
        };
        let args = kvs.get("args").map(|a| {
            a.split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        });
        Ok(Self { target_path, args })
    }
}

#[inline]
fn get_shim_file_path(current_exe: String) -> Result<PathBuf, ShimError> {
    let mut current_exe = PathBuf::from(current_exe);
    if !current_exe.set_extension("shim") {
        let mut err: String = current_exe.to_string_lossy().into();
        err.push_str(" is not a file");
        Err(ShimError {
            reason: ShimErrorKind::Other,
            description: err,
        })
    } else {
        Ok(current_exe)
    }
}

fn unquote(val: &str) -> String {
    val.replacen('"', "", 1)
        .chars()
        .rev()
        .collect::<String>()
        .replacen('"', "", 1)
        .chars()
        .rev()
        .collect::<String>()
}

fn parse_shim_file(shim_path: &Path) -> Result<FxHashMap<String, String>, ShimError> {
    let mut kvs = FxHashMap::default();

    let raw_content = fs::read_to_string(shim_path).map_err(|e| {
        let mut err = String::from("reading ");
        err.push_str(&shim_path.to_string_lossy());
        err.push_str(": ");
        err.push_str(&e.to_string());
        ShimError {
            reason: ShimErrorKind::Other,
            description: err,
        }
    })?;
    //NOTE: expedient trick for utf-8 with bom
    let bom = Bom::from(raw_content.as_bytes());
    for line in raw_content[bom.len()..]
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        let mut components = line.split('=');
        let key = match components.next() {
            Some(k) => unquote(k.trim()),
            None => {
                let mut description = String::from("invaid line in shim file: ");
                description.push_str(line);
                return Err(ShimError {
                    reason: ShimErrorKind::InvalidData,
                    description,
                });
            }
        };
        let value = match components.next() {
            Some(v) => unquote(v.trim()),
            None => {
                let mut description = String::from("invaid line in shim file: ");
                description.push_str(line);
                return Err(ShimError {
                    reason: ShimErrorKind::InvalidData,
                    description,
                });
            }
        };
        kvs.insert(key, value);
    }
    Ok(kvs)
}
