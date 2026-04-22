//! RGB Lighting Context Service
//!
//! This service provides a passive storage for the system-wide lighting "vibe,"
//! which can be a single ambient color (RGBA) or an N-N matrix of colors.
//!
//! The service implements two distinct interfaces:
//! 1. Observer: A read-only/subscriber interface for hardware drivers.
//! 2. Control: A write-only interface for authorized lighting controllers.

use crate::rgb::control;
use crate::rgb::observer;
use std::sync::{Arc, RwLock, mpsc};

/// The maximum allowed dimension for the lighting matrix (N x N)
const MAX_MATRIX_SIZE: i64 = 420;

/// Shared internal state holding the current lighting context and subscribers
#[derive(Clone)]
pub struct RgbService {
    context: Arc<RwLock<LightingState>>,
    subscribers: Arc<RwLock<Vec<mpsc::SyncSender<LightingUpdate>>>>,
}

/// Internal state holding the current lighting context.
/// Note: We use observer types as our internal "canonical" state.
struct LightingState {
    main_color: observer::Color,
    matrix: Option<observer::Matrix>,
}

/// Message sent to subscribers when the context changes
#[derive(Clone)]
struct LightingUpdate {
    main_color: observer::Color,
    matrix: Option<observer::Matrix>,
}

impl RgbService {
    /// Creates a new RgbService with default "dim grey" ambient lighting
    pub fn new() -> Self {
        Self {
            context: Arc::new(RwLock::new(LightingState {
                main_color: observer::Color {
                    r: 50,
                    g: 50,
                    b: 50,
                    a: 255, // Default fully opaque
                },
                matrix: None,
            })),
            subscribers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Helper to broadcast updates to all active subscribers
    fn broadcast(&self, update: LightingUpdate) {
        let mut subs = self.subscribers.write().unwrap();
        subs.retain(|tx| match tx.try_send(update.clone()) {
            Ok(_) => true,
            Err(mpsc::TrySendError::Full(_)) => {
                log::debug!("Subscriber too slow, dropping");
                false
            }
            Err(mpsc::TrySendError::Disconnected(_)) => false,
        });
    }

    /// Validates that an RGBA color has components within the 0-255 range
    fn validate_color(
        &self,
        call: &mut dyn control::Call_SetLightingContext,
        c: &control::Color,
    ) -> varlink::Result<bool> {
        if c.r < 0 || c.r > 255 {
            call.reply_invalid_color_value("r".to_string(), c.r)?;
            return Ok(false);
        }
        if c.g < 0 || c.g > 255 {
            call.reply_invalid_color_value("g".to_string(), c.g)?;
            return Ok(false);
        }
        if c.b < 0 || c.b > 255 {
            call.reply_invalid_color_value("b".to_string(), c.b)?;
            return Ok(false);
        }
        if c.a < 0 || c.a > 255 {
            call.reply_invalid_color_value("a".to_string(), c.a)?;
            return Ok(false);
        }
        Ok(true)
    }
}

/// Implementation of the Observer interface (Read/Subscribe)
impl observer::VarlinkInterface for RgbService {
    fn get_lighting_context(
        &self,
        call: &mut dyn observer::Call_GetLightingContext,
    ) -> varlink::Result<()> {
        let state = self.context.read().unwrap();
        call.reply(state.main_color.clone(), state.matrix.clone())
    }

    fn subscribe_lighting_context(
        &self,
        call: &mut dyn observer::Call_SubscribeLightingContext,
    ) -> varlink::Result<()> {
        let (tx, rx) = mpsc::sync_channel(16);
        {
            let mut subs = self.subscribers.write().unwrap();
            subs.push(tx);
        }

        // Initial reply
        {
            let state = self.context.read().unwrap();
            call.set_continues(true);
            call.reply(state.main_color.clone(), state.matrix.clone())?;
        }

        while let Ok(update) = rx.recv() {
            call.set_continues(true);
            if let Err(e) = call.reply(update.main_color, update.matrix) {
                log::debug!("Subscriber disconnected: {}", e);
                break;
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
    ) -> varlink::Result<()> {
        // 1. Validation Logic
        if let Some(c) = &main_color
            && !self.validate_color(call, c)?
        {
            return Ok(());
        }

        if let Some(m) = &matrix {
            if m.size <= 0 {
                return call.reply_invalid_matrix_size(m.size, m.data.len() as i64);
            }
            if m.size > MAX_MATRIX_SIZE {
                return call.reply_matrix_too_large(MAX_MATRIX_SIZE);
            }
            if m.data.len() != (m.size * m.size) as usize {
                return call.reply_invalid_matrix_size(m.size, m.data.len() as i64);
            }
            // Validate every pixel in the matrix
            for pixel in &m.data {
                if !self.validate_color(call, pixel)? {
                    return Ok(());
                }
            }
        }

        // 2. Commit Logic
        let mut state = self.context.write().unwrap();

        // Convert control types to internal/observer types
        if let Some(c) = main_color {
            state.main_color = observer::Color {
                r: c.r,
                g: c.g,
                b: c.b,
                a: c.a,
            };
        }

        if let Some(m) = matrix {
            state.matrix = Some(observer::Matrix {
                size: m.size,
                data: m
                    .data
                    .into_iter()
                    .map(|c| observer::Color {
                        r: c.r,
                        g: c.g,
                        b: c.b,
                        a: c.a,
                    })
                    .collect(),
            });
        }

        let update = LightingUpdate {
            main_color: state.main_color.clone(),
            matrix: state.matrix.clone(),
        };

        log::debug!("Lighting context updated, broadcasting to observers");
        self.broadcast(update);

        call.reply()
    }
}
