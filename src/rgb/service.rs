//! RGB Lighting Context Service
//!
//! Stores the system-wide lighting "vibe": a single ambient color (RGBA)
//! and an optional matrix of colors.
//!
//! The service implements two interfaces:
//! 1. Observer: read and subscribe, for hardware drivers.
//! 2. Control: write, for authorized lighting controllers.
//!
//! Subscribers always receive the latest state. If several updates arrive
//! while a subscriber is still writing the previous one, it skips straight
//! to the newest instead of queueing or being disconnected.

use crate::auth;
use crate::config;
use crate::rgb::{control, observer};
use crate::util::lock;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

/// The maximum number of pixels in the lighting matrix (128 x 128).
pub const MAX_MATRIX_PIXELS: i64 = 16384;

/// How often an idle subscription checks whether its client hung up.
const HANGUP_CHECK_INTERVAL: Duration = Duration::from_secs(1);

struct Shared {
    main_color: observer::Color,
    matrix: Option<observer::Matrix>,
    /// Incremented on every change; subscribers compare against it.
    generation: u64,
}

#[derive(Clone)]
pub struct RgbService {
    inner: Arc<(Mutex<Shared>, Condvar)>,
}

/// A rejected `SetLightingContext` request.
#[derive(Debug, PartialEq, Eq)]
pub enum Rejection {
    InvalidColor(&'static str, i64),
    InvalidMatrixSize { width: i64, height: i64, len: i64 },
    MatrixTooLarge,
    ConflictingArguments,
}

/// Checks a request without touching any state.
pub fn validate(
    main_color: Option<&control::Color>,
    matrix: Option<&control::Matrix>,
    clear_matrix: bool,
) -> Result<(), Rejection> {
    if clear_matrix && matrix.is_some() {
        return Err(Rejection::ConflictingArguments);
    }
    if let Some(c) = main_color {
        validate_color(c)?;
    }
    if let Some(m) = matrix {
        let len = m.data.len() as i64;
        let size_error = Rejection::InvalidMatrixSize {
            width: m.width,
            height: m.height,
            len,
        };
        if m.width <= 0 || m.height <= 0 {
            return Err(size_error);
        }
        // checked_mul: width * height must not wrap around (2^32 * 2^32 = 0).
        match m.width.checked_mul(m.height) {
            Some(total) if total <= MAX_MATRIX_PIXELS => {
                if len != total {
                    return Err(size_error);
                }
            }
            _ => return Err(Rejection::MatrixTooLarge),
        }
        m.data.iter().try_for_each(validate_color)?;
    }
    Ok(())
}

fn validate_color(c: &control::Color) -> Result<(), Rejection> {
    for (name, value) in [("r", c.r), ("g", c.g), ("b", c.b), ("a", c.a)] {
        if !(0..=255).contains(&value) {
            return Err(Rejection::InvalidColor(name, value));
        }
    }
    Ok(())
}

fn to_observer(c: control::Color) -> observer::Color {
    observer::Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

impl Default for RgbService {
    fn default() -> Self {
        Self::new()
    }
}

impl RgbService {
    /// Creates a new RgbService with default "dim grey" ambient lighting.
    pub fn new() -> Self {
        Self {
            inner: Arc::new((
                Mutex::new(Shared {
                    main_color: observer::Color {
                        r: 50,
                        g: 50,
                        b: 50,
                        a: 255,
                    },
                    matrix: None,
                    generation: 0,
                }),
                Condvar::new(),
            )),
        }
    }

    fn snapshot(&self) -> (observer::Color, Option<observer::Matrix>, u64) {
        let s = lock(&self.inner.0);
        (s.main_color.clone(), s.matrix.clone(), s.generation)
    }

    /// Applies an already validated update and wakes all subscribers.
    pub fn apply(
        &self,
        main_color: Option<control::Color>,
        matrix: Option<control::Matrix>,
        clear_matrix: bool,
    ) {
        let (mutex, cvar) = &*self.inner;
        let mut s = lock(mutex);
        if let Some(c) = main_color {
            s.main_color = to_observer(c);
        }
        if clear_matrix {
            s.matrix = None;
        }
        if let Some(m) = matrix {
            s.matrix = Some(observer::Matrix {
                width: m.width,
                height: m.height,
                data: m.data.into_iter().map(to_observer).collect(),
            });
        }
        s.generation += 1;
        drop(s);
        cvar.notify_all();
    }

    /// Blocks until the state is newer than `seen`, or the timeout passes.
    fn wait_for_change(
        &self,
        seen: u64,
        timeout: Duration,
    ) -> Option<(observer::Color, Option<observer::Matrix>, u64)> {
        let (mutex, cvar) = &*self.inner;
        let guard = lock(mutex);
        let (s, _) = cvar
            .wait_timeout_while(guard, timeout, |s| s.generation == seen)
            .unwrap_or_else(PoisonError::into_inner);
        (s.generation != seen).then(|| (s.main_color.clone(), s.matrix.clone(), s.generation))
    }
}

/// Implementation of the Observer interface (Read/Subscribe)
impl observer::VarlinkInterface for RgbService {
    fn get_lighting_context(
        &self,
        call: &mut dyn observer::Call_GetLightingContext,
    ) -> varlink::Result<()> {
        let (color, matrix, _) = self.snapshot();
        call.reply(color, matrix)
    }

    fn subscribe_lighting_context(
        &self,
        call: &mut dyn observer::Call_SubscribeLightingContext,
    ) -> varlink::Result<()> {
        let (color, matrix, mut seen) = self.snapshot();
        if !call.wants_more() {
            // Called without "more": behave like GetLightingContext.
            return call.reply(color, matrix);
        }
        call.set_continues(true);
        call.reply(color, matrix)?;

        loop {
            match self.wait_for_change(seen, HANGUP_CHECK_INTERVAL) {
                Some((color, matrix, generation)) => {
                    seen = generation;
                    call.set_continues(true);
                    if let Err(e) = call.reply(color, matrix) {
                        log::debug!("Subscriber disconnected: {}", e);
                        break;
                    }
                }
                None => {
                    if auth::current_connection_closed() {
                        log::debug!("Subscriber hung up");
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Implementation of the Control interface (Write)
impl control::VarlinkInterface for RgbService {
    fn set_lighting_context(
        &self,
        call: &mut dyn control::Call_SetLightingContext,
        main_color: Option<control::Color>,
        matrix: Option<control::Matrix>,
        clear_matrix: Option<bool>,
    ) -> varlink::Result<()> {
        let peer = auth::current_peer();
        if let Err(caller) =
            auth::verify_unit_access(&config::get().auth.authorized_units, peer.as_ref())
        {
            log::warn!("Unauthorized lighting change from {}", caller);
            return call.reply_permission_denied(caller);
        }

        let clear_matrix = clear_matrix.unwrap_or(false);
        match validate(main_color.as_ref(), matrix.as_ref(), clear_matrix) {
            Ok(()) => {}
            Err(Rejection::InvalidColor(component, value)) => {
                return call.reply_invalid_color_value(component.to_string(), value);
            }
            Err(Rejection::InvalidMatrixSize { width, height, len }) => {
                return call.reply_invalid_matrix_size(width, height, len);
            }
            Err(Rejection::MatrixTooLarge) => {
                return call.reply_matrix_too_large(MAX_MATRIX_PIXELS);
            }
            Err(Rejection::ConflictingArguments) => {
                return call.reply_conflicting_arguments();
            }
        }

        self.apply(main_color, matrix, clear_matrix);
        log::debug!("Lighting context updated");
        call.reply()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(v: i64) -> control::Color {
        control::Color {
            r: v,
            g: v,
            b: v,
            a: 255,
        }
    }

    fn matrix(width: i64, height: i64, len: usize) -> control::Matrix {
        control::Matrix {
            width,
            height,
            data: vec![color(1); len],
        }
    }

    #[test]
    fn accepts_valid_input() {
        assert_eq!(
            validate(Some(&color(255)), Some(&matrix(4, 2, 8)), false),
            Ok(())
        );
        assert_eq!(
            validate(None, Some(&matrix(128, 128, 16384)), false),
            Ok(())
        );
    }

    #[test]
    fn rejects_bad_colors() {
        assert_eq!(
            validate(Some(&color(256)), None, false),
            Err(Rejection::InvalidColor("r", 256))
        );
        let mut m = matrix(1, 1, 1);
        m.data[0].a = -1;
        assert_eq!(
            validate(None, Some(&m), false),
            Err(Rejection::InvalidColor("a", -1))
        );
    }

    #[test]
    fn rejects_overflowing_dimensions() {
        let huge = 1i64 << 32;
        assert_eq!(
            validate(None, Some(&matrix(huge, huge, 0)), false),
            Err(Rejection::MatrixTooLarge)
        );
        assert_eq!(
            validate(None, Some(&matrix(i64::MAX, 2, 0)), false),
            Err(Rejection::MatrixTooLarge)
        );
        assert_eq!(
            validate(None, Some(&matrix(129, 128, 0)), false),
            Err(Rejection::MatrixTooLarge)
        );
    }

    #[test]
    fn rejects_bad_sizes() {
        assert!(matches!(
            validate(None, Some(&matrix(0, 0, 0)), false),
            Err(Rejection::InvalidMatrixSize { .. })
        ));
        assert!(matches!(
            validate(None, Some(&matrix(-2, -2, 4)), false),
            Err(Rejection::InvalidMatrixSize { .. })
        ));
        assert!(matches!(
            validate(None, Some(&matrix(2, 2, 3)), false),
            Err(Rejection::InvalidMatrixSize { .. })
        ));
    }

    #[test]
    fn clear_and_matrix_conflict() {
        assert_eq!(
            validate(None, Some(&matrix(1, 1, 1)), true),
            Err(Rejection::ConflictingArguments)
        );
    }

    #[test]
    fn matrix_can_be_cleared() {
        let s = RgbService::new();
        s.apply(None, Some(matrix(1, 1, 1)), false);
        assert!(s.snapshot().1.is_some());
        s.apply(Some(color(9)), None, false);
        assert!(s.snapshot().1.is_some(), "main color alone keeps matrix");
        s.apply(None, None, true);
        assert!(s.snapshot().1.is_none());
        assert_eq!(s.snapshot().0.r, 9);
    }

    #[test]
    fn waiters_get_latest_state() {
        let s = RgbService::new();
        let seen = s.snapshot().2;
        assert!(s.wait_for_change(seen, Duration::from_millis(10)).is_none());
        s.apply(Some(color(1)), None, false);
        s.apply(Some(color(2)), None, false);
        let (c, _, generation) = s.wait_for_change(seen, Duration::from_millis(10)).unwrap();
        assert_eq!((c.r, generation), (2, seen + 2), "coalesced to newest");
    }
}
