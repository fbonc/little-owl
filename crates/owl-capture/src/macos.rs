use std::task::Context;

use crate::{Capture, CaptureMethod, Capturer, Error, Provenance, Result, SpanCaptureMethod, ContextCaptureMethod};

use axuielement::prelude::*;

pub struct MacosCapturer { }

impl MacosCapturer {
    pub fn new() -> Self { Self { } }
}

impl Capturer for MacosCapturer {
    fn capture(&self) -> Result<Capture> {
        todo!()
    }

    fn capture_span(&self, prefer: &[crate::SpanCaptureMethod]) -> Result<(String, CaptureMethod<SpanCaptureMethod>)> {
        todo!()
    }

    fn capture_context(&self, prefer: &[crate::ContextCaptureMethod]) -> Result<(String, CaptureMethod<ContextCaptureMethod>)> {
        todo!()
    }

    fn capture_provenance(&self) -> Result<Provenance> {
        todo!()
    }
}