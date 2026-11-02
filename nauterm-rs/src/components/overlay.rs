//! Window-owned overlay layers. GPUI provides painting/input primitives, not
//! open state, dismissal, stacking, focus return or enter/exit behavior.
use super::{
    button::Button,
    motion::{self, Transition},
};
use gpui_kit::{
    base::{Align, POPUP_PRIORITY as OVERLAY_PRIORITY, Placement, Positioner, Root, RootPlugin},
    component::{ActiveTheme, Selectable, StyledExt},
    prelude::*,
    *,
};
use std::{
    cell::Cell,
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};
type Changed = Rc<dyn Fn(&bool, &mut Window, &mut App)>;
type Content = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;
type ModalDismiss = Rc<dyn Fn(&mut Window, &mut App)>;
gpui_kit::actions!(overlay, [Dismiss]);
#[derive(Default)]
struct Registry(HashMap<WindowId, WeakEntity<OverlayLayer>>);
impl Global for Registry {}
pub fn init(cx: &mut App) {
    super::menu::init(cx);
    super::modal::init(cx);
    cx.bind_keys([KeyBinding::new("escape", Dismiss, Some("NautermOverlay"))]);
    cx.set_global(Registry::default());
    Root::register_plugin(cx, |window, cx| {
        let weak = cx.weak_entity();
        cx.global_mut::<Registry>()
            .0
            .insert(window.window_handle().window_id(), weak);
        OverlayLayer {
            entries: vec![],
            modals: vec![],
        }
    });
    cx.on_window_closed(|cx, id| {
        cx.global_mut::<Registry>().0.remove(&id);
    })
    .detach();
    super::tooltip::init(cx);
    super::message::init(cx);
}
pub(crate) fn manager(window: &Window, cx: &App) -> Entity<OverlayLayer> {
    cx.global::<Registry>()
        .0
        .get(&window.window_handle().window_id())
        .and_then(WeakEntity::upgrade)
        .expect("OverlayLayer initialized before opening windows")
}
pub fn is_modal_open(window: &Window, cx: &App) -> bool {
    manager(window, cx).read(cx).modal_top().is_some()
}
struct ModalScope {
    id: EntityId,
    focus: FocusHandle,
    priority: usize,
    dismiss: ModalDismiss,
    complete: ModalDismiss,
}
struct Lease {
    open: bool,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    trigger_focus: FocusHandle,
}
struct Entry {
    id: EntityId,
    owner: WeakEntity<Lease>,
    parent: Option<EntityId>,
    focus: FocusHandle,
    inline_focus: Option<FocusHandle>,
    content_focus: Option<FocusHandle>,
    origin_focus: Option<FocusHandle>,
    return_focus: Option<FocusHandle>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    anchor: Anchor,
    position: Option<Point<Pixels>>,
    side: Option<Placement>,
    fixed_side: bool,
    content: Content,
    changed: Option<Changed>,
    motion: Transition,
    closing: bool,
    duration: Duration,
    protect_trigger: bool,
    surface_bounds: Rc<Cell<Bounds<Pixels>>>,
    modal: Option<EntityId>,
    priority: usize,
    trigger_focus: FocusHandle,
}
impl Entry {
    fn contains_handle(&self, focus: &FocusHandle, window: &Window) -> bool {
        self.focus.contains(focus, window)
            || self
                .inline_focus
                .as_ref()
                .is_some_and(|inline| inline.contains(focus, window))
    }
    fn contains_focus(&self, window: &Window, cx: &App) -> bool {
        self.focus.contains_focused(window, cx)
            || self
                .inline_focus
                .as_ref()
                .is_some_and(|focus| focus.contains_focused(window, cx))
    }
}
pub struct OverlayLayer {
    entries: Vec<Entry>,
    modals: Vec<ModalScope>,
}
impl RootPlugin for OverlayLayer {}
impl OverlayLayer {
    pub(super) fn modal_top(&self) -> Option<EntityId> {
        self.modals.last().map(|modal| modal.id)
    }
    pub(super) fn modal_focus(&self) -> Option<FocusHandle> {
        self.modals.last().map(|modal| modal.focus.clone())
    }
    pub(super) fn modal_dismiss(&self) -> Option<ModalDismiss> {
        self.modals.last().map(|modal| modal.dismiss.clone())
    }
    pub(super) fn modal_complete(&self, focus: &FocusHandle) -> Option<ModalDismiss> {
        self.modals
            .iter()
            .find(|modal| &modal.focus == focus)
            .map(|modal| modal.complete.clone())
    }
    pub(super) fn modal_priority(&self, id: EntityId) -> usize {
        self.modals
            .iter()
            .find(|modal| modal.id == id)
            .map_or(1000, |modal| modal.priority)
    }
    pub(super) fn begin_modal(
        &mut self,
        id: EntityId,
        focus: FocusHandle,
        dismiss: ModalDismiss,
        complete: ModalDismiss,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.modals.iter().any(|modal| modal.id == id) {
            return;
        }
        while let Some(id) = self
            .entries
            .iter()
            .find(|entry| !entry.closing)
            .map(|entry| entry.id)
        {
            self.close(id, false, window, cx);
        }
        self.modals.push(ModalScope {
            id,
            focus,
            priority: 1000 * (self.modals.len() + 1),
            dismiss,
            complete,
        });
        cx.notify();
    }
    pub(super) fn end_modal(
        &mut self,
        id: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let top = self.modal_top() == Some(id);
        while let Some(id) = self
            .entries
            .iter()
            .find(|entry| entry.modal == Some(id) && !entry.closing)
            .map(|entry| entry.id)
        {
            self.close(id, false, window, cx);
        }
        self.modals.retain(|modal| modal.id != id);
        cx.notify();
        top
    }
    fn top(&self) -> Option<EntityId> {
        self.entries
            .iter()
            .rev()
            .find(|entry| !entry.closing)
            .map(|e| e.id)
    }
    fn outside(
        &mut self,
        id: EntityId,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.top() != Some(id) {
            return;
        }
        let mut target = id;
        let mut parent = self
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .and_then(|entry| entry.parent);
        while let Some(parent_id) = parent {
            let Some(entry) = self.entries.iter().find(|entry| entry.id == parent_id) else {
                break;
            };
            if entry.surface_bounds.get().contains(&position)
                || (entry.protect_trigger && entry.bounds.get().contains(&position))
            {
                break;
            }
            target = parent_id;
            parent = entry.parent;
        }
        self.close(target, false, window, cx);
    }
    fn close(&mut self, id: EntityId, restore: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(start) = self.entries.iter().position(|e| e.id == id) else {
            return;
        };
        // Closing a parent closes its branch; closing a child leaves its parent.
        let targets = branch_members(
            &self
                .entries
                .iter()
                .map(|e| (e.id, e.parent))
                .collect::<Vec<_>>(),
            id,
        );
        let previous_focus = window.focused(cx);
        let focus_belongs_to_branch = self
            .entries
            .iter()
            .any(|entry| targets.contains(&entry.id) && entry.contains_focus(window, cx));
        let return_focus = self.entries[start].return_focus.clone();
        for entry in self
            .entries
            .iter_mut()
            .rev()
            .filter(|entry| targets.contains(&entry.id))
        {
            if entry.closing {
                continue;
            }
            entry.closing = true;
            entry
                .motion
                .set(0., Instant::now(), entry.duration, cx.reduce_motion());
            if let Some(owner) = entry.owner.upgrade() {
                owner.update(cx, |s, cx| {
                    s.open = false;
                    cx.notify();
                });
            }
            if let Some(changed) = &entry.changed {
                changed(&false, window, cx);
            }
        }
        if restore && let Some(focus) = &return_focus {
            focus.focus(window, cx);
        } else if focus_belongs_to_branch {
            // Mouse-down-out runs in capture, before the clicked input has
            // received focus. Return only if that target did not move it.
            window.defer(cx, move |window, cx| {
                if window.focused(cx) == previous_focus
                    && let Some(focus) = return_focus
                {
                    focus.focus(window, cx);
                }
            });
        }
        cx.notify();
    }
    fn show(&mut self, mut entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        if self.entries.iter().any(|e| e.id == entry.id && !e.closing) {
            return;
        }
        if let Some(modal) = self.modals.last() {
            let origin = entry.origin_focus.as_ref().or(entry.inline_focus.as_ref());
            if !modal.focus.contains(&entry.trigger_focus, window)
                && !origin.is_some_and(|focus| modal.focus.contains(focus, window))
            {
                if let Some(changed) = &entry.changed {
                    changed(&false, window, cx);
                }
                return;
            }
            entry.modal = Some(modal.id);
            entry.priority = modal.priority + OVERLAY_PRIORITY;
        }
        entry.parent = self
            .entries
            .iter()
            .rev()
            .find(|e| {
                !e.closing
                    && entry.origin_focus.as_ref().map_or_else(
                        || e.contains_focus(window, cx),
                        |origin| e.contains_handle(origin, window),
                    )
            })
            .map(|e| e.id);
        let keep = branch_length(
            &self
                .entries
                .iter()
                .filter(|e| !e.closing)
                .map(|e| e.id)
                .collect::<Vec<_>>(),
            entry.parent,
        );
        let close = self
            .entries
            .iter()
            .filter(|e| !e.closing)
            .nth(keep)
            .map(|e| e.id);
        if let Some(id) = close {
            self.close(id, false, window, cx);
        }
        if entry.inline_focus.is_none() {
            entry
                .content_focus
                .as_ref()
                .unwrap_or(&entry.focus)
                .focus(window, cx);
        }
        if let Some(index) = self.entries.iter().position(|e| e.id == entry.id) {
            entry.motion = self.entries.remove(index).motion;
        }
        entry
            .motion
            .set(1., Instant::now(), entry.duration, cx.reduce_motion());
        self.entries.push(entry);
        cx.notify();
    }
}
fn branch_length<K: PartialEq>(active: &[K], parent: Option<K>) -> usize {
    parent
        .and_then(|id| active.iter().position(|v| v == &id).map(|i| i + 1))
        .unwrap_or(0)
}
fn branch_members<K: Copy + PartialEq>(entries: &[(K, Option<K>)], root: K) -> Vec<K> {
    let mut branch = vec![];
    for (id, parent) in entries {
        if *id == root || parent.is_some_and(|parent| branch.contains(&parent)) {
            branch.push(*id);
        }
    }
    branch
}
fn exit_finished(closing: bool, motion: &Transition, now: Instant, reduced: bool) -> bool {
    closing && (reduced || motion.value(now) <= 0.001)
}
impl Render for OverlayLayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let reduced = cx.reduce_motion();
        let relationships = self
            .entries
            .iter()
            .map(|entry| (entry.id, entry.parent))
            .collect::<Vec<_>>();
        let lost = self
            .entries
            .iter()
            .filter(|entry| entry.owner.upgrade().is_none())
            .flat_map(|entry| branch_members(&relationships, entry.id))
            .collect::<Vec<_>>();
        let callbacks = self
            .entries
            .iter()
            .filter(|entry| lost.contains(&entry.id) && !entry.closing)
            .filter_map(|entry| entry.changed.clone())
            .collect::<Vec<_>>();
        self.entries.retain(|e| {
            !lost.contains(&e.id) && !exit_finished(e.closing, &e.motion, now, reduced)
        });
        if !callbacks.is_empty() {
            window.defer(cx, move |window, cx| {
                for callback in callbacks {
                    callback(&false, window, cx);
                }
            });
        }
        // Surfaces stay under their trigger's dispatch ancestry. Moving them
        // into this root overlay would lose Settings/Main key contexts (e.g.
        // Cmd-Q) and make child overlays appear unrelated to their parent.
        div()
    }
}
impl OverlayLayer {
    fn surface(
        &mut self,
        id: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let depth = self.entries.iter().position(|e| e.id == id)?;
        let now = Instant::now();
        let reduced = cx.reduce_motion();
        if exit_finished(
            self.entries[depth].closing,
            &self.entries[depth].motion,
            now,
            reduced,
        ) {
            self.entries.remove(depth);
            return None;
        }
        let weak = cx.weak_entity();
        let entry = &mut self.entries[depth];
        let (opacity, moving) = entry.motion.set(
            if entry.closing { 0. } else { 1. },
            now,
            entry.duration,
            reduced,
        );
        if moving {
            window.request_animation_frame();
        }
        let owner = weak.clone();
        let outside = weak.clone();
        let bounds = entry.bounds.get();
        let trigger_bounds = entry.bounds.clone();
        let protect_trigger = entry.protect_trigger;
        let surface_bounds = entry.surface_bounds.clone();
        // Enter/exit slides away from the trigger on either side.
        let slide = if entry.side == Some(Placement::Top) {
            3.
        } else {
            -3.
        };
        let position = entry.position.unwrap_or_else(|| match entry.anchor {
            Anchor::TopRight => point(bounds.right(), bounds.bottom() + px(4.)),
            Anchor::BottomLeft => point(bounds.left(), bounds.top() - px(4.)),
            Anchor::BottomRight => point(bounds.right(), bounds.top() - px(4.)),
            _ => point(bounds.left(), bounds.bottom() + px(4.)),
        });
        let panel = div()
            .id(("overlay-surface", id))
            .occlude()
            .role(Role::Pane)
            .key_context("NautermOverlay")
            .track_focus(&entry.focus)
            .on_action(move |_: &Dismiss, w, cx| {
                let _ = owner.update(cx, |s, cx| {
                    if s.top() == Some(id) {
                        s.close(id, true, w, cx);
                    }
                });
                cx.stop_propagation();
            })
            .on_mouse_down_out(move |event, w, cx| {
                // The trigger is part of the interaction, not an outside click.
                // In particular its chevron must be able to close an open panel.
                if trigger_bounds.get().contains(&event.position)
                    && (protect_trigger || event.button == MouseButton::Right)
                {
                    return;
                }
                let _ = outside.update(cx, |s, cx| {
                    s.outside(id, event.position, w, cx);
                });
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            // A right click inside the surface belongs to its content, never
            // the trigger/ancestor card or input frame behind the overlay.
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            // Children may scroll, but an overlay never scrolls its trigger or
            // the page behind it, including empty content and list boundaries.
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .opacity(opacity)
            .relative()
            .top(px(slide * (1. - opacity)))
            .child((entry.content)(window, cx))
            .when(entry.closing, |panel| {
                panel.child(
                    div()
                        .absolute()
                        .inset_0()
                        .occlude()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation()),
                )
            });
        let positioner = if let Some(side) = entry.side {
            // Read at prepaint, after the trigger's canvas recorded this
            // frame's bounds, so a scrolling trigger carries its panel.
            let positioner = Positioner::side(bounds)
                .tracked_trigger_bounds(entry.bounds.clone())
                .placement(side)
                .align(Align::Start)
                .offset(px(4.));
            if entry.fixed_side {
                positioner.fixed_placement()
            } else {
                positioner
            }
        } else {
            Positioner::corner(entry.anchor, position)
        };
        Some(
            deferred(
                positioner
                    .margin(px(8.))
                    .on_position(move |resolved| surface_bounds.set(resolved.bounds))
                    .child(panel),
            )
            .with_priority(entry.priority + depth)
            .into_any_element(),
        )
    }
}
#[derive(IntoElement)]
pub struct Overlay {
    id: ElementId,
    open: bool,
    anchor: Anchor,
    style: StyleRefinement,
    trigger: Option<Box<dyn FnOnce(bool, Changed) -> AnyElement>>,
    content: Option<Content>,
    changed: Option<Changed>,
    inline_focus: Option<FocusHandle>,
    restore_focus: bool,
    duration: Duration,
    protect_trigger: bool,
    content_focus: Option<FocusHandle>,
    origin_focus: Option<FocusHandle>,
    position: Option<Point<Pixels>>,
    side: Option<Placement>,
    fixed_side: bool,
    attached: bool,
}
impl Overlay {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            open: false,
            anchor: Anchor::TopLeft,
            style: StyleRefinement::default(),
            trigger: None,
            content: None,
            changed: None,
            inline_focus: None,
            restore_focus: true,
            duration: motion::OVERLAY,
            protect_trigger: true,
            content_focus: None,
            origin_focus: None,
            position: None,
            side: None,
            fixed_side: false,
            attached: false,
        }
    }
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
    pub fn anchor(mut self, anchor: Anchor) -> Self {
        self.anchor = anchor;
        self
    }
    pub fn position(mut self, position: Point<Pixels>) -> Self {
        self.position = Some(position);
        self
    }
    pub fn side(mut self, side: Placement) -> Self {
        self.side = Some(side);
        self
    }
    /// Never flip to the opposite side, e.g. while the trigger scrolls.
    pub fn fixed_side(mut self) -> Self {
        self.fixed_side = true;
        self
    }
    pub fn content_focus(mut self, focus: &FocusHandle) -> Self {
        self.content_focus = Some(focus.clone());
        self
    }
    /// For a caller that already moved focus synchronously (e.g. a base editor
    /// context handler), preserve its return target and parent overlay scope.
    pub fn origin_focus(mut self, focus: &FocusHandle) -> Self {
        self.origin_focus = Some(focus.clone());
        self
    }
    pub fn restore_focus(mut self, restore: bool) -> Self {
        self.restore_focus = restore;
        self
    }
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
    pub fn protect_trigger(mut self, protect: bool) -> Self {
        self.protect_trigger = protect;
        self
    }
    pub fn on_open_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.changed = Some(Rc::new(f));
        self
    }
    pub fn trigger<const S: bool>(mut self, button: Button<S>) -> Self {
        self.trigger = Some(Box::new(move |open, change| {
            button
                .open(open)
                .on_click(move |_, w, cx| change(&!open, w, cx))
                .into_any_element()
        }));
        self
    }
    /// An editable trigger keeps its native editor, rather than becoming a Button.
    pub fn trigger_element(mut self, element: impl IntoElement) -> Self {
        let element = element.into_any_element();
        self.trigger = Some(Box::new(move |_, _| element));
        self
    }
    /// Use the parent's bounds without introducing an in-flow wrapper. In
    /// particular, a full-size canvas must retain its parent's definite size.
    pub fn attach_to<E: ParentElement>(mut self, element: E) -> E {
        self.attached = true;
        self.trigger = None;
        element.child(self)
    }
    /// Suggestions belong to the inline editor's focus scope. Opening must not
    /// steal its caret/IME focus, and closing after Tab/blur must not steal it back.
    pub fn inline_focus(mut self, focus: &FocusHandle) -> Self {
        self.inline_focus = Some(focus.clone());
        self.restore_focus = false;
        self
    }
    pub fn content(mut self, f: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> Self {
        self.content = Some(Rc::new(f));
        self
    }
}
impl Styled for Overlay {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl RenderOnce for Overlay {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let lease = window.use_keyed_state(self.id.clone(), cx, |_, cx| Lease {
            open: false,
            bounds: Rc::new(Cell::new(Bounds::default())),
            trigger_focus: cx.focus_handle().tab_stop(false),
        });
        let trigger_focus = lease.read(cx).trigger_focus.clone();
        // An attached overlay is not a new control. Keep the focus established
        // by its owner, including the originating modal/parent overlay scope.
        let origin_focus = self
            .origin_focus
            .clone()
            .or_else(|| self.attached.then(|| window.focused(cx)).flatten());
        let manager = manager(window, cx);
        let bounds = lease.read(cx).bounds.clone();
        let was_open = lease.read(cx).open;
        if self.open != was_open {
            lease.update(cx, |s, _| s.open = self.open);
            if self.open {
                let style = self.style.clone();
                let content = self.content.clone();
                let factory: Content = Rc::new(move |window, cx| {
                    div()
                        .rounded(px(8.))
                        .p(px(10.))
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .shadow_sm()
                        .refine_style(&style)
                        .when_some(content.clone(), |v, f| v.child(f(window, cx)))
                        .into_any_element()
                });
                let entry = Entry {
                    id: lease.entity_id(),
                    owner: lease.downgrade(),
                    parent: None,
                    focus: cx.focus_handle(),
                    inline_focus: self.inline_focus,
                    content_focus: self.content_focus,
                    return_focus: origin_focus.clone().or_else(|| window.focused(cx)),
                    origin_focus,
                    bounds: bounds.clone(),
                    anchor: self.anchor,
                    position: self.position,
                    side: self.side,
                    fixed_side: self.fixed_side,
                    content: factory,
                    changed: self.changed.clone(),
                    motion: Transition::new(0.),
                    closing: false,
                    duration: self.duration,
                    protect_trigger: self.protect_trigger,
                    surface_bounds: Rc::new(Cell::new(Bounds::default())),
                    modal: None,
                    priority: OVERLAY_PRIORITY,
                    trigger_focus: trigger_focus.clone(),
                };
                manager.update(cx, |m, cx| m.show(entry, window, cx));
            } else {
                manager.update(cx, |m, cx| {
                    m.close(lease.entity_id(), self.restore_focus, window, cx)
                });
            }
        }
        let changed = self.changed;
        let toggle: Changed = Rc::new(move |open, w, cx| {
            if let Some(f) = &changed {
                f(open, w, cx);
            }
        });
        let weak = manager.downgrade();
        // A side panel belongs next to a visible trigger. Once scrolling moves
        // the trigger out of its clip, close instead of floating detached.
        let clipped = (self.open && self.side.is_some()).then(|| (weak.clone(), lease.entity_id()));
        let surface = manager.update(cx, |manager, cx| {
            if let Some(entry) = manager
                .entries
                .iter_mut()
                .find(|entry| entry.id == lease.entity_id())
            {
                entry.position = self.position;
                entry.side = self.side;
                entry.fixed_side = self.fixed_side;
                entry.anchor = self.anchor;
            }
            manager.surface(lease.entity_id(), window, cx)
        });
        div()
            .id(self.id)
            .when(!self.attached, |host| host.track_focus(&trigger_focus))
            .tab_stop(false)
            .relative()
            .when(self.attached, |host| host.absolute().inset_0())
            .when_some(self.trigger, |v, f| v.child(f(self.open, toggle)))
            .when_some(surface, |view, surface| view.child(surface))
            .child(
                canvas(
                    move |rect, window, cx| {
                        if bounds.replace(rect) != rect {
                            let _ = weak.update(cx, |_, cx| cx.notify());
                        }
                        if let Some((manager, id)) = clipped.clone()
                            && !rect.intersects(&window.content_mask().bounds)
                        {
                            window.defer(cx, move |window, cx| {
                                let _ = manager.update(cx, |m, cx| m.close(id, false, window, cx));
                            });
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
    }
}
#[cfg(test)]
mod tests {
    use super::{branch_length, branch_members, exit_finished};
    use crate::components::motion::{OVERLAY, Transition};
    use std::time::Instant;
    #[test]
    fn nested_overlays_keep_only_the_requested_parent_branch() {
        let active = [1, 2, 3];
        assert_eq!(branch_length(&active, Some(2)), 2);
        assert_eq!(branch_length(&active, Some(3)), 3);
        assert_eq!(branch_length(&active, None), 0);
        assert_eq!(branch_length(&active, Some(7)), 0);
    }
    #[test]
    fn dismissing_a_branch_does_not_close_unrelated_surfaces() {
        let entries = [
            (1, None),
            (2, Some(1)),
            (3, Some(2)),
            (4, None),
            (5, Some(4)),
        ];
        assert_eq!(branch_members(&entries, 2), vec![2, 3]);
        assert_eq!(branch_members(&entries, 1), vec![1, 2, 3]);
        assert_eq!(branch_members(&entries, 4), vec![4, 5]);
    }
    #[test]
    fn closing_surfaces_stay_until_exit_and_reduced_motion_removes_immediately() {
        let now = Instant::now();
        let mut transition = Transition::new(1.);
        transition.set(0., now, OVERLAY, false);
        assert!(!exit_finished(true, &transition, now, false));
        assert!(exit_finished(true, &transition, now + OVERLAY, false));
        assert!(exit_finished(true, &transition, now, true));
        assert!(!exit_finished(false, &Transition::new(0.), now, false));
    }
}
