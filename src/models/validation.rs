use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelValidationError {
    pub field: &'static str,
    pub message: &'static str,
}

impl ModelValidationError {
    pub fn new(field: &'static str, message: &'static str) -> Self {
        Self { field, message }
    }
}

impl fmt::Display for ModelValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for ModelValidationError {}

pub(crate) fn check_finite(
    value: Option<f64>,
    field: &'static str,
) -> Result<(), ModelValidationError> {
    if let Some(value) = value {
        if !value.is_finite() {
            return Err(ModelValidationError::new(field, "value must be finite"));
        }
    }

    Ok(())
}

pub(crate) fn check_range(
    value: Option<f64>,
    min: f64,
    max: f64,
    field: &'static str,
) -> Result<(), ModelValidationError> {
    check_finite(value, field)?;

    if let Some(value) = value {
        if !(min..=max).contains(&value) {
            return Err(ModelValidationError::new(
                field,
                "value is outside the allowed range",
            ));
        }
    }

    Ok(())
}

pub(crate) fn check_non_negative(
    value: Option<f64>,
    field: &'static str,
) -> Result<(), ModelValidationError> {
    check_range(value, 0.0, f64::MAX, field)
}

pub(crate) fn check_positive(
    value: Option<f64>,
    field: &'static str,
) -> Result<(), ModelValidationError> {
    check_finite(value, field)?;

    if let Some(value) = value {
        if value <= 0.0 {
            return Err(ModelValidationError::new(
                field,
                "value must be greater than zero",
            ));
        }
    }

    Ok(())
}
