//! RGB Lighting Context Service
//!
//! Stores the system-wide lighting "vibe": a single ambient color (RGBA)
//! and an optional matrix of colors.
//!
//! The service implements two interfaces:
//! 1. Observer: read and subscribe, for hardware drivers.
//! 2. Control: write, for authorized lighting controllers.
//!
//! The matrix is stored as raw RGBA bytes behind an `Arc`, with its base64
//! form computed once per update, so handing it to many subscribers is
//! cheap. Clients choose between the compact `packed_matrix` form and the
//! original `matrix` (a JSON array of colors).
//!
//! Subscribers always receive the latest state. If several updates arrive
//! while a subscriber is still writing the previous one, it skips straight
//! to the newest instead of queueing or being disconnected.

use crate::auth;
use crate::config;
use crate::rgb::{control, observer};
use crate::util::lock;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

/// The maximum number of pixels in the lighting matrix (128 x 128).
pub const MAX_MATRIX_PIXELS: i64 = 16384;

/// How often an idle subscription checks whether its client hung up.
const HANGUP_CHECK_INTERVAL: Duration = Duration::from_secs(1);

/// A validated matrix: `width * height` pixels, 4 bytes each (RGBA).
#[derive(Debug, PartialEq, Eq)]
pub struct StoredMatrix {
    width: i64,
    height: i64,
    rgba: Vec<u8>,
    rgba_base64: String,
}

impl StoredMatrix {
    fn new(width: i64, height: i64, rgba: Vec<u8>) -> Self {
        let rgba_base64 = BASE64.encode(&rgba);
        Self {
            width,
            height,
            rgba,
            rgba_base64,
        }
    }

    fn packed(&self) -> observer::PackedMatrix {
        observer::PackedMatrix {
            width: self.width,
            height: self.height,
            rgba: self.rgba_base64.clone(),
        }
    }

    fn expanded(&self) -> observer::Matrix {
        observer::Matrix {
            width: self.width,
            height: self.height,
            data: self
                .rgba
                .chunks_exact(4)
                .map(|p| observer::Color {
                    r: p[0] as i64,
                    g: p[1] as i64,
                    b: p[2] as i64,
                    a: p[3] as i64,
                })
                .collect(),
        }
    }
}

/// What a request does to the matrix.
#[derive(Debug, PartialEq, Eq)]
pub enum MatrixUpdate {
    Keep,
    Clear,
    Set(StoredMatrix),
}

struct Shared {
    main_color: observer::Color,
    matrix: Option<Arc<StoredMatrix>>,
    /// Incremented on every change; subscribers compare against it.
    generation: u64,
}

type Snapshot = (observer::Color, Option<Arc<StoredMatrix>>, u64);

#[derive(Clone)]
pub struct RgbService {
    inner: Arc<(Mutex<Shared>, Condvar)>,
}

/// A rejected `SetLightingContext` request.
#[derive(Debug, PartialEq, Eq)]
pub enum Rejection {
    InvalidColor(&'static str, i64),
    InvalidMatrixSize { width: i64, height: i64, len: i64 },
    InvalidPacked(String),
    MatrixTooLarge,
    ConflictingArguments,
}

/// Checks dimensions against the pixel count `len`.
fn check_size(width: i64, height: i64, len: i64) -> Result<(), Rejection> {
    if width <= 0 || height <= 0 {
        return Err(Rejection::InvalidMatrixSize { width, height, len });
    }
    // checked_mul: width * height must not wrap around (2^32 * 2^32 = 0).
    match width.checked_mul(height) {
        Some(total) if total <= MAX_MATRIX_PIXELS => {
            if len != total {
                return Err(Rejection::InvalidMatrixSize { width, height, len });
            }
            Ok(())
        }
        _ => Err(Rejection::MatrixTooLarge),
    }
}

/// Checks a request without touching any state, and decodes the matrix.
pub fn validate(
    main_color: Option<&control::Color>,
    matrix: Option<&control::Matrix>,
    packed: Option<&control::PackedMatrix>,
    clear_matrix: bool,
) -> Result<MatrixUpdate, Rejection> {
    let given = matrix.is_some() as u8 + packed.is_some() as u8 + clear_matrix as u8;
    if given > 1 {
        return Err(Rejection::ConflictingArguments);
    }
    if let Some(c) = main_color {
        validate_color(c)?;
    }
    if let Some(m) = matrix {
        check_size(m.width, m.height, m.data.len() as i64)?;
        m.data.iter().try_for_each(validate_color)?;
        let rgba = m
            .data
            .iter()
            .flat_map(|c| [c.r as u8, c.g as u8, c.b as u8, c.a as u8])
            .collect();
        return Ok(MatrixUpdate::Set(StoredMatrix::new(
            m.width, m.height, rgba,
        )));
    }
    if let Some(p) = packed {
        // Reject oversized input before decoding it.
        let max_encoded = (MAX_MATRIX_PIXELS as usize * 4).div_ceil(3) * 4;
        if p.rgba.len() > max_encoded {
            return Err(Rejection::MatrixTooLarge);
        }
        let rgba = BASE64
            .decode(p.rgba.as_bytes())
            .map_err(|e| Rejection::InvalidPacked(format!("rgba is not valid base64: {e}")))?;
        if rgba.len() % 4 != 0 {
            return Err(Rejection::InvalidPacked(format!(
                "rgba has {} bytes, not a whole number of 4-byte pixels",
                rgba.len()
            )));
        }
        check_size(p.width, p.height, (rgba.len() / 4) as i64)?;
        return Ok(MatrixUpdate::Set(StoredMatrix::new(
            p.width, p.height, rgba,
        )));
    }
    Ok(if clear_matrix {
        MatrixUpdate::Clear
    } else {
        MatrixUpdate::Keep
    })
}

fn validate_color(c: &control::Color) -> Result<(), Rejection> {
    for (name, value) in [("r", c.r), ("g", c.g), ("b", c.b), ("a", c.a)] {
        if !(0..=255).contains(&value) {
            return Err(Rejection::InvalidColor(name, value));
        }
    }
    Ok(())
}

/// The matrix fields of a reply, in the form the client asked for.
fn matrix_fields(
    matrix: Option<&StoredMatrix>,
    packed: bool,
) -> (Option<observer::Matrix>, Option<observer::PackedMatrix>) {
    match (matrix, packed) {
        (None, _) => (None, None),
        (Some(m), true) => (None, Some(m.packed())),
        (Some(m), false) => (Some(m.expanded()), None),
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

    fn snapshot(&self) -> Snapshot {
        let s = lock(&self.inner.0);
        (s.main_color.clone(), s.matrix.clone(), s.generation)
    }

    /// Applies an already validated update and wakes all subscribers.
    pub fn apply(&self, main_color: Option<control::Color>, matrix: MatrixUpdate) {
        let (mutex, cvar) = &*self.inner;
        let mut s = lock(mutex);
        if let Some(c) = main_color {
            s.main_color = observer::Color {
                r: c.r,
                g: c.g,
                b: c.b,
                a: c.a,
            };
        }
        match matrix {
            MatrixUpdate::Keep => {}
            MatrixUpdate::Clear => s.matrix = None,
            MatrixUpdate::Set(m) => s.matrix = Some(Arc::new(m)),
        }
        s.generation += 1;
        drop(s);
        cvar.notify_all();
    }

    /// Blocks until the state is newer than `seen`, or the timeout passes.
    fn wait_for_change(&self, seen: u64, timeout: Duration) -> Option<Snapshot> {
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
        packed: Option<bool>,
    ) -> varlink::Result<()> {
        let (color, matrix, _) = self.snapshot();
        let (matrix, packed_matrix) = matrix_fields(matrix.as_deref(), packed == Some(true));
        call.reply(color, matrix, packed_matrix)
    }

    fn subscribe_lighting_context(
        &self,
        call: &mut dyn observer::Call_SubscribeLightingContext,
        packed: Option<bool>,
    ) -> varlink::Result<()> {
        let packed = packed == Some(true);
        let (color, matrix, mut seen) = self.snapshot();
        let (m, p) = matrix_fields(matrix.as_deref(), packed);
        if !call.wants_more() {
            // Called without "more": behave like GetLightingContext.
            return call.reply(color, m, p);
        }
        call.set_continues(true);
        call.reply(color, m, p)?;

        loop {
            match self.wait_for_change(seen, HANGUP_CHECK_INTERVAL) {
                Some((color, matrix, generation)) => {
                    seen = generation;
                    let (m, p) = matrix_fields(matrix.as_deref(), packed);
                    call.set_continues(true);
                    if let Err(e) = call.reply(color, m, p) {
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
        packed_matrix: Option<control::PackedMatrix>,
        clear_matrix: Option<bool>,
    ) -> varlink::Result<()> {
        let peer = auth::current_peer();
        if let Err(caller) =
            auth::verify_unit_access(&config::get().auth.authorized_units, peer.as_ref())
        {
            log::warn!("Unauthorized lighting change from {}", caller);
            return call.reply_permission_denied(caller);
        }

        let update = match validate(
            main_color.as_ref(),
            matrix.as_ref(),
            packed_matrix.as_ref(),
            clear_matrix.unwrap_or(false),
        ) {
            Ok(update) => update,
            Err(Rejection::InvalidColor(component, value)) => {
                return call.reply_invalid_color_value(component.to_string(), value);
            }
            Err(Rejection::InvalidMatrixSize { width, height, len }) => {
                return call.reply_invalid_matrix_size(width, height, len);
            }
            Err(Rejection::InvalidPacked(reason)) => {
                return call.reply_invalid_packed_matrix(reason);
            }
            Err(Rejection::MatrixTooLarge) => {
                return call.reply_matrix_too_large(MAX_MATRIX_PIXELS);
            }
            Err(Rejection::ConflictingArguments) => {
                return call.reply_conflicting_arguments();
            }
        };

        self.apply(main_color, update);
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

    fn packed(width: i64, height: i64, bytes: &[u8]) -> control::PackedMatrix {
        control::PackedMatrix {
            width,
            height,
            rgba: BASE64.encode(bytes),
        }
    }

    fn check(m: Option<&control::Matrix>) -> Result<MatrixUpdate, Rejection> {
        validate(None, m, None, false)
    }

    #[test]
    fn accepts_valid_input() {
        assert!(validate(Some(&color(255)), Some(&matrix(4, 2, 8)), None, false).is_ok());
        assert!(check(Some(&matrix(128, 128, 16384))).is_ok());
    }

    #[test]
    fn rejects_bad_colors() {
        assert_eq!(
            validate(Some(&color(256)), None, None, false),
            Err(Rejection::InvalidColor("r", 256))
        );
        let mut m = matrix(1, 1, 1);
        m.data[0].a = -1;
        assert_eq!(check(Some(&m)), Err(Rejection::InvalidColor("a", -1)));
    }

    #[test]
    fn rejects_overflowing_dimensions() {
        let huge = 1i64 << 32;
        assert_eq!(
            check(Some(&matrix(huge, huge, 0))),
            Err(Rejection::MatrixTooLarge)
        );
        assert_eq!(
            check(Some(&matrix(i64::MAX, 2, 0))),
            Err(Rejection::MatrixTooLarge)
        );
        assert_eq!(
            check(Some(&matrix(129, 128, 0))),
            Err(Rejection::MatrixTooLarge)
        );
    }

    #[test]
    fn rejects_bad_sizes() {
        for m in [matrix(0, 0, 0), matrix(-2, -2, 4), matrix(2, 2, 3)] {
            assert!(matches!(
                check(Some(&m)),
                Err(Rejection::InvalidMatrixSize { .. })
            ));
        }
    }

    #[test]
    fn only_one_matrix_argument() {
        let p = packed(1, 1, &[1, 2, 3, 4]);
        let m = matrix(1, 1, 1);
        for (m, p, clear) in [
            (Some(&m), None, true),
            (Some(&m), Some(&p), false),
            (None, Some(&p), true),
        ] {
            assert_eq!(
                validate(None, m, p, clear),
                Err(Rejection::ConflictingArguments)
            );
        }
    }

    #[test]
    fn packed_round_trips_with_expanded() {
        let bytes: Vec<u8> = (0..2 * 3 * 4).map(|i| i as u8).collect();
        let MatrixUpdate::Set(stored) =
            validate(None, None, Some(&packed(2, 3, &bytes)), false).unwrap()
        else {
            panic!("expected a matrix");
        };
        let expanded = stored.expanded();
        assert_eq!(expanded.data.len(), 6);
        assert_eq!((expanded.data[1].r, expanded.data[1].a), (4, 7));
        assert_eq!(stored.packed().rgba, BASE64.encode(&bytes));

        // The JSON form of the same pixels stores the same bytes.
        let as_json = control::Matrix {
            width: 2,
            height: 3,
            data: expanded
                .data
                .iter()
                .map(|c| control::Color {
                    r: c.r,
                    g: c.g,
                    b: c.b,
                    a: c.a,
                })
                .collect(),
        };
        assert_eq!(check(Some(&as_json)), Ok(MatrixUpdate::Set(stored)));
    }

    #[test]
    fn rejects_bad_packed_data() {
        let bad = control::PackedMatrix {
            width: 1,
            height: 1,
            rgba: "not base64!".into(),
        };
        assert!(matches!(
            validate(None, None, Some(&bad), false),
            Err(Rejection::InvalidPacked(_))
        ));
        assert!(matches!(
            validate(None, None, Some(&packed(1, 1, &[1, 2, 3])), false),
            Err(Rejection::InvalidPacked(_))
        ));
        assert!(matches!(
            validate(None, None, Some(&packed(2, 2, &[0; 4])), false),
            Err(Rejection::InvalidMatrixSize { .. })
        ));
        let too_big = packed(128, 129, &vec![0; 128 * 129 * 4]);
        assert_eq!(
            validate(None, None, Some(&too_big), false),
            Err(Rejection::MatrixTooLarge)
        );
    }

    #[test]
    fn packed_is_much_smaller_than_json() {
        let MatrixUpdate::Set(stored) = check(Some(&matrix(128, 128, 16384))).unwrap() else {
            panic!("expected a matrix");
        };
        let json = serde_json::to_string(&stored.expanded()).unwrap().len();
        let compact = serde_json::to_string(&stored.packed()).unwrap().len();
        assert!(
            compact * 4 < json,
            "packed {compact} bytes vs JSON {json} bytes"
        );
    }

    #[test]
    fn matrix_can_be_cleared() {
        let s = RgbService::new();
        s.apply(None, check(Some(&matrix(1, 1, 1))).unwrap());
        assert!(s.snapshot().1.is_some());
        s.apply(Some(color(9)), MatrixUpdate::Keep);
        assert!(s.snapshot().1.is_some(), "main color alone keeps matrix");
        s.apply(None, MatrixUpdate::Clear);
        assert!(s.snapshot().1.is_none());
        assert_eq!(s.snapshot().0.r, 9);
    }

    #[test]
    fn waiters_get_latest_state() {
        let s = RgbService::new();
        let seen = s.snapshot().2;
        assert!(s.wait_for_change(seen, Duration::from_millis(10)).is_none());
        s.apply(Some(color(1)), MatrixUpdate::Keep);
        s.apply(Some(color(2)), MatrixUpdate::Keep);
        let (c, _, generation) = s.wait_for_change(seen, Duration::from_millis(10)).unwrap();
        assert_eq!((c.r, generation), (2, seen + 2), "coalesced to newest");
    }
}
