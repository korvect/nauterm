use super::{SessionView, TabContent, TerminalTab};
use crate::pane_tree::PaneTree;

#[test]
fn preparation_callbacks_cannot_publish_after_pane_close_move_or_superseding_request() {
    let session = |id| SessionView {
        id,
        title: "Synthetic pending SSH".into(),
        content: TabContent::Preparing(None),
        ssh: None,
        restore_target: None,
        restore_directory: None,
        files: None,
        tool_files: None,
        _file_events: vec![],
        _metadata: vec![],
        replay: None,
        preparation_generation: 1,
    };
    let mut tab = TerminalTab {
        id: 7,
        workspace: 2,
        tree: PaneTree::new(7, session(7)),
        show_sftp: false,
        tools: Default::default(),
    };
    // Another pending session survives closing the first, so tab existence alone
    // cannot authorize an old callback. Exercise the actual session tree.
    assert!(tab.tree.insert_session(7, session(8)).is_ok());
    assert!(tab.owns_preparation((7, 2), 1));
    assert!(tab.owns_preparation((8, 2), 1));
    tab.tree.remove_session(7);
    assert!(!tab.owns_preparation((7, 2), 1));
    assert!(tab.owns_preparation((8, 2), 1));
    tab.workspace = 3;
    assert!(!tab.owns_preparation((8, 2), 1));
    assert!(tab.owns_preparation((8, 3), 1));
    tab.tree.session_mut(8).unwrap().preparation_generation = 2;
    assert!(!tab.owns_preparation((8, 3), 1));
    assert!(tab.owns_preparation((8, 3), 2));
    tab.tree.session_mut(8).unwrap().content =
        TabContent::Preparing(Some("Synthetic storage error".into()));
    assert!(!tab.owns_preparation((8, 3), 2));
    tab.tree.remove_session(8);
    assert!(!tab.owns_preparation((8, 3), 2));
}
