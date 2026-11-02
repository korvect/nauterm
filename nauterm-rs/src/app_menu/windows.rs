//! Recreate a closed main window after its owned work has drained. The Store
//! and recovery lease belong to the process; terminal views belong to a window.
use super::*;
use std::rc::Rc;

type Open = Rc<dyn Fn(&mut App) -> anyhow::Result<()>>;
struct Factory {
    open: Open,
    #[cfg(target_os = "macos")]
    closing: bool,
    #[cfg(target_os = "macos")]
    reopen: bool,
    #[cfg(target_os = "macos")]
    quit: bool,
    #[cfg(target_os = "macos")]
    task: Option<Task<()>>,
}
impl Global for Factory {}

pub fn set_window_factory(cx: &mut App, open: impl Fn(&mut App) -> anyhow::Result<()> + 'static) {
    cx.set_global(Factory {
        open: Rc::new(open),
        #[cfg(target_os = "macos")]
        closing: false,
        #[cfg(target_os = "macos")]
        reopen: false,
        #[cfg(target_os = "macos")]
        quit: false,
        #[cfg(target_os = "macos")]
        task: None,
    });
}

pub(super) fn has_main_window(cx: &App) -> bool {
    cx.try_global::<MainWindow>()
        .is_some_and(|main| cx.windows().contains(&main.handle))
}

pub(super) fn open(cx: &mut App) -> bool {
    #[cfg(target_os = "macos")]
    if let Some(factory) = cx.try_global::<Factory>()
        && factory.closing
    {
        cx.global_mut::<Factory>().reopen = true;
        return false;
    }
    let Some(open) = cx
        .try_global::<Factory>()
        .map(|factory| factory.open.clone())
    else {
        return false;
    };
    if let Err(error) = open(cx) {
        eprintln!("Cannot reopen Nauterm RS: {error:#}");
        return false;
    }
    true
}

#[cfg(target_os = "macos")]
pub fn close_main_window(window: &mut Window, cx: &mut App) {
    cx.global_mut::<Factory>().closing = true;
    window.remove_window();
    // First release the removed window's frame, views, session ownership and
    // cancellation guards. Do not keep an ordered-out live terminal tree.
    cx.defer(|cx| {
        let task = cx.spawn(async move |cx| {
            cx.background_executor()
                .spawn(async {
                    crate::lifecycle::prepare_window_close();
                })
                .await;
            cx.update(|cx| {
                let factory = cx.global_mut::<Factory>();
                factory.closing = false;
                factory.task = None;
                let quit = std::mem::take(&mut factory.quit);
                let reopen = std::mem::take(&mut factory.reopen);
                if quit {
                    cx.quit();
                } else if reopen {
                    open(cx);
                }
            });
        });
        cx.global_mut::<Factory>().task = Some(task);
    });
}

#[cfg(target_os = "macos")]
pub(super) fn defer_quit(cx: &mut App) -> bool {
    if cx
        .try_global::<Factory>()
        .is_some_and(|factory| factory.closing)
    {
        let factory = cx.global_mut::<Factory>();
        factory.quit = true;
        factory.reopen = false;
        true
    } else {
        false
    }
}
