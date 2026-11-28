//! The same locked notice inventory is embedded and staged with RS packages.
use anyhow::{Result, ensure};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, OnceLock},
};

#[derive(Deserialize)]
struct Inventory {
    licenses: Vec<Group>,
}
#[derive(Deserialize)]
struct Group {
    id: String,
    name: String,
    text: String,
    packages: Vec<Dependency>,
}
#[derive(Deserialize)]
struct Dependency {
    name: String,
    version: String,
}
#[derive(PartialEq, Eq)]
pub struct License {
    pub id: String,
    pub name: String,
    pub text: String,
}
pub struct Package {
    pub name: String,
    pub version: String,
    pub components: BTreeSet<&'static str>,
    pub licenses: Vec<Arc<License>>,
}
impl Package {
    pub fn label(&self) -> String {
        if self.version.is_empty() {
            self.name.clone()
        } else {
            format!("{} {}", self.name, self.version)
        }
    }
    pub fn matches(&self, query: &str) -> bool {
        let text = format!(
            "{} {} {} {}",
            self.name,
            self.version,
            self.components
                .iter()
                .copied()
                .collect::<Vec<_>>()
                .join(" "),
            self.licenses
                .iter()
                .map(|license| license.id.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        )
        .to_lowercase();
        query
            .split_whitespace()
            .all(|term| text.contains(&term.to_lowercase()))
    }
    pub fn text(&self) -> String {
        self.licenses
            .iter()
            .map(|license| format!("{} ({})\n\n{}", license.name, license.id, license.text))
            .collect::<Vec<_>>()
            .join("\n\n────────────────────────────────\n\n")
    }
}
pub struct Catalog {
    pub packages: Vec<Package>,
}
impl Catalog {
    fn parse(inventories: &[(&'static str, &str)], manual: &[(&str, &str, &str)]) -> Result<Self> {
        let mut packages: BTreeMap<(String, String), Package> = BTreeMap::new();
        let mut licenses: Vec<Arc<License>> = Vec::new();
        for (component, json) in inventories {
            let inventory: Inventory = serde_json::from_str(json)?;
            for group in inventory.licenses {
                ensure!(
                    !group.id.trim().is_empty() && !group.text.trim().is_empty(),
                    "The bundled license inventory is incomplete."
                );
                let license = License {
                    id: group.id,
                    name: group.name,
                    text: group.text,
                };
                let license =
                    if let Some(existing) = licenses.iter().find(|item| ***item == license) {
                        existing.clone()
                    } else {
                        let license = Arc::new(license);
                        licenses.push(license.clone());
                        license
                    };
                for dependency in group.packages {
                    ensure!(
                        !dependency.name.trim().is_empty() && !dependency.version.trim().is_empty(),
                        "A bundled package is missing its name or version."
                    );
                    let package = packages
                        .entry((dependency.name.clone(), dependency.version.clone()))
                        .or_insert_with(|| Package {
                            name: dependency.name,
                            version: dependency.version,
                            components: BTreeSet::new(),
                            licenses: Vec::new(),
                        });
                    package.components.insert(component);
                    if !package.licenses.contains(&license) {
                        package.licenses.push(license.clone());
                    }
                }
            }
        }
        for (name, id, text) in manual {
            ensure!(
                !text.trim().is_empty(),
                "A bundled native license is empty."
            );
            packages.insert(
                ((*name).into(), String::new()),
                Package {
                    name: (*name).into(),
                    version: String::new(),
                    components: BTreeSet::from(["Native"]),
                    licenses: vec![Arc::new(License {
                        id: (*id).into(),
                        name: (*id).into(),
                        text: (*text).into(),
                    })],
                },
            );
        }
        let mut packages: Vec<_> = packages.into_values().collect();
        packages.sort_by_key(|package| (package.name.to_lowercase(), package.version.clone()));
        ensure!(!packages.is_empty(), "No bundled licenses were found.");
        Ok(Self { packages })
    }
}
pub fn bundled() -> Result<Arc<Catalog>, String> {
    static CATALOG: OnceLock<Result<Arc<Catalog>, String>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            Catalog::parse(
                &[
                    (
                        "Rust",
                        include_str!("../../assets/licenses/nauterm-rs-rust.json"),
                    ),
                    (
                        "Mosh",
                        include_str!("../../../assets/licenses/nauterm-mosh-rust.json"),
                    ),
                ],
                &[
                    (
                        "Nerd Fonts Symbols",
                        "MIT",
                        include_str!("../../../assets/licenses/manual/nerd-fonts-LICENSE"),
                    ),
                    (
                        "libghostty-vt",
                        "MIT",
                        include_str!("../../../assets/licenses/manual/ghostty-LICENSE"),
                    ),
                    (
                        "Lucide icons",
                        "ISC + MIT",
                        include_str!("../../assets/licenses/manual/lucide-LICENSE"),
                    ),
                    #[cfg(target_os = "macos")]
                    (
                        "Sparkle",
                        "MIT",
                        include_str!("../../../assets/licenses/manual/sparkle-LICENSE"),
                    ),
                ],
            )
            .map(Arc::new)
            .map_err(|error| error.to_string())
        })
        .clone()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_notices_include_native_and_mosh_packages_without_loading_user_files() {
        let catalog = bundled().unwrap();
        assert!(catalog.packages.len() > 100);
        for name in [
            "libghostty-vt",
            "Lucide icons",
            "gpui-pre",
            "gpui-base",
            "zeroize",
        ] {
            assert!(catalog.packages.iter().any(|package| package.name == name));
        }
        assert!(
            catalog
                .packages
                .iter()
                .any(|package| package.components.contains("Mosh"))
        );
        assert!(
            catalog
                .packages
                .iter()
                .all(|package| !package.licenses.is_empty() && !package.text().is_empty())
        );
    }
    #[test]
    fn shared_license_bodies_merge_packages_but_keep_different_versions_and_components()
    -> Result<()> {
        let source = r#"{"licenses":[{"id":"MIT","name":"MIT License","text":"Synthetic license","packages":[{"name":"example","version":"1"},{"name":"example","version":"2"}]}]}"#;
        let catalog = Catalog::parse(&[("Rust", source), ("Mosh", source)], &[])?;
        assert_eq!(catalog.packages.len(), 2);
        assert_eq!(catalog.packages[0].components.len(), 2);
        assert_eq!(catalog.packages[0].licenses.len(), 1);
        assert!(Arc::ptr_eq(
            &catalog.packages[0].licenses[0],
            &catalog.packages[1].licenses[0]
        ));
        assert!(catalog.packages[0].matches("EXAMPLE MIT mosh"));
        assert!(!catalog.packages[0].matches("unknown"));
        Ok(())
    }
}
