//! Coordinate calibration module for automatic scale factor detection.

mod calibrator;

pub use calibrator::{
    CalibrationConfig, CalibrationResult, CoordinateCalibrator,
    DEFAULT_CALIBRATION_POINTS,
};
