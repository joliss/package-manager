use im::OrdMap as Map;
use pm_lib::package::PackageName;
use pm_lib::version::Version;
use solver::mappable::Mappable;
use solver::path::Path;
use std::convert::From;
use std::iter::{FromIterator, IntoIterator};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JustifiedVersion {
    pub version: Arc<Version>,
    pub path: Path,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartialSolution(pub Map<Arc<PackageName>, JustifiedVersion>);

impl PartialSolution {
    pub fn new() -> PartialSolution {
        PartialSolution(Map::new())
    }
}

impl Mappable for PartialSolution {
    type K = Arc<PackageName>;
    type V = JustifiedVersion;

    fn as_map(&self) -> &Map<Self::K, Self::V> {
        &self.0
    }

    fn wrap(m: Map<Self::K, Self::V>) -> Self {
        PartialSolution(m)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Solution(pub Map<Arc<PackageName>, Arc<Version>>);

impl Mappable for Solution {
    type K = Arc<PackageName>;
    type V = Arc<Version>;

    fn as_map(&self) -> &Map<Self::K, Self::V> {
        &self.0
    }

    fn wrap(m: Map<Self::K, Self::V>) -> Self {
        Solution(m)
    }
}

impl FromIterator<(Arc<PackageName>, Arc<Version>)> for Solution {
    fn from_iter<T>(iter: T) -> Solution
    where
        T: IntoIterator<Item = (Arc<PackageName>, Arc<Version>)>,
    {
        Solution(Map::<Arc<PackageName>, Arc<Version>>::from_iter(iter))
    }
}

impl From<PartialSolution> for Solution {
    fn from(partial_solution: PartialSolution) -> Solution {
        // Strip all paths from a PartialSolution to obtain a Solution
        partial_solution
            .iter()
            .map(|(package_name, justified_version)| {
                (package_name.clone(), justified_version.version.clone())
            })
            .collect()
    }
}
