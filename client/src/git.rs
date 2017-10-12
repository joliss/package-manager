#![allow(dead_code)]

use std::path::{Path,PathBuf};
use std::fs::canonicalize;
use git2::{Repository,Tree};
use git2;
use error::Error;

pub struct GitScmProvider {
    pub relative_package_root: PathBuf, // relative to repo.workdir()
    pub repo: Repository,
}

impl GitScmProvider {
    pub fn new(package_root: &Path) -> Result<Self, Error> {
        let absolute_package_root = canonicalize(package_root)?;
        let repo = Repository::discover(&absolute_package_root)?;
        let repo_workdir = match repo.workdir() {
            None => return Err(Error::from(GitError::BaseDir)),
            Some(p) => p,
        }.to_path_buf();
        let relative_package_root
            = match absolute_package_root.strip_prefix(&repo_workdir) {
                Ok(p) => p,
                Err(_) => return Err(Error::from(GitError::RepoNotParent(
                    repo_workdir.to_string_lossy().to_string(), absolute_package_root.to_string_lossy().to_string())))
            };
        Ok(GitScmProvider {
            relative_package_root: relative_package_root.to_path_buf(),
            repo: repo,
        })
    }

    pub fn check_is_pristine(&self) -> Result<(), Error> {
        if self.repo.state() != RepositoryState::Clean {
            return Err(Error::from(GitError::NotCleanState))
        }
    }

    pub fn ls_files(&self) -> Result<Vec<String>, Error> {
        let head_sha = self.repo.refname_to_id("HEAD")?;
        let head = self.repo.find_commit(head_sha)?;
        let tree = head.tree()?;
        let mut files = self.ls_files_inner(tree, "")?;
        files.sort();
        Ok(files.into_iter().filter_map(|file|
            if Path::new(&file).starts_with(&self.relative_package_root) {
                Some(Path::new(&file).strip_prefix(&self.relative_package_root)
                    .expect("Path::strip_prefix should succeed if Path::starts_with is true")
                    .to_str().expect("stripping a prefix should not break UTF-8 well-formedness").to_string())
            } else {
                None
            }
        ).collect())
    }

    fn ls_files_inner(&self, tree: Tree, prefix: &str) -> Result<Vec<String>, Error> {
        // TODO: Exclude (or throw error on) submodules
        let mut files = Vec::new();
        for entry in tree.iter() {
            let name = match entry.name() {
                None => return Err(Error::from(GitError::Utf8)),
                Some(n) => n
            };
            let relative_path = prefix.to_string() + name;
            match entry.kind() {
                Some(git2::ObjectType::Blob) => {
                    files.push(relative_path);
                },
                Some(git2::ObjectType::Tree) => {
                    let object = entry.to_object(&self.repo)?;
                    let subtree = match object.into_tree() {
                        Ok(t) => t,
                        Err(_) => return Err(Error::from(GitError::ObjectType))
                    };
                    files.extend(self.ls_files_inner(subtree, &(relative_path + "/"))?);
                }
                _ => return Err(Error::from(GitError::ObjectType))
            }
        }
        Ok(files)
    }
}


quick_error! {
    #[derive(Debug)]
    pub enum GitError {
        Utf8 {
            description("Git returned a filename that is not valid UTF-8")
        }
        BaseDir {
            description("Failed to get base directory of Git repository.")
        }
        RepoNotParent(repo_root: String, package_root: String) {
            display("Git found a repo at {}, which is not a parent directory of {}", repo_root, package_root)
        }
        ObjectType {
            description("Git returned an unexpected object type")
        }

        NotCleanState {
            description("A Git operation such as merge or rebase is in progress in your working tree")
        }
        NonEmptyStatus {
            description("There are modified or untracked files. Commit them to Git or add them to .gitignore before running this command.")
        }
    }
}


pub fn test_git() {
    println!("{:?}", GitScmProvider::new(&Path::new(".")).unwrap().ls_files());
    let repo = Repository::discover(&".").unwrap();
    let statuses = repo.statuses(None).unwrap();
    println!("{:?}", statuses.iter().map(|e| e.path().unwrap().to_string()).collect::<Vec<String>>());
}
