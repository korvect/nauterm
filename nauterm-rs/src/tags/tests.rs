use super::*;
use crate::model::Host;

fn sample() -> AppState {
    let mut data = AppState {
        next_id: 5,
        hosts: vec![
            Host {
                id: 2,
                name: "One".into(),
                address: "one.invalid".into(),
                port: 22,
                tags: vec!["Prod".into(), "Linux".into()],
                ..Default::default()
            },
            Host {
                id: 3,
                name: "Two".into(),
                address: "two.invalid".into(),
                port: 22,
                tags: vec!["linux".into()],
                ..Default::default()
            },
            Host {
                id: 4,
                name: "Other".into(),
                address: "other.invalid".into(),
                port: 22,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    materialize(&mut data);
    data
}
#[test]
fn stable_catalog_rename_delete_and_or_filter_preserve_host_references() -> Result<()> {
    let data = sample();
    assert_eq!(data.tags.len(), 2);
    assert_eq!(data.tags, inventory(&sample()));
    let selected = ids(&data.tags, &data.hosts[0].tags);
    let filter = Filter::new(&data.tags, &selected);
    assert!(filter.matches(&data.hosts[0]) && filter.matches(&data.hosts[1]));
    assert!(!filter.matches(&data.hosts[2]));
    let prod = data.tags.iter().find(|tag| tag.name == "Prod").unwrap();
    let changed = rename(&data, &prod.id, "生产, West")?;
    assert_eq!(
        changed
            .tags
            .iter()
            .find(|tag| tag.id == prod.id)
            .unwrap()
            .name,
        "生产, West"
    );
    assert!(
        changed.hosts[0]
            .tags
            .iter()
            .any(|name| name == "生产, West")
    );
    assert!(
        names(&changed.tags, &selected)
            .iter()
            .any(|name| name == "生产, West")
    );
    assert!(rename(&changed, &prod.id, "LINUX").is_err());
    let deleted = delete(&changed, &prod.id)?;
    assert_eq!(deleted.hosts[0].tags, ["Linux"]);
    assert_eq!(names(&deleted.tags, &selected), ["Linux"]);
    // A stale, now absent filter must not hide the whole library.
    assert!(
        Filter::new(&deleted.tags, &BTreeSet::from([prod.id.clone()])).matches(&deleted.hosts[2])
    );
    data.validate()?;
    changed.validate()?;
    deleted.validate()?;
    Ok(())
}
#[test]
fn unused_tag_identity_survives_public_model_serialization() -> Result<()> {
    let (mut data, tag) = create(&sample(), "Unused")?;
    let (_, same) = create(&data, "unused")?;
    assert_eq!(same.id, tag.id);
    data.hosts.clear();
    let restored: AppState = serde_json::from_str(&serde_json::to_string(&data)?)?;
    assert!(restored.tags.iter().any(|value| value.id == tag.id));
    Ok(())
}
