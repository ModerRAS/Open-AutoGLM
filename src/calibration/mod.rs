//! Coordinate calibration module for automatic scale factor detection.

mod calibrator;

pub use calibrator::{
    CalibrationConfig, CalibrationMode, CalibrationResult, CoordinateCalibrator,
    DEFAULT_CALIBRATION_POINTS,
};
