//! Pure surface-presentation demand model; runnable with `rustc --test`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PresentationState {
    Unpresented,
    Presented,
    RetryBeforeFirstPresent,
    RetryAfterPresent,
}

impl PresentationState {
    pub(crate) fn requires_presentation(self) -> bool {
        matches!(
            self,
            Self::RetryBeforeFirstPresent | Self::RetryAfterPresent
        )
    }

    pub(crate) fn invalidated(self) -> Self {
        match self {
            Self::Unpresented | Self::RetryBeforeFirstPresent => Self::RetryBeforeFirstPresent,
            Self::Presented | Self::RetryAfterPresent => Self::RetryAfterPresent,
        }
    }
}

#[cfg(test)]
mod presentation_state_tests {
    use super::PresentationState;

    #[test]
    fn invalidation_tracks_whether_the_surface_has_presented() {
        assert_eq!(
            PresentationState::Unpresented.invalidated(),
            PresentationState::RetryBeforeFirstPresent
        );
        assert_eq!(
            PresentationState::RetryBeforeFirstPresent.invalidated(),
            PresentationState::RetryBeforeFirstPresent
        );
        assert_eq!(
            PresentationState::Presented.invalidated(),
            PresentationState::RetryAfterPresent
        );
        assert_eq!(
            PresentationState::RetryAfterPresent.invalidated(),
            PresentationState::RetryAfterPresent
        );
    }

    #[test]
    fn geometry_invalidation_keeps_presentation_required_until_a_fresh_buffer() {
        for state in [PresentationState::Unpresented, PresentationState::Presented] {
            let resized = state.invalidated();
            assert!(resized.requires_presentation());
            assert!(resized.invalidated().requires_presentation());
        }
        assert!(!PresentationState::Presented.requires_presentation());
    }

    #[test]
    fn only_retry_states_require_presentation() {
        assert!(!PresentationState::Unpresented.requires_presentation());
        assert!(!PresentationState::Presented.requires_presentation());
        assert!(PresentationState::RetryBeforeFirstPresent.requires_presentation());
        assert!(PresentationState::RetryAfterPresent.requires_presentation());
    }
}
