use serde::{Serialize, Serializer, Deserialize, Deserializer};
use std::path::{Path, PathBuf};
use constraint::VersionConstraint;
use version::Version;
use std::fmt;
use std::fmt::Display;
use std::env;
use std::fs::File;
use std::io::Read;
use std::iter::FromIterator;
use std::clone::Clone;
use std::collections::BTreeMap;
use std::str;
use toml;
use license_exprs::validate_license_expr;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Io(err: ::std::io::Error) {
            cause(err)
            description(err.description())
            from()
        }
        Custom(err: String) {
            description(err)
            from()
        }
        FromToml(err: toml::de::Error) {
            cause(err)
            description(err.description())
            from()
        }
        ToToml(err: toml::ser::Error) {
            cause(err)
            description(err.description())
            from()
        }
    }
}

fn is_false(a: &bool) -> bool {
    !*a
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Manifest {
    pub name: PackageName,
    pub description: String,
    pub author: String,
    pub license: Option<String>,
    pub license_file: Option<String>,
    pub homepage: Option<String>,
    pub bugs: Option<String>,
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub private: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: DependencySet,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dev_dependencies: DependencySet,
}

impl Manifest {
    pub fn to_string(&self) -> Result<String, Error> {
        Ok(toml::ser::to_string(self)?)
    }
}

pub type DependencySet = BTreeMap<PackageName, VersionConstraint>;

pub type VersionSet = BTreeMap<PackageName, Version>;

#[derive(PartialEq, Eq, Hash, Default, Clone, PartialOrd, Ord)]
pub struct PackageName {
    pub namespace: Option<String>,
    pub name: String,
}

impl fmt::Debug for PackageName {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}/{}",
            self.namespace.clone().unwrap_or("<missing>".to_string()),
            self.name
        )
    }
}

// We probably want to disallow LPT1, etc.
// https://msdn.microsoft.com/en-us/library/aa561308.aspx

fn validate_package_namespace(s: &str) -> bool {
    s.chars().all(|c| {
        (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '_' || c == '-'
    }) && s.len() > 0 && s.len() <= 128 && s.chars().next().unwrap() != '-'
}

fn validate_package_name(s: &str) -> bool {
    s.chars().all(|c| {
        (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_' ||
            c == '-'
    }) && s.len() > 0 && s.len() <= 128 && s.chars().next().unwrap() != '-'
}

impl PackageName {
    pub fn from_str(s: &str) -> Option<PackageName> {
        let segments = s.split('/').count();
        if segments == 1 {
            if validate_package_name(s) {
                Some(PackageName {
                    namespace: None,
                    name: s.to_string(),
                })
            } else {
                None
            }
        } else if segments == 2 {
            let mut it = s.split('/');
            let namespace = it.next().unwrap();
            let name = it.next().unwrap();
            if validate_package_namespace(namespace) && validate_package_name(name) {
                Some(PackageName {
                    namespace: Some(namespace.to_string()),
                    name: name.to_string(),
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl Display for PackageName {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        match &self.namespace {
            &Some(ref namespace) => write!(f, "{}/{}", namespace, self.name),
            &None => write!(f, "{}", self.name),
        }
    }
}

impl Serialize for PackageName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&*self.to_string())
    }
}

impl<'de> Deserialize<'de> for PackageName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        let s = String::deserialize(deserializer)?;

        match PackageName::from_str(&s) {
            Some(package_name) => Ok(package_name),
            _ => Err(D::Error::custom("Invalid package name")),
        }
    }
}

fn normalise_dep(path: &String, dep: &PackageName) -> PackageName {
    PackageName {
        namespace: Some(dep.namespace.clone().unwrap_or((*path).clone())),
        name: dep.name.clone(),
    }
}

fn denormalise_dep(path: &String, dep: &PackageName) -> PackageName {
    match dep.namespace {
        Some(ref ns) => {
            PackageName {
                namespace: if ns == path {
                    None
                } else {
                    Some((*ns).clone())
                },
                name: dep.name.clone(),
            }
        }
        None => dep.clone(),
    }
}

fn normalise_deps(path: &String, deps: &DependencySet) -> DependencySet {
    DependencySet::from_iter(deps.into_iter().map(|(k, v)| {
        (normalise_dep(path, k), (*v).clone())
    }))
}

fn denormalise_deps(path: &String, deps: &DependencySet) -> DependencySet {
    DependencySet::from_iter(deps.into_iter().map(|(k, v)| {
        (denormalise_dep(path, k), (*v).clone())
    }))
}

pub fn normalise_manifest(manifest: &Manifest) -> Result<Manifest, Error> {
    validate_manifest(manifest)?;
    let path = manifest.name.clone().namespace.unwrap();
    let deps = normalise_deps(&path, &manifest.dependencies);
    let dev_deps = normalise_deps(&path, &manifest.dev_dependencies);
    let mut m = (*manifest).clone();
    m.dependencies = deps;
    m.dev_dependencies = dev_deps;
    Ok(m)
}

pub fn denormalise_manifest(manifest: &Manifest) -> Result<Manifest, Error> {
    validate_manifest(manifest)?;
    let path = manifest.name.clone().namespace.unwrap();
    let deps = denormalise_deps(&path, &manifest.dependencies);
    let dev_deps = denormalise_deps(&path, &manifest.dev_dependencies);
    let mut m = (*manifest).clone();
    m.dependencies = deps;
    m.dev_dependencies = dev_deps;
    Ok(m)
}

// TODO watch https://github.com/serde-rs/serde/issues/642 - when this issue is implemented,
// make the deserialiser call this function instead of calling it manually.
pub fn validate_manifest(manifest: &Manifest) -> Result<(), Error> {
    match manifest.name.namespace {
        None => {
            return Err(Error::Custom(
                "Package name must contain a namespace!".to_string(),
            ))
        }
        _ => (),
    }
    match &manifest.license {
        &Some(ref l) => {
            match validate_license_expr(l.as_str()) {
                Err(ref e) => return Err(Error::Custom(format!("{}", e))),
                _ => (),
            }
        }
        _ => (),
    }
    Ok(())
}

pub fn serialise_manifest(manifest: &Manifest) -> Result<String, Error> {
    denormalise_manifest(manifest)?.to_string()
}

pub fn deserialise_manifest(data: &String) -> Result<Manifest, Error> {
    Ok(normalise_manifest(&toml::from_str(data)?)?)
}

fn find_manifest(path: &Path) -> Option<PathBuf> {
    let manifest = path.join("manifest.toml");
    if manifest.exists() {
        Some(manifest)
    } else {
        path.parent().and_then(|p| find_manifest(p))
    }
}

pub fn find_manifest_path() -> Result<PathBuf, Error> {
    let cwd = env::current_dir()?;
    find_manifest(&cwd).ok_or(Error::Custom("no project file found!".to_string()))
}

pub fn find_project_dir() -> Result<PathBuf, Error> {
    let mut manifest_path = find_manifest_path()?;
    manifest_path.pop();
    Ok(manifest_path)
}

pub fn read_manifest() -> Result<Manifest, Error> {
    let manifest_path = find_manifest_path()?;
    let data = File::open(manifest_path).and_then(|mut f| {
        let mut s = String::new();
        f.read_to_string(&mut s).map(|_| s)
    })?;
    deserialise_manifest(&data)
}



#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn package_name_from_str() {
        assert_eq!(
            PackageName::from_str("a/B"),
            Some(PackageName {
                namespace: Some("a".to_string()),
                name: "B".to_string(),
            })
        );

        assert_eq!(
            PackageName::from_str("B"),
            Some(PackageName {
                namespace: None,
                name: "B".to_string(),
            })
        );

        assert_eq!(PackageName::from_str("A/B"), None);
        assert_eq!(PackageName::from_str("a/:-)"), None);
    }

    #[test]
    fn deserialise_and_normalise() {
        let left_pad: &'static str = "name = \"javascript/left-pad\"
description = \"A generalised sinister spatiomorphism.\"
author = \"IEEE Text Alignment Working Group\"

[dependencies]
right-pad = \"^8.23\"
";

        let m = deserialise_manifest(&left_pad.to_string()).unwrap();

        let mut my_deps = BTreeMap::new();
        my_deps.insert(
            PackageName {
                namespace: Some("javascript".to_string()),
                name: "right-pad".to_string(),
            },
            VersionConstraint::Caret(ver!(8, 23)),
        );
        assert_eq!(
            m,
            Manifest {
                name: PackageName {
                    namespace: Some("javascript".to_string()),
                    name: "left-pad".to_string(),
                },
                description: "A generalised sinister spatiomorphism.".to_string(),
                author: "IEEE Text Alignment Working Group".to_string(),
                license: None,
                license_file: None,
                homepage: None,
                bugs: None,
                repository: None,
                keywords: vec![],
                files: vec![],
                private: false,
                dev_dependencies: BTreeMap::new(),
                dependencies: my_deps,
            }
        );
    }

    #[test]
    fn denormalise_and_serialise() {
        let left_pad: &'static str = "name = \"javascript/left-pad\"
description = \"A generalised sinister spatiomorphism.\"
author = \"IEEE Text Alignment Working Group\"

[dependencies]
right-pad = \">= 8.23 < 9\"
";

        let mut my_deps = BTreeMap::new();
        my_deps.insert(
            PackageName {
                namespace: Some("javascript".to_string()),
                name: "right-pad".to_string(),
            },
            VersionConstraint::Range(Some(ver!(8, 23)), Some(ver!(9))),
        );
        let manifest = Manifest {
            name: PackageName {
                namespace: Some("javascript".to_string()),
                name: "left-pad".to_string(),
            },
            description: "A generalised sinister spatiomorphism.".to_string(),
            author: "IEEE Text Alignment Working Group".to_string(),
            license: None,
            license_file: None,
            homepage: None,
            bugs: None,
            repository: None,
            keywords: vec![],
            files: vec![],
            private: false,
            dev_dependencies: BTreeMap::new(),
            dependencies: my_deps,
        };

        let m = serialise_manifest(&manifest).unwrap();
        assert_eq!(m, left_pad);
    }

    #[test]
    #[should_panic]
    fn required_fields() {
        let left_pad: &'static str = "name = \"javascript/left-pad\"";
        deserialise_manifest(&left_pad.to_string()).unwrap();
    }

    #[test]
    #[should_panic]
    fn namespace_required() {
        let left_pad: &'static str = "name = \"left-pad\"
description = \"A generalised sinister spatiomorphism.\"
author = \"IEEE Text Alignment Working Group\"
";
        deserialise_manifest(&left_pad.to_string()).unwrap();
    }

    #[test]
    #[should_panic]
    fn reject_invalid_license_field() {
        let left_pad: &'static str = "name = \"left-pad\"
description = \"A generalised sinister spatiomorphism.\"
author = \"IEEE Text Alignment Working Group\"
license = \"LOLPL\"
";
        deserialise_manifest(&left_pad.to_string()).unwrap();
    }

    #[test]
    fn no_unexpected_fields() {
        let left_pad: &'static str = "name = \"javascript/left-pad\"
description = \"A generalised sinister spatiomorphism.\"
author = \"IEEE Text Alignment Working Group\"
hippopotamus = \"A large, thick-skinned, semiaquatic African mammal.\"
";
        let r = deserialise_manifest(&left_pad.to_string());
        assert!(r.is_err());
        match r {
            Err(e) => {
                let m = format!("{:?}", e);
                assert!(
                    m.contains("unknown field `hippopotamus`"),
                    "error message {:?} doesn't complain about \"hippopotamus\"",
                    m
                )
            }
            _ => panic!("parsing unexpected fields didn't return an error!"),
        }
    }

    #[test]
    fn accepts_all_defined_fields() {
        let left_pad: &'static str = "name = \"javascript/left-pad\"
description = \"A generalised sinister spatiomorphism.\"
author = \"IEEE Text Alignment Working Group\"
license = \"GPL-3.0+\"
licenseFile = \"LICENSE.txt\"
homepage = \"https://left-pad.com/\"
bugs = \"https://jira.left-pad.com\"
repository = \"https://git.left-pad.com/left-pad.git\"
keywords = [ \"left-pad\", \"left\", \"pad\", \"leftpad\" ]
files = [ \"index.js\" ]
private = false

[dependencies]
right-pad = \"^1.2.3\"
down-pad = \"~5.6.0\"

[devDependencies]
webpack = \"^7.0.5\"
widdershins-pad = \"^4.0.0\"
";
        deserialise_manifest(&left_pad.to_string()).unwrap();
    }
}
